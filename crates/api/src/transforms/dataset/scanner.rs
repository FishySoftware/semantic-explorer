use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

use semantic_explorer_core::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::{DatasetTransformJob, EmbedderConfig, QdrantConnectionConfig};
use semantic_explorer_core::observability::{
    record_scanner_backpressure_skip, record_scanner_batches_created,
    record_scanner_circuit_breaker_trip, record_scanner_items_discovered,
    record_scanner_stats_refresh_skip,
};
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
use crate::storage::postgres::{INTERNAL_BATCH_SIZE, fetch_all_batched};
use crate::storage::s3;
use crate::transforms::dataset::models::DatasetTransform;

/// Backpressure state for scanner throttling (#3)
enum BackpressureState {
    /// Queue is within limits, proceed with scanning
    Ok(u64),
    /// Queue is overloaded, skip this scan cycle
    Overloaded(u64),
}

/// Configuration for the dataset scanner, loaded from env at startup.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Delay in ms between batch publishes (default: 0 = no limit)
    pub batch_delay_ms: u64,
    /// Maximum batches per scan (default: 1000)
    pub max_batches_per_scan: usize,
    /// Maximum items per scan (default: 100_000)
    pub max_items_per_scan: usize,
    /// Timeout for individual scans in seconds (default: 300)
    pub scan_timeout_secs: u64,
    /// Maximum pending messages before backpressure kicks in (default: 500)
    pub max_pending: u64,
}

