use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use semantic_explorer_core::jobs::{DatasetTransformJob, EmbedderConfig, VectorDatabaseConfig};
use semantic_explorer_core::storage::{DocumentUpload, ensure_bucket_exists, upload_document};

use crate::storage::postgres::dataset_transforms::get_active_dataset_transforms;
use crate::storage::postgres::datasets;
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
        let mut interval = interval(Duration::from_secs(10)); // Check for new dataset transform jobs every 10 seconds
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
    let embedded_datasets_list = embedded_datasets::get_embedded_datasets_for_transform(
        pool,
        transform.dataset_transform_id,
    )
    .await?;

    info!(
        "Found {} embedded datasets for dataset transform {}",
        embedded_datasets_list.len(),
        transform.dataset_transform_id
    );

    // Get vector database config from environment
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let vector_db_config = VectorDatabaseConfig {
        database_type: "qdrant".to_string(),
        connection_url: qdrant_url,
        api_key: std::env::var("QDRANT_API_KEY").ok(),
    };

    // Get batch size from job config
    let configured_batch_size = transform
        .job_config
        .get("embedding_batch_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(100);

    let wipe_collection = transform
        .job_config
        .get("wipe_collection")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let embedded_datasets_count = embedded_datasets_list.len();
    let mut total_jobs = 0;

    for embedded_dataset in embedded_datasets_list {
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

        // Use the minimum of configured batch size and embedder's max_batch_size
        let embedding_batch_size = configured_batch_size.min(embedder.max_batch_size as usize);

        // The bucket name is derived from the Qdrant collection name (lowercase)
        let bucket = embedded_dataset.collection_name.to_lowercase();

        // Ensure the S3 bucket exists
        if let Err(e) = ensure_bucket_exists(s3, &bucket).await {
            error!(
                "Failed to ensure bucket exists for embedded dataset {}: {}. Skipping.",
                embedded_dataset.embedded_dataset_id, e
            );
            continue;
        }

        // Get processed batches for this embedded dataset
        let processed_batches =
            embedded_datasets::get_processed_batches(pool, embedded_dataset.embedded_dataset_id)
                .await?;
        let processed_keys: HashSet<String> =
            processed_batches.into_iter().map(|b| b.file_key).collect();

        // List existing batch files in S3
        let mut existing_batch_files = HashSet::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let files =
                match rustfs::list_files(s3, &bucket, 100, continuation_token.as_deref()).await {
                    Ok(files) => files,
                    Err(e) => {
                        error!(
                            "Failed to list files in bucket '{}': {}. Will create new batches.",
                            bucket, e
                        );
                        break;
                    }
                };
            if files.files.is_empty() {
                break;
            }

            for file in files.files {
                if file.key.starts_with("batches/") {
                    existing_batch_files.insert(file.key);
                }
            }

            if !files.has_more {
                break;
            }
            continuation_token = files.continuation_token;
        }

        // Calculate unprocessed existing batches
        let unprocessed_existing: Vec<String> = existing_batch_files
            .iter()
            .filter(|k| !processed_keys.contains(*k))
            .cloned()
            .collect();

        info!(
            "Embedded dataset {} (embedder: {}) has {} batch files, {} already processed, {} pending",
            embedded_dataset.embedded_dataset_id,
            embedder.name,
            existing_batch_files.len(),
            processed_keys.len(),
            unprocessed_existing.len()
        );

        // If there are unprocessed batches, dispatch jobs for them
        for batch_file_key in &unprocessed_existing {
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
                batch_size: Some(embedding_batch_size),
            };

            let payload = serde_json::to_vec(&job)?;
            nats.publish("workers.dataset-transform".to_string(), payload.into())
                .await?;
            total_jobs += 1;
        }

        // If all existing batches are processed (or none exist), check if we need to create new batches
        if unprocessed_existing.is_empty() {
            // Get dataset items that haven't been batched yet
            let items_created = create_batches_from_dataset_items(
                pool,
                s3,
                nats,
                transform,
                &embedded_dataset,
                &embedder_config,
                &vector_db_config,
                &bucket,
                &processed_keys,
                embedding_batch_size,
                wipe_collection,
            )
            .await?;
            total_jobs += items_created;
        }
    }

    info!(
        "Dataset transform scan finished for {}. Created {} jobs across {} embedded datasets.",
        transform.dataset_transform_id, total_jobs, embedded_datasets_count
    );

    Ok(())
}

