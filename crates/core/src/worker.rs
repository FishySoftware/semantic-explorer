use anyhow::Result;
use async_nats::jetstream::consumer::{Consumer, pull::Config};
use futures_util::StreamExt;
use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tracing::{Instrument, error, info, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{
    EnvFilter, Layer, Registry as TracingRegistry, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::adaptive_concurrency::AdaptiveConcurrency;
use crate::config::NatsConfig;
use crate::observability;

/// Configuration for worker initialization
pub struct WorkerConfig {
    pub service_name: String,
    pub stream_name: String,
    pub consumer_config: Config,
    pub max_concurrent_jobs: usize,
    /// Maximum number of delivery attempts before sending to DLQ
    pub max_deliver: u64,
    /// Port for the health check HTTP server (default: 8082)
    pub health_check_port: u16,
    /// NATS configuration (extracted from env in main)
    pub nats_config: NatsConfig,
}

/// DLQ subject mapping for each stream type
fn get_dlq_subject(stream_name: &str) -> &'static str {
    match stream_name {
        "COLLECTION_TRANSFORMS" => "dlq.collection-transforms",
        "DATASET_TRANSFORMS" => "dlq.dataset-transforms",
        "VISUALIZATION_TRANSFORMS" => "dlq.visualization-transforms",
        _ => "dlq.unknown-transforms",
    }
}

/// Get transform type from stream name for metrics
fn get_transform_type(stream_name: &str) -> &'static str {
    match stream_name {
        "COLLECTION_TRANSFORMS" => "collection",
        "DATASET_TRANSFORMS" => "dataset",
        "VISUALIZATION_TRANSFORMS" => "visualization",
        _ => "unknown",
    }
}

#[derive(Clone)]
pub struct WorkerContext {
    pub s3_client: aws_sdk_s3::Client,
    pub nats_client: async_nats::Client,
}

/// Initialize OpenTelemetry for the worker
pub fn initialize_opentelemetry(
    service_name: &str,
    otlp_endpoint: &str,
    log_format: &str,
) -> Result<()> {
    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .build();

    // Initialize trace exporter
    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(10))
        .build();

    if let Ok(exporter) = trace_exporter {
        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource.clone())
            .with_id_generator(RandomIdGenerator::default())
            .with_sampler(Sampler::AlwaysOn)
            .build();
        global::set_tracer_provider(tracer_provider);
    }

    // Initialize log exporter
    let log_exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(10))
        .build();

    let logger_provider = if let Ok(exporter) = log_exporter {
        Some(
            SdkLoggerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(resource.clone())
                .build(),
        )
    } else {
        None
    };

    // Initialize metric exporter
    let metric_exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(10))
        .build();

    if let Ok(exporter) = metric_exporter {
        let meter_provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter)
            .with_resource(resource.clone())
            .build();
        global::set_meter_provider(meter_provider);
        info!("OpenTelemetry metrics initialized successfully");
    } else {
        info!("Failed to initialize OpenTelemetry metrics exporter");
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    observability::init_metrics_otel()?;

    // Initialize tracing subscriber
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to initialize tracing filter layer");

    // Use JSON format for structured logging in production, human-readable for development
    let use_json = log_format.to_lowercase() == "json";

    let format_layer = if use_json {
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_target(true)
            .with_file(true)
            .flatten_event(true)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .boxed()
    };

    let tracer = global::tracer_provider().tracer(service_name.to_string());
    let otel_trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    if let Some(logger_provider) = logger_provider {
        let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);
        TracingRegistry::default()
            .with(env_filter)
            .with(format_layer)
            .with(otel_trace_layer)
            .with(otel_log_layer)
            .try_init()?;
    } else {
        TracingRegistry::default()
            .with(env_filter)
            .with(format_layer)
            .with(otel_trace_layer)
            .try_init()?;
    }

    Ok(())
}

