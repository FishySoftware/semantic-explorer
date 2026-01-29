use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use tracing::{error, info, warn};
use uuid::Uuid;

use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::{DatasetTransformJob, EmbedderConfig, QdrantConnectionConfig};
use semantic_explorer_core::observability::record_scanner_items_discovered;
use semantic_explorer_core::storage::{DocumentUpload, upload_document};

use crate::auth::AuthenticatedUser;
use crate::embedded_datasets::EmbeddedDataset;
use crate::storage::postgres::dataset_transform_pending_batches::{
    self as pending_batches, CreatePendingBatch,
};
use crate::storage::postgres::dataset_transforms::{
    get_active_dataset_transforms_privileged, get_dataset_transform_privileged,
};
use crate::storage::postgres::datasets;
use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::embedders;
use crate::storage::s3;
use crate::transforms::dataset::models::DatasetTransform;

/// Configuration for batch processing of dataset items
#[derive(Debug, Clone)]
struct DatasetBatchConfig {
    embedder_config: EmbedderConfig,
    qdrant_config: QdrantConnectionConfig,
    s3_bucket: String,
    embedded_dataset_prefix: String,
    embedding_batch_size: usize,
}

#[tracing::instrument(name = "scan_active_dataset_transforms", skip_all)]
pub(crate) async fn scan_active_dataset_transforms(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    encryption: &EncryptionService,
    qdrant_config: &QdrantConnectionConfig,
) -> Result<()> {
    let transforms = get_active_dataset_transforms_privileged(pool).await?;
    info!("Scanning {} active dataset transforms", transforms.len());

    for transform in transforms {
        if let Err(e) = process_dataset_transform_scan(
            pool,
            nats,
            s3,
            s3_bucket_name,
            &transform,
            encryption,
            qdrant_config,
        )
        .await
        {
            error!(
                "Failed to process dataset transform scan for {}: {}",
                transform.dataset_transform_id, e
            );
        }
    }
    Ok(())
}

/// Scan a specific dataset transform by ID (privileged, for NATS triggers)
#[tracing::instrument(
    name = "scan_dataset_transform",
    skip(pool, nats, s3, encryption, qdrant_config),
    fields(dataset_transform_id = %dataset_transform_id)
)]
pub(crate) async fn scan_dataset_transform(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    dataset_transform_id: i32,
    encryption: &EncryptionService,
    qdrant_config: &QdrantConnectionConfig,
) -> Result<()> {
    let transform = get_dataset_transform_privileged(pool, dataset_transform_id).await?;

    if !transform.is_enabled {
        info!(
            "Dataset transform {} is disabled, skipping scan",
            dataset_transform_id
        );
        return Ok(());
    }

    process_dataset_transform_scan(
        pool,
        nats,
        s3,
        s3_bucket_name,
        &transform,
        encryption,
        qdrant_config,
    )
    .await
}

