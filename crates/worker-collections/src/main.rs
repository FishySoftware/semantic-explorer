use anyhow::Result;
use semantic_explorer_core::{
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

    // Initialize OpenTelemetry and tracing
    worker::initialize_opentelemetry(&service_name)?;

    // Initialize S3 client
    let s3_client = initialize_client().await?;

    // Initialize NATS client with auto-reconnect
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let nats_client = connect_with_retry(&nats_url).await?;

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

    let config = worker::WorkerConfig {
        service_name,
        stream_name: "COLLECTION_TRANSFORMS".to_string(),
        consumer_config: semantic_explorer_core::nats::create_transform_file_consumer_config(),
        max_concurrent_jobs,
        max_deliver: 5, // Matches consumer config
    };

    worker::run_worker(config, context, job::process_file_job).await
}