impl ScannerConfig {
    /// Load scanner configuration from environment variables.
    /// Call once at startup.
    pub fn from_env() -> Self {
        Self {
            batch_delay_ms: std::env::var("DATASET_SCANNER_BATCH_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            max_batches_per_scan: std::env::var("DATASET_SCANNER_MAX_BATCHES_PER_SCAN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            max_items_per_scan: std::env::var("DATASET_SCANNER_MAX_ITEMS_PER_SCAN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100_000),
            scan_timeout_secs: std::env::var("DATASET_SCANNER_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            max_pending: std::env::var("DATASET_SCANNER_MAX_PENDING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(500),
        }
    }
}

/// Check NATS JetStream stream for pending message count to implement backpressure
async fn check_backpressure(
    nats: &NatsClient,
    stream_name: &str,
    max_pending: u64,
) -> BackpressureState {
    let jetstream = async_nats::jetstream::new(nats.clone());
    match jetstream.get_stream(stream_name).await {
        Ok(mut stream) => match stream.info().await {
            Ok(info) => {
                let pending = info.state.messages;
                if pending >= max_pending {
                    BackpressureState::Overloaded(pending)
                } else {
                    BackpressureState::Ok(pending)
                }
            }
            Err(e) => {
                // If we can't check, proceed with scan (fail-open)
                warn!("Failed to get stream info for backpressure check: {}", e);
                BackpressureState::Ok(0)
            }
        },
        Err(e) => {
            // Stream might not exist yet, proceed
            warn!("Failed to get stream for backpressure check: {}", e);
            BackpressureState::Ok(0)
        }
    }
}

/// Simple rate limiter for batch publishing (#8)
/// Uses a fixed delay between operations to prevent overwhelming downstream systems.
struct RateLimiter {
    delay: Duration,
}

impl RateLimiter {
    /// Create a rate limiter from scanner config.
    fn new(batch_delay_ms: u64) -> Self {
        Self {
            delay: Duration::from_millis(batch_delay_ms),
        }
    }

    /// Wait for the rate limit interval (no-op if delay is zero)
    async fn acquire(&self) {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
    }
}

/// Create a circuit breaker for scanner batch publishing (#9)
fn scanner_circuit_breaker() -> Arc<CircuitBreaker> {
    CircuitBreaker::new(CircuitBreakerConfig::new("dataset_scanner"))
}

/// Resource limits for scanner operations
struct ScannerLimits {
    /// Maximum number of batches to create per transform scan
    max_batches_per_scan: usize,
    /// Maximum number of items to process in a single scan
    max_items_per_scan: usize,
}

impl ScannerLimits {
    fn new(config: &ScannerConfig) -> Self {
        Self {
            max_batches_per_scan: config.max_batches_per_scan,
            max_items_per_scan: config.max_items_per_scan,
        }
    }
}

/// Validate a dataset transform configuration before scanning (#14)
fn validate_transform_config(transform: &DatasetTransform) -> Result<()> {
    if transform.embedder_ids.is_empty() {
        anyhow::bail!(
            "Dataset transform {} has no embedder IDs configured",
            transform.dataset_transform_id
        );
    }
    if transform.source_dataset_id <= 0 {
        anyhow::bail!(
            "Dataset transform {} has invalid source_dataset_id: {}",
            transform.dataset_transform_id,
            transform.source_dataset_id
        );
    }
    if transform.owner_id.is_empty() {
        anyhow::bail!(
            "Dataset transform {} has empty owner_id",
            transform.dataset_transform_id
        );
    }
    Ok(())
}

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
    scanner_config: &ScannerConfig,
) -> Result<()> {
    let transforms = get_active_dataset_transforms_privileged(pool).await?;
    info!("Scanning {} active dataset transforms", transforms.len());

    for transform in transforms {
        match tokio::time::timeout(
            Duration::from_secs(scanner_config.scan_timeout_secs),
            process_dataset_transform_scan(
                pool,
                nats,
                s3,
                s3_bucket_name,
                &transform,
                encryption,
                qdrant_config,
                scanner_config,
            ),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                error!(
                    "Failed to process dataset transform scan for {}: {}",
                    transform.dataset_transform_id, e
                );
            }
            Err(_) => {
                error!(
                    dataset_transform_id = transform.dataset_transform_id,
                    timeout_secs = scanner_config.scan_timeout_secs,
                    "Dataset transform scan timed out"
                );
            }
        }
    }
    Ok(())
}

/// Scan a specific dataset transform by ID (privileged, for NATS triggers)
#[tracing::instrument(
    name = "scan_dataset_transform",
    skip(pool, nats, s3, encryption, qdrant_config, scanner_config),
    fields(dataset_transform_id = %dataset_transform_id)
)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn scan_dataset_transform(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    dataset_transform_id: i32,
    encryption: &EncryptionService,
    qdrant_config: &QdrantConnectionConfig,
    scanner_config: &ScannerConfig,
) -> Result<()> {
    let transform = get_dataset_transform_privileged(pool, dataset_transform_id).await?;

    if !transform.is_enabled {
        info!(
            "Dataset transform {} is disabled, skipping scan",
            dataset_transform_id
        );
        return Ok(());
    }

    // Validate transform configuration (#14)
    if let Err(e) = validate_transform_config(&transform) {
        error!(
            dataset_transform_id = dataset_transform_id,
            error = %e,
            "Transform configuration validation failed"
        );
        return Err(e);
    }

    process_dataset_transform_scan(
        pool,
        nats,
        s3,
        s3_bucket_name,
        &transform,
        encryption,
        qdrant_config,
        scanner_config,
    )
    .await
}

