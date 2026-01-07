use crate::job::WorkerContext;
use anyhow::Result;
use async_nats::jetstream::new;
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
use semantic_explorer_core::models::DatasetTransformJob;
use semantic_explorer_core::observability;
use semantic_explorer_core::storage::initialize_client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use tracing_subscriber::{
    EnvFilter, Layer, Registry as TracingRegistry, layer::SubscriberExt, util::SubscriberInitExt,
};

mod embedder;
mod job;

#[tokio::main]
async fn main() -> Result<()> {
    let service_name =
        std::env::var("SERVICE_NAME").unwrap_or_else(|_| "worker-datasets".to_string());

    let resource = Resource::builder()
        .with_service_name(service_name.clone())
        .build();

    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

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
    } else {
        info!("Failed to initialize OpenTelemetry metrics exporter");
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    observability::init_metrics_otel()?;

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to initialize tracing filter layer");

    let format_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true)
        .boxed();

    let tracer = global::tracer_provider().tracer(service_name);
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

    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let nats_client = async_nats::connect(&nats_url).await?;

    let s3_client = initialize_client().await?;

    let context = WorkerContext {
        s3_client,
        nats_client: nats_client.clone(),
    };

    let use_jetstream = std::env::var("NATS_USE_JETSTREAM")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let semaphore = Arc::new(Semaphore::new(100));

    if use_jetstream {
        info!("Using JetStream mode for reliable message delivery");

        semantic_explorer_core::nats::initialize_jetstream(&nats_client).await?;

        let jetstream = new(nats_client.clone());
        let consumer_config = semantic_explorer_core::nats::create_vector_embed_consumer_config();

        let consumer = semantic_explorer_core::nats::ensure_consumer(
            &jetstream,
            "VECTOR_EMBED",
            consumer_config,
        )
        .await?;

        info!("Worker started with JetStream, listening on VECTOR_EMBED stream");

        let mut messages = consumer.messages().await?;

        while let Some(msg) = messages.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to receive message: {}", e);
                    continue;
                }
            };

            let job: DatasetTransformJob = match serde_json::from_slice(&msg.payload) {
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
            tokio::spawn(async move {
                let _permit = permit; // Hold permit until task completes

                match job::process_vector_job(job, ctx).await {
                    Ok(_) => {
                        // Acknowledge success
                        if let Err(e) = msg.ack().await {
                            error!("Failed to acknowledge successful job: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Job failed: {}", e);
                        // Negative acknowledgment for retry (30s delay)
                        if let Err(ack_err) = msg
                            .ack_with(async_nats::jetstream::AckKind::Nak(Some(
                                Duration::from_secs(30),
                            )))
                            .await
                        {
                            error!("Failed to negatively acknowledge failed job: {}", ack_err);
                        }
                    }
                }
            });
        }
    } else {
        info!("Using legacy pub/sub mode (consider migrating to JetStream)");

        let mut subscriber = nats_client
            .subscribe("workers.dataset-transform".to_string())
            .await?;

        info!("Worker started, listening on workers.dataset-transform");

        while let Some(msg) = subscriber.next().await {
            let job: DatasetTransformJob = match serde_json::from_slice(&msg.payload) {
                Ok(j) => j,
                Err(e) => {
                    error!("Failed to deserialize job: {}", e);
                    continue;
                }
            };

            // Acquire semaphore permit for backpressure even in legacy mode
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(p) => p,
                Err(_) => {
                    warn!(
                        "Semaphore limit reached, workers are at capacity. Dropping message (legacy mode)."
                    );
                    continue;
                }
            };

            let ctx = context.clone();
            tokio::spawn(async move {
                let _permit = permit; // Hold permit until task completes

                if let Err(e) = job::process_vector_job(job, ctx).await {
                    error!("Job failed: {}", e);
                }
            });
        }
    }

    Ok(())
}