#[tracing::instrument(
    name = "process_dataset_transform_scan",
    skip(pool, nats, s3, transform, encryption, qdrant_config),
    fields(dataset_transform_id = %transform.dataset_transform_id, embedder_count = %transform.embedder_ids.len())
)]
async fn process_dataset_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    transform: &DatasetTransform,
    encryption: &EncryptionService,
    qdrant_config: &QdrantConnectionConfig,
) -> Result<()> {
    info!(
        "Starting dataset transform scan for {} with {} embedders",
        transform.dataset_transform_id,
        transform.embedder_ids.len()
    );

    // Refresh total_chunks_to_process in case source dataset has changed
    if let Err(e) = crate::storage::postgres::dataset_transform_stats::refresh_total_chunks(
        pool,
        &transform.owner_id,
        transform.dataset_transform_id,
    )
    .await
    {
        error!(
            "Failed to refresh total chunks for dataset transform {}: {}",
            transform.dataset_transform_id, e
        );
        // Continue processing even if refresh fails - stats will be stale but transforms still work
    }

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

    let embedded_datasets_count = embedded_datasets_list.len();
    let mut total_jobs = 0;

    let embedder_ids: Vec<i32> = embedded_datasets_list
        .iter()
        .map(|ed| ed.embedder_id)
        .collect();
    let user = AuthenticatedUser(transform.owner_display_name.clone());
    let embedders_list =
        embedders::get_embedders_batch(pool, &user, &embedder_ids, encryption).await?;

    let embedders_map: std::collections::HashMap<i32, _> = embedders_list
        .into_iter()
        .map(|embedder| (embedder.embedder_id, embedder))
        .collect();

    for embedded_dataset in embedded_datasets_list {
        // Get embedder from pre-fetched map
        let embedder = embedders_map
            .get(&embedded_dataset.embedder_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Embedder {} not found in batch fetch",
                    embedded_dataset.embedder_id
                )
            })?;

        let model = embedder
            .config
            .get("model")
            .and_then(|m| m.as_str())
            .ok_or_else(|| anyhow::anyhow!("Embedder config must specify a 'model' field"))?
            .to_string();

        let embedder_config = EmbedderConfig::new(
            embedder.provider.clone(),
            embedder.base_url.clone(),
            embedder.api_key.clone(),
            model,
            embedder.config.clone(),
            embedder.batch_size,
            embedder.max_input_tokens,
        );

        // Use the embedder's configured batch size
        let embedding_batch_size = embedder.batch_size as usize;

        // Use single-bucket architecture with embedded-datasets prefix
        let s3_bucket = s3_bucket_name.to_string();
        let embedded_dataset_prefix = format!(
            "embedded-datasets/embedded-dataset-{}",
            embedded_dataset.embedded_dataset_id
        );

        // Note: No need to ensure bucket exists as it should already exist
        // The main bucket is created during infrastructure setup

        // Get processed batches for this embedded dataset
        let processed_batches =
            embedded_datasets::get_processed_batches(pool, embedded_dataset.embedded_dataset_id)
                .await?;
        let processed_keys: HashSet<String> = processed_batches
            .iter()
            .map(|b| b.file_key.clone())
            .collect();

        // List existing batch files in S3
        let mut existing_batch_files = HashSet::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let files = match s3::list_files(
                s3,
                &s3_bucket,
                &embedded_dataset_prefix,
                100,
                continuation_token.as_deref(),
            )
            .await
            {
                Ok(files) => files,
                Err(e) => {
                    error!(
                        "Failed to list files in bucket '{}': {}. Will create new batches.",
                        s3_bucket, e
                    );
                    break;
                }
            };
            if files.files.is_empty() {
                break;
            }

            for file in files.files {
                if file
                    .key
                    .starts_with(&format!("{}/batches/", embedded_dataset_prefix))
                {
                    existing_batch_files.insert(file.key);
                }
            }

            continuation_token = files.continuation_token;
            if continuation_token.is_none() {
                break;
            }
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
                bucket: s3_bucket.clone(),
                dataset_id: transform.source_dataset_id,
                dataset_transform_id: transform.dataset_transform_id,
                embedded_dataset_id: embedded_dataset.embedded_dataset_id,
                owner_id: transform.owner_id.clone(),
                embedder_config: embedder_config.clone(),
                qdrant_config: qdrant_config.clone(),
                collection_name: embedded_dataset.collection_name.clone(),
                batch_size: Some(embedding_batch_size),
            };

            let payload = serde_json::to_vec(&job)?;

            // Publish with retry (3 attempts with exponential backoff)
            let msg_id = format!("dt-{}-{}", transform.dataset_transform_id, batch_file_key);
            match semantic_explorer_core::nats::publish_with_retry(
                nats,
                "workers.dataset-transform",
                &msg_id,
                payload.clone(),
                3,
            )
            .await
            {
                semantic_explorer_core::nats::PublishResult::Published => {
                    total_jobs += 1;
                    // Track dispatched batch for accurate completion detection
                    // Note: We don't know exact chunk count for existing batches, use 0 as placeholder
                    if let Err(e) = crate::storage::postgres::dataset_transform_stats::increment_dispatched_batch(
                        pool,
                        transform.dataset_transform_id,
                        0, // Unknown chunk count for existing batches
                    ).await {
                        warn!("Failed to track dispatched batch: {}", e);
                    }
                }
                semantic_explorer_core::nats::PublishResult::Failed(e) => {
                    // Store in pending_batches for recovery
                    error!(
                        "Failed to publish job for batch {} after retries: {}. Saving for recovery.",
                        batch_file_key, e
                    );
                    if let Err(pe) = pending_batches::insert_pending_batch(
                        pool,
                        CreatePendingBatch {
                            batch_type: "dataset".to_string(),
                            dataset_transform_id: Some(transform.dataset_transform_id),
                            embedded_dataset_id: Some(embedded_dataset.embedded_dataset_id),
                            batch_key: batch_file_key.clone(),
                            s3_bucket: s3_bucket.clone(),
                            job_payload: serde_json::from_slice(&payload).unwrap_or_default(),
                        },
                    )
                    .await
                    {
                        error!("Failed to save pending batch for recovery: {}", pe);
                    }
                }
            }
        }

        // If all existing batches are processed (or none exist), check if we need to create new batches
        if unprocessed_existing.is_empty() {
            // Get dataset items that haven't been batched yet
            let batch_config = DatasetBatchConfig {
                embedder_config: embedder_config.clone(),
                qdrant_config: qdrant_config.clone(),
                s3_bucket: s3_bucket.clone(),
                embedded_dataset_prefix: embedded_dataset_prefix.clone(),
                embedding_batch_size,
            };
            let items_created = create_batches_from_dataset_items(
                pool,
                s3,
                nats,
                transform,
                &embedded_dataset,
                &batch_config,
            )
            .await?;
            total_jobs += items_created;
        }
    }

    info!(
        "Dataset transform scan finished for {}. Created {} jobs across {} embedded datasets.",
        transform.dataset_transform_id, total_jobs, embedded_datasets_count
    );

    if total_jobs > 0 {
        record_scanner_items_discovered("dataset", total_jobs as u64);
    }

    Ok(())
}

