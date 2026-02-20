use anyhow::Result;
use semantic_explorer_core::{
    config::{EmbeddingInferenceConfig, NatsConfig},
    nats::connect_with_retry,
    storage::initialize_client,
    worker::{self, WorkerContext},
};

mod chunk;
mod extract;
mod job;

#[tokio::main]
async fn main() -> Result<()> {
    let service_name =
        std::env::var("SERVICE_NAME").unwrap_or_else(|_| "worker-collections".to_string());

    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "json".to_string());

    // Initialize OpenTelemetry and tracing
    worker::initialize_opentelemetry(&service_name, &otlp_endpoint, &log_format)?;

    // Initialize S3 client
    let s3_client = initialize_client().await?;

    // Load NATS config from env at startup
    let nats_config = NatsConfig::from_env()?;
    let nats_client = connect_with_retry(&nats_config.url).await?;

    // Initialize embedding client config from centralized config
    let embedding_config = EmbeddingInferenceConfig::from_env()?;
    semantic_explorer_core::embedder::init_embedder(
        &embedding_config.url,
        embedding_config.max_concurrent_requests,
    );

    // Create worker context
    let context = WorkerContext {
        s3_client,
        nats_client: nats_client.clone(),
    };

    // Configure and run worker
    let max_concurrent_jobs = std::env::var("MAX_CONCURRENT_JOBS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<usize>()
        .unwrap_or(10);

    let health_check_port: u16 = std::env::var("HEALTH_CHECK_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8082);

    let config = worker::WorkerConfig {
        service_name,
        stream_name: "COLLECTION_TRANSFORMS".to_string(),
        consumer_config: semantic_explorer_core::nats::create_transform_file_consumer_config(),
        max_concurrent_jobs,
        max_deliver: 5, // Matches consumer config
        health_check_port,
        nats_config,
    };

    worker::run_worker(config, context, job::process_file_job).await
}
