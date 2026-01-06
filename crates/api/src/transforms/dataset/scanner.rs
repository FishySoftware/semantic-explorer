use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use semantic_explorer_core::jobs::{DatasetTransformJob, EmbedderConfig, VectorDatabaseConfig};

use crate::storage::postgres::dataset_transforms::get_active_dataset_transforms;
use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::embedders;
use crate::storage::rustfs;

/// Initialize the background scanner for dataset transforms
pub(crate) fn initialize_scanner(
    postgres_pool: Pool<Postgres>,
    nats_client: NatsClient,
    s3_client: S3Client,
) -> JoinHandle<()> {
    spawn(async move {
        let mut interval = interval(Duration::from_secs(90)); // Slightly offset from collection scanner
        loop {
            interval.tick().await;
            if let Err(e) =
                scan_active_dataset_transforms(&postgres_pool, &nats_client, &s3_client).await
            {
                error!("Error scanning dataset transforms: {}", e);
            }
        }
    })
}

#[tracing::instrument(name = "scan_active_dataset_transforms", skip_all)]
async fn scan_active_dataset_transforms(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
) -> Result<()> {
    let transforms = get_active_dataset_transforms(pool).await?;
    info!("Scanning {} active dataset transforms", transforms.len());

    for transform in transforms {
        if let Err(e) = process_dataset_transform_scan(pool, nats, s3, &transform).await {
            error!(
                "Failed to process dataset transform scan for {}: {}",
                transform.dataset_transform_id, e
            );
        }
    }
    Ok(())
}

#[tracing::instrument(
    name = "process_dataset_transform_scan",
    skip(pool, nats, s3, transform),
    fields(dataset_transform_id = %transform.dataset_transform_id, embedder_count = %transform.embedder_ids.len())
)]
async fn process_dataset_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    transform: &crate::transforms::dataset::DatasetTransform,
) -> Result<()> {
    info!(
        "Starting dataset transform scan for {} with {} embedders",
        transform.dataset_transform_id,
        transform.embedder_ids.len()
    );

    // Get all embedded datasets for this transform
    let embedded_datasets = embedded_datasets::get_embedded_datasets_for_transform(
        pool,
        transform.dataset_transform_id,
    )
    .await?;

    info!(
        "Found {} embedded datasets for dataset transform {}",
        embedded_datasets.len(),
        transform.dataset_transform_id
    );

    // Get vector database config from environment
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let vector_db_config = VectorDatabaseConfig {
        database_type: "qdrant".to_string(),
        connection_url: qdrant_url,
        api_key: None,
    };

    // Get batch size from job config
    let batch_size = transform
        .job_config
        .get("embedding_batch_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let wipe_collection = transform
        .job_config
        .get("wipe_collection")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // For each embedded dataset (one per embedder), create jobs for batch files
    // Note: The batch files are stored in a bucket named after the Qdrant collection (lowercase)
    let embedded_datasets_count = embedded_datasets.len();
    let mut total_jobs = 0;
    for embedded_dataset in embedded_datasets {
        // Get embedder configuration
        let embedder =
            embedders::get_embedder(pool, &transform.owner, embedded_dataset.embedder_id).await?;

        let embedder_config = EmbedderConfig {
            provider: embedder.provider.clone(),
            base_url: embedder.base_url.clone(),
            api_key: embedder.api_key.clone(),
            model: embedder
                .config
                .get("model")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string()),
            config: embedder.config.clone(),
            max_batch_size: embedder.max_batch_size,
        };

        // The bucket name is derived from the Qdrant collection name (lowercase)
        let bucket = embedded_dataset.collection_name.to_lowercase();

        // List all batch files in this embedded dataset's bucket
        let mut continuation_token: Option<String> = None;
        let mut batch_files = Vec::new();

        loop {
            let files = rustfs::list_files(s3, &bucket, 100, continuation_token.as_deref()).await?;
            if files.files.is_empty() {
                break;
            }

            for file in files.files {
                if file.key.starts_with("batches/") {
                    batch_files.push(file.key);
                }
            }

            if !files.has_more {
                break;
            }
            continuation_token = files.continuation_token;
        }

        // Get processed batches for this embedded dataset
        let processed_batches =
            embedded_datasets::get_processed_batches(pool, embedded_dataset.embedded_dataset_id)
                .await?;
        let processed_keys: std::collections::HashSet<String> =
            processed_batches.into_iter().map(|b| b.file_key).collect();

        info!(
            "Embedded dataset {} (embedder: {}) has {} batch files, {} already processed",
            embedded_dataset.embedded_dataset_id,
            embedder.name,
            batch_files.len(),
            processed_keys.len()
        );

        // Create jobs for unprocessed batch files
        for batch_file_key in &batch_files {
            if !processed_keys.contains(batch_file_key) {
                let job = DatasetTransformJob {
                    job_id: Uuid::new_v4(),
                    batch_file_key: batch_file_key.clone(),
                    bucket: bucket.clone(),
                    dataset_transform_id: transform.dataset_transform_id,
                    embedded_dataset_id: embedded_dataset.embedded_dataset_id,
                    embedder_config: embedder_config.clone(),
                    vector_database_config: vector_db_config.clone(),
                    collection_name: embedded_dataset.collection_name.clone(),
                    wipe_collection,
                    batch_size,
                };

                let payload = serde_json::to_vec(&job)?;
                nats.publish("workers.dataset-transform".to_string(), payload.into())
                    .await?;
                total_jobs += 1;
            }
        }
    }

    info!(
        "Dataset transform scan finished for {}. Created {} jobs across {} embedded datasets.",
        transform.dataset_transform_id, total_jobs, embedded_datasets_count
    );

    Ok(())
}
