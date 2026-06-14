use anyhow::Result;
use semantic_explorer_core::config::{EmbeddingInferenceConfig, NatsConfig, QdrantCacheConfig};
use semantic_explorer_core::nats::connect_with_retry;
use semantic_explorer_core::worker::WorkerContext;
use semantic_explorer_core::{storage::initialize_client, worker};

mod job;
mod qdrant_cache;

#[tokio::main]
async fn main() -> Result<()> {
    let service_name =
        std::env::var("SERVICE_NAME").unwrap_or_else(|_| "worker-datasets".to_string());

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

    // Initialize job-level config from env at startup
    let qdrant_parallel_uploads: usize = std::env::var("QDRANT_PARALLEL_UPLOADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let upsert_max_attempts: u32 = std::env::var("QDRANT_UPSERT_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(job::DEFAULT_UPSERT_MAX_ATTEMPTS);
    job::init_job_config(qdrant_parallel_uploads, upsert_max_attempts);

    // Initialize Qdrant cache sizing from centralized config at startup
    qdrant_cache::init_cache_config(QdrantCacheConfig::from_env()?);

    // Create worker context with Qdrant client cache
    let context = WorkerContext {
        s3_client,
        nats_client: nats_client.clone(),
    };

    // Configure and run worker
    const DEFAULT_MAX_CONCURRENT_JOBS: usize = 10;
    let max_concurrent_jobs = match std::env::var("MAX_CONCURRENT_JOBS") {
        Ok(value) => value.parse::<usize>().unwrap_or_else(|_| {
            tracing::warn!(
                value = %value,
                default = DEFAULT_MAX_CONCURRENT_JOBS,
                "Invalid MAX_CONCURRENT_JOBS, using default"
            );
            DEFAULT_MAX_CONCURRENT_JOBS
        }),
        Err(_) => DEFAULT_MAX_CONCURRENT_JOBS,
    };

    let health_check_port: u16 = std::env::var("HEALTH_CHECK_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8083);

    let config = worker::WorkerConfig {
        service_name,
        stream_name: "DATASET_TRANSFORMS".to_string(),
        consumer_config: semantic_explorer_core::nats::create_dataset_transform_consumer_config(),
        max_concurrent_jobs,
        max_deliver: 5, // Matches consumer config
        health_check_port,
        nats_config,
    };

    worker::run_worker(config, context, job::process_dataset_transform_job).await
}
