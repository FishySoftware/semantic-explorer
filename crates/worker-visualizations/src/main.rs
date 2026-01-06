/// Visualization Worker with NATS Integration
///
/// This worker consumes visualization transform jobs from NATS and processes them:
/// 1. Fetches document vectors from source Qdrant collection
/// 2. Reduces dimensionality using UMAP
/// 3. Identifies topic clusters using HDBSCAN
/// 4. Generates topic labels using TF-IDF
/// 5. Exports reduced vectors to `-reduced` collection
/// 6. Exports topic centroids to `-reduced-topics` collection
///
/// Environment Variables:
/// - NATS_URL: NATS server URL (default: nats://localhost:4222)
/// - OTEL_EXPORTER_OTLP_ENDPOINT: OpenTelemetry collector endpoint
mod job;
mod topic_naming;

use async_nats::jetstream::{self, consumer::pull::MessagesErrorKind};
use futures_util::StreamExt;
use opentelemetry::{trace::TracerProvider, KeyValue};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{
    trace::{SdkTracerProvider, Tracer},
    Resource,
};
use semantic_explorer_core::jobs::VisualizationTransformJob;
use semantic_explorer_core::nats::{create_visualization_consumer_config, ensure_consumer};
use std::env;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_tracing() -> Result<Tracer, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = SpanExporter::builder().with_tonic().build()?;

    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", "worker-visualizations"))
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    let tracer = provider.tracer("worker-visualizations");

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer.clone());

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry_layer)
        .init();

    Ok(tracer)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = init_tracing() {
        eprintln!("Failed to initialize tracing: {}", e);
    }

    info!("Starting visualization worker");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    info!("Connecting to NATS at {}", nats_url);

    let nats_client = async_nats::connect(&nats_url).await?;
    info!("Connected to NATS");

    semantic_explorer_core::nats::initialize_jetstream(&nats_client).await?;

    let jetstream = jetstream::new(nats_client.clone());

    let consumer_config = create_visualization_consumer_config();
    let consumer = ensure_consumer(&jetstream, "VISUALIZATION_TRANSFORMS", consumer_config).await?;

    info!("Worker started with JetStream, listening on VISUALIZATION_TRANSFORMS stream");

    let mut messages = consumer.messages().await?;

    while let Some(msg) = messages.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                match e.kind() {
                    MessagesErrorKind::MissingHeartbeat => {
                        warn!("Missed heartbeat, reconnecting...");
                    }
                    _ => {
                        error!("Failed to receive message: {}", e);
                    }
                }
                continue;
            }
        };

        let job: VisualizationTransformJob = match serde_json::from_slice(&msg.payload) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to deserialize job: {}", e);
                if let Err(ack_err) = msg.ack().await {
                    error!("Failed to acknowledge bad message: {}", ack_err);
                }
                continue;
            }
        };

        info!("Received visualization job: {}", job.job_id);

        let start = Instant::now();
        match job::process_visualization_job(job.clone(), &nats_client).await {
            Ok((n_points, n_clusters, duration_ms)) => {
                info!(
                    "Successfully processed job {} in {}ms ({} points, {} clusters)",
                    job.job_id, duration_ms, n_points, n_clusters
                );
                if let Err(e) = msg.ack().await {
                    error!("Failed to acknowledge successful job: {}", e);
                }
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as i64;
                error!(
                    "Failed to process job {} after {}ms: {}",
                    job.job_id, duration_ms, e
                );

                let result_msg = semantic_explorer_core::jobs::VisualizationTransformResult {
                    job_id: job.job_id,
                    visualization_transform_id: job.visualization_transform_id,
                    status: "failed".to_string(),
                    error: Some(e.to_string()),
                    processing_duration_ms: Some(duration_ms),
                    n_points: 0,
                    n_clusters: 0,
                    output_collection_reduced: job.output_collection_reduced.clone(),
                    output_collection_topics: job.output_collection_topics.clone(),
                };
                if let Ok(payload) = serde_json::to_vec(&result_msg) {
                    let _ = nats_client
                        .publish("worker.result.visualization".to_string(), payload.into())
                        .await;
                }

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
    }

    info!("Worker shutting down");
    Ok(())
}