/// Create batch files from dataset items and dispatch jobs
async fn create_batches_from_dataset_items(
    pool: &Pool<Postgres>,
    s3: &S3Client,
    nats: &NatsClient,
    transform: &DatasetTransform,
    embedded_dataset: &EmbeddedDataset,
    config: &DatasetBatchConfig,
) -> Result<usize> {
    // Use timestamp-based tracking to identify new items that need processing
    let last_processed_at = embedded_dataset.last_processed_at;

    info!(
        "Embedded dataset {} last processed at: {:?}",
        embedded_dataset.embedded_dataset_id, last_processed_at
    );

    // Fetch only items that were modified since the last processing
    let items = datasets::get_dataset_items_modified_since(
        pool,
        transform.source_dataset_id,
        last_processed_at,
    )
    .await?;

    if items.is_empty() {
        info!(
            "Embedded dataset {} has no new items since last processing. Skipping.",
            embedded_dataset.embedded_dataset_id
        );
        return Ok(0);
    }

    // Capture the max updated_at timestamp from items we're about to process
    // This prevents race conditions where items created between query and watermark update are missed
    let max_item_timestamp = items.iter().filter_map(|item| item.updated_at).max();

    info!(
        "Embedded dataset {} found {} items with new/modified chunks, max_timestamp: {:?}",
        embedded_dataset.embedded_dataset_id,
        items.len(),
        max_item_timestamp
    );

    // Convert dataset items to batch items (one per chunk)
    let mut all_batch_items: Vec<serde_json::Value> = Vec::new();
    // Use a namespace UUID for generating deterministic chunk IDs
    // This ensures the same item+chunk always gets the same UUID, enabling idempotent upserts
    let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap(); // URL namespace UUID
    for item in &items {
        for (chunk_idx, chunk) in item.chunks.iter().enumerate() {
            // Generate a deterministic UUID based on embedded_dataset_id, item_id, and chunk_index
            // This allows re-processing to update existing vectors rather than create duplicates
            let chunk_id_string = format!(
                "ed-{}-item-{}-chunk-{}",
                embedded_dataset.embedded_dataset_id, item.item_id, chunk_idx
            );
            let chunk_uuid = Uuid::new_v5(&namespace, chunk_id_string.as_bytes());
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

    if all_batch_items.is_empty() {
        info!(
            "No chunks found in modified items for embedded dataset {}",
            embedded_dataset.embedded_dataset_id
        );
        return Ok(0);
    }

    info!(
        "Created {} batch items from {} modified dataset items",
        all_batch_items.len(),
        items.len()
    );

    // Split into batches and upload to S3, then dispatch jobs
    let mut jobs_created = 0;
    let chunks_per_batch = config.embedding_batch_size * 10; // Create larger batches for efficiency

    for (batch_idx, batch_chunk) in all_batch_items.chunks(chunks_per_batch).enumerate() {
        let batch_filename = format!("batch-{}-{}.json", batch_idx, Uuid::new_v4());
        let batch_key = format!(
            "{}/batches/{}",
            config.embedded_dataset_prefix, batch_filename
        );
        let batch_json = serde_json::to_vec(batch_chunk)?;

        // Upload batch to S3 using single-bucket architecture
        if let Err(e) = upload_document(
            s3,
            DocumentUpload {
                collection_id: config.s3_bucket.clone(),
                name: batch_key.clone(),
                content: batch_json,
                mime_type: "application/json".to_string(),
            },
        )
        .await
        {
            error!(
                "Failed to upload batch {} to s3://{}/{}: {}",
                batch_filename, config.s3_bucket, batch_key, e
            );
            continue;
        }

        info!(
            "Uploaded batch {} with {} chunks for embedded dataset {} (s3://{}/{}) ",
            batch_filename,
            batch_chunk.len(),
            embedded_dataset.embedded_dataset_id,
            config.s3_bucket,
            batch_key
        );

        // Dispatch job for this batch
        let job = DatasetTransformJob {
            job_id: Uuid::new_v4(),
            batch_file_key: batch_key.clone(),
            bucket: config.s3_bucket.clone(),
            dataset_id: transform.source_dataset_id,
            dataset_transform_id: transform.dataset_transform_id,
            embedded_dataset_id: embedded_dataset.embedded_dataset_id,
            owner_id: transform.owner_id.clone(),
            embedder_config: config.embedder_config.clone(),
            qdrant_config: config.qdrant_config.clone(),
            collection_name: embedded_dataset.collection_name.clone(),
            batch_size: Some(config.embedding_batch_size),
        };

        let payload = serde_json::to_vec(&job)?;
        let chunk_count = batch_chunk.len() as i64;

        // Publish with retry (3 attempts with exponential backoff)
        let msg_id = format!("dt-{}-{}", transform.dataset_transform_id, batch_key);
        match semantic_explorer_core::nats::publish_with_retry(
            nats,
            "workers.dataset-transform",
            &msg_id,
            payload.clone(),
            3,
        )
        .await
        {
            semantic_explorer_core::nats::PublishResult::Published => {
                jobs_created += 1;
                // Track dispatched batch for accurate completion detection
                if let Err(e) =
                    crate::storage::postgres::dataset_transform_stats::increment_dispatched_batch(
                        pool,
                        transform.dataset_transform_id,
                        chunk_count,
                    )
                    .await
                {
                    warn!("Failed to track dispatched batch: {}", e);
                }
            }
            semantic_explorer_core::nats::PublishResult::Failed(e) => {
                // Store in pending_batches for recovery
                error!(
                    batch_key = %batch_key,
                    chunk_count = chunk_count,
                    error = %e,
                    "Failed to publish job after retries. Saving for recovery."
                );
                if let Err(pe) = pending_batches::insert_pending_batch(
                    pool,
                    CreatePendingBatch {
                        batch_type: "dataset".to_string(),
                        dataset_transform_id: Some(transform.dataset_transform_id),
                        embedded_dataset_id: Some(embedded_dataset.embedded_dataset_id),
                        batch_key: batch_key.clone(),
                        s3_bucket: config.s3_bucket.clone(),
                        job_payload: serde_json::from_slice(&payload).unwrap_or_default(),
                    },
                )
                .await
                {
                    error!("Failed to save pending batch for recovery: {}", pe);
                }
            }
        }
    }

    info!(
        "Created {} batch jobs for embedded dataset {}",
        jobs_created, embedded_dataset.embedded_dataset_id
    );

    // Update the last_processed_at timestamp to the max item timestamp we processed
    // This prevents race conditions where items created between query and update are missed
    if jobs_created > 0
        && let Some(max_ts) = max_item_timestamp
    {
        embedded_datasets::update_embedded_dataset_last_processed_at_to(
            pool,
            embedded_dataset.embedded_dataset_id,
            max_ts,
        )
        .await?;
    }

    Ok(jobs_created)
}

/// Trigger a dataset transform scan via NATS (non-blocking)
///
/// This publishes a targeted scan trigger to NATS, which will be processed
/// asynchronously by the trigger listener. This allows the API to return
/// immediately while the scan runs in the background.
#[tracing::instrument(
    name = "trigger_dataset_transform_scan",
    skip(nats),
    fields(dataset_transform_id = %dataset_transform_id)
)]
pub async fn trigger_dataset_transform_scan(
    nats: &NatsClient,
    dataset_transform_id: i32,
    owner: &str,
) -> Result<()> {
    info!(
        "Publishing dataset transform scan trigger for {}",
        dataset_transform_id
    );

    crate::transforms::trigger::publish_targeted_trigger(
        nats,
        "dataset",
        dataset_transform_id,
        owner,
    )
    .await?;

    info!(
        "Published dataset transform scan trigger for {}",
        dataset_transform_id
    );

    Ok(())
}
