use crate::job::WorkerContext;
use anyhow::Result;
use semantic_explorer_core::{storage::initialize_client, worker};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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

    // Initialize NATS client
    let nats_client = async_nats::connect(
        &std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string()),
    )
    .await?;

    // Create worker context with Qdrant client cache
    let context = WorkerContext {
        s3_client,
        nats_client: nats_client.clone(),
        qdrant_cache: Arc::new(Mutex::new(HashMap::new())),
    };

    // Configure and run worker
    let max_concurrent_jobs = std::env::var("MAX_CONCURRENT_JOBS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<usize>()
        .unwrap_or(10);

    let config = worker::WorkerConfig {
        service_name,
        stream_name: "DATASET_TRANSFORMS".to_string(),
        consumer_config: semantic_explorer_core::nats::create_vector_embed_consumer_config(),
        max_concurrent_jobs,
        nats_client: Some(nats_client),
    };

    worker::run_worker(config, context, job::process_vector_job).await
}
