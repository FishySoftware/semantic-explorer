use crate::job::WorkerContext;
use anyhow::Result;
use semantic_explorer_core::{storage::initialize_client, worker};

mod embedder;
mod job;

#[tokio::main]
async fn main() -> Result<()> {
    let service_name =
        std::env::var("SERVICE_NAME").unwrap_or_else(|_| "worker-datasets".to_string());

    // Initialize OpenTelemetry and tracing
    worker::initialize_opentelemetry(&service_name)?;

    // Initialize S3 client
    let s3_client = initialize_client().await?;

    // Create worker context
    let context = WorkerContext {
        s3_client,
        nats_client: async_nats::connect(
            &std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string()),
        )
        .await?,
    };

    // Configure and run worker
    let config = worker::WorkerConfig {
        service_name,
        stream_name: "DATASET_TRANSFORMS".to_string(),
        consumer_config: semantic_explorer_core::nats::create_vector_embed_consumer_config(),
        max_concurrent_jobs: 100,
    };

    worker::run_worker(config, context, job::process_vector_job).await
}