/// Run the worker message processing loop with graceful shutdown support
pub async fn run_worker<J, F, Fut>(
    config: WorkerConfig,
    context: WorkerContext,
    process_job: F,
) -> Result<()>
where
    J: DeserializeOwned + Send + 'static,
    F: Fn(J, WorkerContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    // Use provided NATS client or create a new connection
    let nats_client = context.nats_client.clone();

    let concurrency = AdaptiveConcurrency::new(config.max_concurrent_jobs);
    let shutdown = Arc::new(AtomicBool::new(false));
    let in_flight = Arc::new(AtomicUsize::new(0));

    info!("Using JetStream mode for reliable message delivery");

    // Initialize NATS JetStream with provided configuration
    crate::nats::initialize_jetstream(&nats_client, &config.nats_config).await?;

    // Start NATS metrics collector
    crate::nats::start_metrics_collector(nats_client.clone()).await?;

    // Start health check HTTP endpoint for K8s liveness/readiness probes
    let health_concurrency = Arc::clone(&concurrency);
    let health_in_flight = Arc::clone(&in_flight);
    let health_shutdown = Arc::clone(&shutdown);
    let health_service_name = config.service_name.clone();
    let health_port = config.health_check_port;
    tokio::spawn(async move {
        if let Err(e) = run_health_server(
            health_service_name,
            health_concurrency,
            health_in_flight,
            health_shutdown,
            health_port,
        )
        .await
        {
            warn!("Health check server failed: {}", e);
        }
    });

    // Clamp max_ack_pending to max_concurrent_jobs to prevent NAK/redelivery churn.
    // When max_ack_pending >> max_concurrent_jobs, NATS delivers far more messages
    // than the worker can process concurrently, causing the excess to be NAK'd and
    // redelivered repeatedly — wasting resources and inflating redelivery counts.
    let mut consumer_config = config.consumer_config;
    if consumer_config.max_ack_pending > config.max_concurrent_jobs as i64 {
        info!(
            "Clamping max_ack_pending from {} to {} to match max_concurrent_jobs",
            consumer_config.max_ack_pending, config.max_concurrent_jobs
        );
        consumer_config.max_ack_pending = config.max_concurrent_jobs as i64;
    }

    let jetstream = async_nats::jetstream::new(nats_client.clone());
    let consumer =
        crate::nats::ensure_consumer(&jetstream, &config.stream_name, consumer_config).await?;

    info!(
        "Worker started with JetStream, listening on {} stream",
        config.stream_name
    );

    // Set worker ready status
    crate::observability::set_worker_ready(&config.service_name, true);

    let service_name = config.service_name.clone();
    let shutdown_clone = shutdown.clone();
    let in_flight_clone = in_flight.clone();

    let proc_ctx = ProcessingContext {
        shutdown: shutdown.clone(),
        in_flight: in_flight.clone(),
        stream_name: config.stream_name.clone(),
        max_deliver: config.max_deliver,
    };

    // Run message processing with graceful shutdown
    tokio::select! {
        result = process_messages(
            consumer,
            context,
            concurrency,
            process_job,
            proc_ctx,
        ) => {
            result
        }
        _ = shutdown_signal() => {
            info!("Shutdown signal received, initiating graceful shutdown...");
            shutdown_clone.store(true, Ordering::SeqCst);

            // Wait for in-flight jobs to complete (max 5 minutes)
            let shutdown_timeout = Duration::from_secs(300);
            let start = std::time::Instant::now();

            while in_flight_clone.load(Ordering::SeqCst) > 0 {
                let remaining = in_flight_clone.load(Ordering::SeqCst);
                info!("Waiting for {} in-flight jobs to complete...", remaining);

                if start.elapsed() > shutdown_timeout {
                    warn!(
                        "Shutdown timeout reached, {} jobs still in progress",
                        remaining
                    );
                    break;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            crate::observability::set_worker_ready(&service_name, false);
            info!("Graceful shutdown complete");
            Ok(())
        }
    }
}

/// Wait for shutdown signals (SIGTERM or SIGINT)
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received SIGINT (Ctrl+C)");
        }
        _ = terminate => {
            info!("Received SIGTERM");
        }
    }
}

/// Internal context for message processing
struct ProcessingContext {
    shutdown: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
    stream_name: String,
    max_deliver: u64,
}