#[tracing::instrument(
    name = "process_dataset_transform_scan",
    skip(pool, nats, s3, transform, encryption, qdrant_config, scanner_config),
    fields(dataset_transform_id = %transform.dataset_transform_id, embedder_count = %transform.embedder_ids.len())
)]
#[allow(clippy::too_many_arguments)]
async fn process_dataset_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    transform: &DatasetTransform,
    encryption: &EncryptionService,
    qdrant_config: &QdrantConnectionConfig,
    scanner_config: &ScannerConfig,
) -> Result<()> {
    info!(
        "Starting dataset transform scan for {} with {} embedders",
        transform.dataset_transform_id,
        transform.embedder_ids.len()
    );

    // Backpressure check: Skip scan if too many pending jobs (#3)
    match check_backpressure(nats, "DATASET_TRANSFORMS", scanner_config.max_pending).await {
        BackpressureState::Overloaded(pending) => {
            warn!(
                pending_messages = pending,
                max_pending = scanner_config.max_pending,
                dataset_transform_id = transform.dataset_transform_id,
                "Workers at capacity, skipping scan (backpressure)"
            );
            record_scanner_backpressure_skip("dataset");
            return Ok(());
        }
        BackpressureState::Ok(pending) => {
            if pending > 0 {
                info!(
                    pending_messages = pending,
                    "Backpressure check passed, proceeding with scan"
                );
            }
        }
    }

    // Get all embedded datasets for this transform
    let embedded_datasets_list = fetch_all_batched(INTERNAL_BATCH_SIZE, |limit, offset| {
        embedded_datasets::get_embedded_datasets_for_transform(
            pool,
            transform.dataset_transform_id,
            limit,
            offset,
        )
    })
    .await?;

    // Only refresh total_chunks_to_process if the source dataset has changed (#4)
    let dataset_version = datasets::get_dataset_version(pool, transform.source_dataset_id).await?;

    let needs_refresh = match (
        dataset_version,
        embedded_datasets_list
            .first()
            .and_then(|ed| ed.source_dataset_version),
    ) {
        (Some(current), Some(cached)) => current != cached,
        (Some(_), None) => true, // No cached version yet, need initial refresh
        _ => false,              // Dataset doesn't exist or no embedded datasets
    };

    if needs_refresh {
        info!(
            dataset_transform_id = transform.dataset_transform_id,
            "Source dataset version changed, refreshing stats"
        );
        if let Err(e) = crate::storage::postgres::dataset_transform_stats::refresh_total_chunks(
            pool,
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

        // Update cached version on all embedded datasets
        if let Some(version) = dataset_version {
            for ed in &embedded_datasets_list {
                if let Err(e) = embedded_datasets::update_source_dataset_version(
                    pool,
                    ed.embedded_dataset_id,
                    version,
                )
                .await
                {
                    warn!(
                        embedded_dataset_id = ed.embedded_dataset_id,
                        "Failed to update source dataset version: {}", e
                    );
                }
            }
        }
    } else {
        record_scanner_stats_refresh_skip("dataset");
    }

    info!(
        "Found {} embedded datasets for dataset transform {}",
        embedded_datasets_list.len(),
        transform.dataset_transform_id
    );

    let embedded_datasets_count = embedded_datasets_list.len();
    let mut total_jobs = 0;

    // Initialize rate limiter and circuit breaker for batch publishing (#8, #9)
    let rate_limiter = RateLimiter::new(scanner_config.batch_delay_ms);
    let circuit_breaker = scanner_circuit_breaker();

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
        // Get embedder from pre-fetched map (skip if embedder was deleted)
        let embedder = match embedders_map.get(&embedded_dataset.embedder_id) {
            Some(e) => e,
            None => {
                warn!(
                    embedded_dataset_id = embedded_dataset.embedded_dataset_id,
                    embedder_id = embedded_dataset.embedder_id,
                    "Skipping embedded dataset: embedder no longer exists"
                );
                continue;
            }
        };

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
        let processed_batches = fetch_all_batched(INTERNAL_BATCH_SIZE, |limit, offset| {
            embedded_datasets::get_processed_batches(
                pool,
                embedded_dataset.embedded_dataset_id,
                limit,
                offset,
            )
        })
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
            // Use a unique msg_id with UUID to avoid NATS deduplication rejecting
            // re-dispatched batches (e.g. after retry-failed resets them to pending).
            let msg_id = format!(
                "dt-redispatch-{}-{}-{}",
                transform.dataset_transform_id,
                batch_file_key,
                Uuid::new_v4()
            );
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
                            collection_transform_id: None,
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
            // Acquire scan lock to prevent concurrent scanners from racing on the same watermark.
            // The lock auto-expires after scan_timeout_secs to prevent stale locks.
            let lock_acquired = embedded_datasets::try_acquire_scan_lock(
                pool,
                embedded_dataset.embedded_dataset_id,
                scanner_config.scan_timeout_secs,
            )
            .await
            .unwrap_or(false);

            if !lock_acquired {
                info!(
                    embedded_dataset_id = embedded_dataset.embedded_dataset_id,
                    "Skipping embedded dataset: another scanner holds the scan lock"
                );
                continue;
            }

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
                &rate_limiter,
                &circuit_breaker,
                scanner_config,
            )
            .await;

            // Always release the scan lock, even on error
            if let Err(e) =
                embedded_datasets::release_scan_lock(pool, embedded_dataset.embedded_dataset_id)
                    .await
            {
                warn!(
                    embedded_dataset_id = embedded_dataset.embedded_dataset_id,
                    "Failed to release scan lock: {}", e
                );
            }

            total_jobs += items_created?;
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
#[allow(clippy::too_many_arguments)]
async fn create_batches_from_dataset_items(
    pool: &Pool<Postgres>,
    s3: &S3Client,
    nats: &NatsClient,
    transform: &DatasetTransform,
    embedded_dataset: &EmbeddedDataset,
    config: &DatasetBatchConfig,
    rate_limiter: &RateLimiter,
    circuit_breaker: &Arc<CircuitBreaker>,
    scanner_config: &ScannerConfig,
) -> Result<usize> {
    // Use timestamp-based tracking to identify new items that need processing
    let last_processed_at = embedded_dataset.last_processed_at;

    // Snapshot the current time BEFORE querying to prevent watermark race conditions.
    // Any items added after this point will have timestamps >= query_start_time
    // and will be picked up on the next scan. Items with timestamps between
    // last_processed_at and query_start_time are guaranteed to be included.
    let query_start_time = chrono::Utc::now();

    info!(
        "Embedded dataset {} last processed at: {:?}, query snapshot at: {:?}",
        embedded_dataset.embedded_dataset_id, last_processed_at, query_start_time
    );

    // Apply resource limits
    let limits = ScannerLimits::new(scanner_config);
    // Fetch at most max_items_per_scan + 1 so we can detect truncation
    // without pulling unbounded rows from the database.
    let fetch_limit = (limits.max_items_per_scan as i64).saturating_add(1);
    let mut items = datasets::get_dataset_items_modified_since(
        pool,
        transform.source_dataset_id,
        last_processed_at,
        fetch_limit,
        0,
    )
    .await?;

    if items.is_empty() {
        info!(
            "Embedded dataset {} has no new items since last processing. Skipping.",
            embedded_dataset.embedded_dataset_id
        );
        return Ok(0);
    }

    let was_truncated = items.len() > limits.max_items_per_scan;
    if was_truncated {
        warn!(
            embedded_dataset_id = embedded_dataset.embedded_dataset_id,
            total_items = items.len(),
            max_items = limits.max_items_per_scan,
            "Truncating items to max_items_per_scan limit, remaining will be picked up on next scan"
        );
        items.truncate(limits.max_items_per_scan);
    }

    info!(
        "Embedded dataset {} found {} items with new/modified chunks",
        embedded_dataset.embedded_dataset_id,
        items.len(),
    );

    // Convert dataset items to batch items (one per chunk)
    let mut all_batch_items: Vec<serde_json::Value> = Vec::new();
    // Track cumulative chunk count per item for watermark calculation
    // (items are ordered ASC by updated_at, so last entry = newest processed item)
    let mut item_chunk_boundaries: Vec<(usize, chrono::DateTime<chrono::Utc>)> = Vec::new();
    // Use a namespace UUID for generating deterministic chunk IDs
    // This ensures the same item+chunk always gets the same UUID, enabling idempotent upserts
    let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap(); // URL namespace UUID
    for item in &items {
        let item_ts = item
            .updated_at
            .or(item.created_at)
            .unwrap_or(query_start_time);
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
        let cumulative_chunks = all_batch_items.len();
        item_chunk_boundaries.push((cumulative_chunks, item_ts));
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
    let mut actual_chunks_dispatched: usize = 0;
    let chunks_per_batch = config.embedding_batch_size;
    let total_batches = all_batch_items.len().div_ceil(chunks_per_batch);

    // Apply max batch limit
    let effective_total = total_batches.min(limits.max_batches_per_scan);
    let was_batches_capped = total_batches > effective_total;
    if was_batches_capped {
        warn!(
            total_batches = total_batches,
            max_batches = limits.max_batches_per_scan,
            "Capping batch creation at max_batches_per_scan limit"
        );
    }

    for (batch_idx, batch_chunk) in all_batch_items
        .chunks(chunks_per_batch)
        .take(effective_total)
        .enumerate()
    {
        // Circuit breaker check (#9): Stop publishing if too many failures
        if !circuit_breaker.should_allow().await {
            warn!(
                embedded_dataset_id = embedded_dataset.embedded_dataset_id,
                batch = batch_idx + 1,
                total_batches = total_batches,
                "Circuit breaker open, stopping batch creation"
            );
            record_scanner_circuit_breaker_trip("dataset");
            break;
        }

        // Rate limiting (#8): Throttle batch publishing
        rate_limiter.acquire().await;

        // Progress tracking (#5): Log progress every 10 batches or at start/end
        if batch_idx == 0 || (batch_idx + 1) % 10 == 0 || batch_idx + 1 == total_batches {
            info!(
                embedded_dataset_id = embedded_dataset.embedded_dataset_id,
                batch = batch_idx + 1,
                total_batches = total_batches,
                progress_pct = ((batch_idx + 1) * 100) / total_batches,
                "Batch creation progress"
            );
        }

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
                actual_chunks_dispatched += batch_chunk.len();
                circuit_breaker.record_success().await;
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
                circuit_breaker.record_failure().await;
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
                        collection_transform_id: None,
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

    if jobs_created > 0 {
        record_scanner_batches_created("dataset", jobs_created as u64);
    }

    // Advance the watermark based on what was actually processed.
    // Items are ordered ASC by updated_at, so we advance incrementally.
    //
    // - No truncation, no batch cap: safe to advance to query_start_time
    //   (all items between last_processed_at and query_start_time were batched)
    // - Items truncated (but all batches dispatched): advance to the max updated_at
    //   of fetched items. Truncated (newer) items will be picked up next scan.
    // - Batches capped: advance only to the updated_at of the last item whose chunks
    //   were fully included in dispatched batches. Remaining items will be picked up next scan.
    // - Re-processing is always safe due to deterministic chunk UUIDs (Uuid::new_v5)
    if jobs_created > 0 {
        let new_watermark = if !was_truncated && !was_batches_capped {
            // All items fetched and all batches dispatched
            query_start_time
        } else if was_batches_capped {
            // Only advance to cover items fully within dispatched batches.
            // Use actual_chunks_dispatched (tracked during the loop) instead of
            // effective_total * chunks_per_batch — this is resilient to partial
            // batches, circuit breaker breaks, and future refactors.
            item_chunk_boundaries
                .iter()
                .rfind(|(cumul, _)| *cumul <= actual_chunks_dispatched)
                .map(|(_, ts)| *ts)
                .unwrap_or(query_start_time)
        } else {
            // Items truncated, all batches dispatched:
            // advance to max updated_at of processed items (last in ASC order)
            item_chunk_boundaries
                .iter()
                .last()
                .map(|(_, ts)| *ts)
                .unwrap_or(query_start_time)
        };

        info!(
            embedded_dataset_id = embedded_dataset.embedded_dataset_id,
            was_truncated = was_truncated,
            was_batches_capped = was_batches_capped,
            ?new_watermark,
            ?query_start_time,
            "Advancing watermark"
        );

        embedded_datasets::update_embedded_dataset_last_processed_at_to(
            pool,
            embedded_dataset.embedded_dataset_id,
            new_watermark,
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