/// Create batch files from dataset items and dispatch jobs
async fn create_batches_from_dataset_items(
    pool: &Pool<Postgres>,
    s3: &S3Client,
    nats: &NatsClient,
    transform: &crate::transforms::dataset::DatasetTransform,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    embedder_config: &EmbedderConfig,
    vector_db_config: &VectorDatabaseConfig,
    bucket: &str,
    processed_keys: &HashSet<String>,
    embedding_batch_size: usize,
    wipe_collection: bool,
) -> Result<usize> {
    // Get the total count of items in the source dataset
    let total_items = datasets::count_dataset_items(pool, transform.source_dataset_id).await?;

    if total_items == 0 {
        info!(
            "No items in source dataset {} for embedded dataset {}",
            transform.source_dataset_id, embedded_dataset.embedded_dataset_id
        );
        return Ok(0);
    }

    // Count already processed chunks (from processed batch file names)
    // We track progress by counting processed batches
    let batches_processed = processed_keys.len();

    // If we've processed some batches, assume we're done unless new items were added
    // This is a simple approach - in production you might want more sophisticated tracking
    if batches_processed > 0 {
        info!(
            "Embedded dataset {} already has {} processed batches, skipping batch creation",
            embedded_dataset.embedded_dataset_id, batches_processed
        );
        return Ok(0);
    }

    info!(
        "Creating batches for embedded dataset {} from {} dataset items",
        embedded_dataset.embedded_dataset_id, total_items
    );

    // Fetch all dataset items and create batches
    let mut all_batch_items: Vec<serde_json::Value> = Vec::new();
    let page_size = 100i64;
    let mut page = 0i64;

    loop {
        let items =
            datasets::get_dataset_items(pool, transform.source_dataset_id, page, page_size).await?;

        if items.is_empty() {
            break;
        }

        // Convert dataset items to batch items (one per chunk)
        for item in &items {
            for (chunk_idx, chunk) in item.chunks.iter().enumerate() {
                // Generate a unique UUID for each chunk
                // Note: This is not deterministic, but Qdrant requires valid UUIDs as point IDs
                let chunk_uuid = Uuid::new_v4();
                let batch_item = serde_json::json!({
                    "id": chunk_uuid.to_string(),
                    "text": chunk.content,
                    "payload": {
                        "item_id": item.item_id,
                        "item_title": item.title,
                        "chunk_index": chunk_idx,
                        "chunk_metadata": chunk.metadata,
                        "item_metadata": item.metadata
                    }
                });
                all_batch_items.push(batch_item);
            }
        }

        page += 1;
        if items.len() < page_size as usize {
            break;
        }
    }

    if all_batch_items.is_empty() {
        info!(
            "No chunks found in dataset items for embedded dataset {}",
            embedded_dataset.embedded_dataset_id
        );
        return Ok(0);
    }

    info!(
        "Found {} total chunks to embed for embedded dataset {}",
        all_batch_items.len(),
        embedded_dataset.embedded_dataset_id
    );

    // Split into batches and upload to S3, then dispatch jobs
    let mut jobs_created = 0;
    let chunks_per_batch = embedding_batch_size * 10; // Create larger batches for efficiency

    for (batch_idx, batch_chunk) in all_batch_items.chunks(chunks_per_batch).enumerate() {
        let batch_key = format!("batches/batch-{}-{}.json", batch_idx, Uuid::new_v4());
        let batch_json = serde_json::to_vec(batch_chunk)?;

        // Upload batch to S3
        if let Err(e) = upload_document(
            s3,
            DocumentUpload {
                collection_id: bucket.to_string(),
                name: batch_key.clone(),
                content: batch_json,
                mime_type: "application/json".to_string(),
            },
        )
        .await
        {
            error!(
                "Failed to upload batch {} to bucket '{}': {}",
                batch_key, bucket, e
            );
            continue;
        }

        info!(
            "Uploaded batch {} with {} chunks for embedded dataset {}",
            batch_key,
            batch_chunk.len(),
            embedded_dataset.embedded_dataset_id
        );

        // Dispatch job for this batch
        let job = DatasetTransformJob {
            job_id: Uuid::new_v4(),
            batch_file_key: batch_key,
            bucket: bucket.to_string(),
            dataset_transform_id: transform.dataset_transform_id,
            embedded_dataset_id: embedded_dataset.embedded_dataset_id,
            embedder_config: embedder_config.clone(),
            vector_database_config: vector_db_config.clone(),
            collection_name: embedded_dataset.collection_name.clone(),
            wipe_collection: wipe_collection && batch_idx == 0, // Only wipe on first batch
            batch_size: Some(embedding_batch_size),
        };

        let payload = serde_json::to_vec(&job)?;
        nats.publish("workers.dataset-transform".to_string(), payload.into())
            .await?;
        jobs_created += 1;
    }

    info!(
        "Created {} batch jobs for embedded dataset {}",
        jobs_created, embedded_dataset.embedded_dataset_id
    );

    Ok(jobs_created)
}