/// Process messages from the consumer with DLQ support
#[allow(clippy::too_many_arguments)]
async fn process_messages<J, F, Fut>(
    consumer: Consumer<Config>,
    context: WorkerContext,
    concurrency: Arc<AdaptiveConcurrency>,
    process_job: F,
    proc_ctx: ProcessingContext,
) -> Result<()>
where
    J: DeserializeOwned + Send + 'static,
    F: Fn(J, WorkerContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    let process_job = Arc::new(process_job);
    let mut messages = consumer.messages().await?;
    let jetstream = async_nats::jetstream::new(context.nats_client.clone());

    while let Some(msg) = messages.next().await {
        // Check for shutdown signal
        if proc_ctx.shutdown.load(Ordering::SeqCst) {
            info!("Shutdown in progress, stopping message consumption");
            break;
        }

        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to receive message: {}", e);
                continue;
            }
        };

        // Get delivery count for DLQ decision
        let delivery_count = msg.info().map(|info| info.delivered as u64).unwrap_or(1);

        // Extract trace context from message headers for distributed tracing
        let parent_context = crate::nats::extract_otel_context(msg.headers.as_ref());

        let job: J = match serde_json::from_slice(&msg.payload) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to deserialize job: {}", e);
                // Acknowledge the message to prevent reprocessing bad messages
                if let Err(ack_err) = msg.ack().await {
                    error!("Failed to acknowledge bad message: {}", ack_err);
                }
                continue;
            }
        };

        // If downstream is under pressure, pause briefly before acquiring
        // to avoid spinning on jobs that will likely fail anyway.
        if concurrency.is_downstream_pressured() {
            info!("Downstream pressure active, pausing 2s before accepting next job");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // Acquire semaphore permit for backpressure.
        // We use a blocking acquire here instead of try_acquire. Combined with
        // max_ack_pending being clamped to max_concurrent_jobs, NATS won't deliver
        // more messages than we can handle, so this should rarely block. If it does
        // block briefly (e.g., between a task acking and releasing its permit), it
        // naturally throttles consumption without causing NAK/redelivery churn.
        let permit = match tokio::time::timeout(
            Duration::from_secs(300), // Safety timeout (5 min) — should never be reached
            concurrency.acquire(),
        )
        .await
        {
            Ok(Ok(p)) => p,
            Ok(Err(_)) => {
                // Semaphore closed — shouldn't happen in normal operation
                error!("Semaphore closed unexpectedly");
                break;
            }
            Err(_) => {
                // Timeout waiting for permit — indicates max_ack_pending is too high
                // relative to max_concurrent_jobs, or jobs are taking too long
                warn!(
                    "Timed out waiting for semaphore permit (300s). \
                     Consider lowering NATS_MAX_ACK_PENDING to match MAX_CONCURRENT_JOBS."
                );
                if let Err(e) = msg
                    .ack_with(async_nats::jetstream::AckKind::Nak(Some(
                        Duration::from_secs(30),
                    )))
                    .await
                {
                    error!("Failed to Nak message during backpressure timeout: {}", e);
                }
                continue;
            }
        };

        // Track in-flight jobs for graceful shutdown
        proc_ctx.in_flight.fetch_add(1, Ordering::SeqCst);

        let ctx = context.clone();
        let process_job = Arc::clone(&process_job);
        let stream_name_clone = proc_ctx.stream_name.clone();
        let max_deliver = proc_ctx.max_deliver;
        let in_flight_clone = proc_ctx.in_flight.clone();
        let jetstream_clone = jetstream.clone();
        let payload = msg.payload.clone();
        let concurrency_clone = Arc::clone(&concurrency);

        // Create a span with the parent context for distributed tracing
        let job_span = info_span!(
            "process_worker_job",
            stream = %stream_name_clone,
            delivery_attempt = delivery_count,
        );
        let _ = job_span.set_parent(parent_context);

        tokio::spawn(
            async move {
                let _permit = permit; // Hold permit until task completes

                match process_job(job, ctx).await {
                    Ok(_) => {
                        info!("Job processed successfully.");
                        // Signal that downstream is healthy
                        concurrency_clone.record_downstream_success();
                        // Acknowledge success
                        if let Err(e) = msg.ack().await {
                            error!("Failed to acknowledge successful job: {}", e);
                        }
                    }
                    Err(e) => {
                        // Check if downstream embedding service is under pressure
                        // and propagate to adaptive concurrency controller
                        if crate::embedder::is_downstream_under_pressure() {
                            concurrency_clone.record_downstream_pressure();
                        }

                        let error_type = format!("{:?}", e)
                            .split(':')
                            .next()
                            .unwrap_or("Unknown")
                            .to_string();
                        crate::observability::record_worker_job_failure(
                            &stream_name_clone,
                            &error_type,
                        );

                        error!(
                            "Job failed (attempt {}/{}): {}",
                            delivery_count, max_deliver, e
                        );

                        // Check if we've exhausted retries
                        if delivery_count >= max_deliver {
                            // Send to DLQ
                            let dlq_subject = get_dlq_subject(&stream_name_clone);
                            let transform_type = get_transform_type(&stream_name_clone);

                            warn!(
                                "Max delivery attempts ({}) reached, sending to DLQ: {}",
                                max_deliver, dlq_subject
                            );

                            if let Err(dlq_err) =
                                jetstream_clone.publish(dlq_subject, payload).await
                            {
                                error!("Failed to publish to DLQ: {}", dlq_err);
                            } else {
                                crate::observability::record_dlq_message(
                                    transform_type,
                                    &error_type,
                                );
                                info!("Message sent to DLQ successfully");
                            }

                            // Acknowledge to remove from main queue
                            if let Err(ack_err) = msg.ack().await {
                                error!("Failed to acknowledge DLQ'd message: {}", ack_err);
                            }
                        } else {
                            // Negative acknowledgment for retry (30s delay)
                            if let Err(ack_err) = msg
                                .ack_with(async_nats::jetstream::AckKind::Nak(Some(
                                    Duration::from_secs(30),
                                )))
                                .await
                            {
                                error!("Failed to negatively acknowledge failed job: {}", ack_err);
                            } else {
                                // Track retry
                                crate::observability::record_worker_job_retry(
                                    &stream_name_clone,
                                    delivery_count as u32,
                                );
                            }
                        }
                    }
                }

                // Decrement in-flight counter
                in_flight_clone.fetch_sub(1, Ordering::SeqCst);
            }
            .instrument(job_span),
        );
    }

    Ok(())
}

