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
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use tracing_subscriber::{
    EnvFilter, Layer, Registry as TracingRegistry, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::config::NatsConfig;
use crate::observability;

/// Configuration for worker initialization
pub struct WorkerConfig {
    pub service_name: String,
    pub stream_name: String,
    pub consumer_config: Config,
    pub max_concurrent_jobs: usize,
}

/// Initialize OpenTelemetry for the worker
pub fn initialize_opentelemetry(service_name: &str) -> Result<()> {
    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .build();

    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    // Initialize trace exporter
    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&otlp_endpoint)
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
        .with_endpoint(&otlp_endpoint)
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
        .with_endpoint(&otlp_endpoint)
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
    let use_json = std::env::var("LOG_FORMAT")
        .unwrap_or_else(|_| "json".to_string())
        .to_lowercase()
        == "json";

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

/// Run the worker message processing loop
pub async fn run_worker<J, C, F, Fut>(
    config: WorkerConfig,
    context: C,
    process_job: F,
) -> Result<()>
where
    J: DeserializeOwned + Send + 'static,
    C: Clone + Send + 'static,
    F: Fn(J, C) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    // Initialize NATS connection
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let nats_client = async_nats::connect(&nats_url).await?;

    let semaphore = Arc::new(Semaphore::new(config.max_concurrent_jobs));

    info!("Using JetStream mode for reliable message delivery");

    // Initialize NATS configuration
    let nats_config = NatsConfig::from_env()?;
    crate::nats::initialize_jetstream(&nats_client, &nats_config).await?;

    // Start NATS metrics collector
    crate::nats::start_metrics_collector(nats_client.clone()).await?;

    let jetstream = async_nats::jetstream::new(nats_client.clone());
    let consumer =
        crate::nats::ensure_consumer(&jetstream, &config.stream_name, config.consumer_config)
            .await?;

    info!(
        "Worker started with JetStream, listening on {} stream",
        config.stream_name
    );

    process_messages(consumer, context, semaphore, process_job).await
}

/// Process messages from the consumer
async fn process_messages<J, C, F, Fut>(
    consumer: Consumer<Config>,
    context: C,
    semaphore: Arc<Semaphore>,
    process_job: F,
) -> Result<()>
where
    J: DeserializeOwned + Send + 'static,
    C: Clone + Send + 'static,
    F: Fn(J, C) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    let process_job = Arc::new(process_job);
    let mut messages = consumer.messages().await?;

    while let Some(msg) = messages.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to receive message: {}", e);
                continue;
            }
        };

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

        // Acquire semaphore permit for backpressure
        let permit = match semaphore.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                warn!(
                    "Semaphore limit reached, workers are at capacity. Message will be redelivered."
                );
                // Don't acknowledge - let message be redelivered after ack_wait timeout
                continue;
            }
        };

        let ctx = context.clone();
        let process_job = Arc::clone(&process_job);
        let stream_name = consumer.cached_info().name.clone();

        tokio::spawn(async move {
            let _permit = permit; // Hold permit until task completes

            match process_job(job, ctx).await {
                Ok(_) => {
                    // Acknowledge success
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge successful job: {}", e);
                    }
                }
                Err(e) => {
                    let error_type = format!("{:?}", e)
                        .split(':')
                        .next()
                        .unwrap_or("Unknown")
                        .to_string();
                    crate::observability::record_worker_job_failure(&stream_name, &error_type);

                    error!("Job failed: {}", e);
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
                        crate::observability::record_worker_job_retry(&stream_name, 1);
                    }
                }
            }
        });
    }

    Ok(())
}