/// Tiny HTTP health check server for K8s liveness/readiness probes.
///
/// Listens on `HEALTH_CHECK_PORT` (default 8082) and serves:
/// - `GET /healthz` — liveness probe (always 200 unless shutting down)
/// - `GET /readyz` — readiness probe (200 when not shutting down)
/// - `GET /status` — JSON status with concurrency info
async fn run_health_server(
    service_name: String,
    concurrency: Arc<AdaptiveConcurrency>,
    in_flight: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    port: u16,
) -> Result<()> {
    use std::io::Write;

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => {
            info!(port = port, "Health check server listening");
            l
        }
        Err(e) => {
            warn!(port = port, error = %e, "Failed to bind health check port, skipping");
            return Ok(());
        }
    };

    loop {
        let (stream, _) = listener.accept().await?;
        let is_shutdown = shutdown.load(Ordering::SeqCst);
        let current_in_flight = in_flight.load(Ordering::SeqCst);
        let effective = concurrency.effective_limit();
        let max = concurrency.max_limit();
        let available = concurrency.available_permits();
        let pressured = concurrency.is_downstream_pressured();
        let svc = service_name.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let stream = stream.into_std().ok();
            if let Some(mut stream) = stream {
                use std::io::Read;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);

                let (status, body) = if request.contains("GET /healthz") {
                    if is_shutdown {
                        ("503 Service Unavailable", "{\"status\":\"shutting_down\"}")
                    } else {
                        ("200 OK", "{\"status\":\"ok\"}")
                    }
                } else if request.contains("GET /readyz") {
                    if is_shutdown {
                        ("503 Service Unavailable", "{\"status\":\"shutting_down\"}")
                    } else {
                        ("200 OK", "{\"status\":\"ready\"}")
                    }
                } else if request.contains("GET /status") {
                    // Return detailed JSON status (we'll format inline)
                    let json = format!(
                        "{{\"service\":\"{}\",\"in_flight\":{},\"effective_limit\":{},\
                         \"max_limit\":{},\"available_permits\":{},\
                         \"downstream_pressure\":{}}}",
                        svc, current_in_flight, effective, max, available, pressured
                    );
                    // Can't use the json variable with a static lifetime; write directly
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        json.len(),
                        json
                    );
                    let _ = stream.write_all(response.as_bytes());
                    return;
                } else {
                    ("404 Not Found", "{\"error\":\"not_found\"}")
                };

                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    status,
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
    }
}
