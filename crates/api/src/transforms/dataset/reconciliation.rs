//! Transform Reconciliation Job
//!
//! This background job ensures reliability in distributed transform processing:
//!
//! 1. **Pending Batch Recovery**: Retries batches that failed to publish to NATS (dataset + collection)
//! 2. **Stats Reconciliation**: Corrects stats that may have drifted due to failures
//! 3. **Failed Batch Recovery**: Re-publishes batches that failed during processing
//! 4. **Orphaned Batch Cleanup**: Removes stale pending batches older than 24 hours
//!
//! Run frequency: Every 5 minutes (configurable via RECONCILIATION_INTERVAL_SECS)

use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{Instrument, error, info, info_span, warn};

use crate::auth::AuthenticatedUser;
use crate::storage::postgres::dataset_transform_pending_batches::{
    self as pending_batches, PendingBatch,
};
use crate::storage::postgres::{dataset_transform_batches, dataset_transform_stats};
use crate::storage::postgres::{embedded_datasets, embedders};
use crate::storage::s3 as s3_storage;
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::{
    CollectionTransformJob, DatasetTransformJob, QdrantConnectionConfig,
};
use semantic_explorer_core::observability::{
    record_scanner_failed_batch_recovery, record_scanner_orphaned_batch_cleanup,
    record_scanner_pending_batch_recovery,
};

/// Configuration for the reconciliation job
#[derive(Clone)]
pub struct ReconciliationConfig {
    /// How often to run reconciliation (default: 5 minutes)
    pub interval: Duration,
    /// Maximum pending batches to retry per run
    pub max_pending_retries: i64,
    /// Hours after which a "processing" batch is considered stuck (default: 2)
    pub stuck_batch_threshold_hours: i64,
}

impl ReconciliationConfig {
    /// Load reconciliation configuration from environment variables.
    /// Call once at startup.
    pub fn from_env() -> Self {
        Self {
            interval: Duration::from_secs(
                std::env::var("RECONCILIATION_INTERVAL_SECS")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .unwrap_or(300),
            ),
            max_pending_retries: 100,
            stuck_batch_threshold_hours: std::env::var("STUCK_BATCH_THRESHOLD_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
        }
    }
}

/// Context for reconciliation operations
#[derive(Clone)]
pub struct ReconciliationContext {
    pub pool: Pool<Postgres>,
    pub nats_client: NatsClient,
    pub s3_client: S3Client,
    pub s3_bucket_name: String,
    pub config: ReconciliationConfig,
    pub encryption: EncryptionService,
    pub qdrant_config: QdrantConnectionConfig,
}

/// Start the background reconciliation job
pub fn start_reconciliation_job(ctx: ReconciliationContext) {
    actix_web::rt::spawn(async move {
        info!(
            interval_secs = ctx.config.interval.as_secs(),
            "Starting dataset transform reconciliation job"
        );

        let mut interval = tokio::time::interval(ctx.config.interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let span = info_span!("reconciliation_run");
                    async {
                        if let Err(e) = run_reconciliation(&ctx).await {
                            error!(error = %e, "Reconciliation run failed");
                        }
                    }
                    .instrument(span)
                    .await;
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Reconciliation job received shutdown signal, exiting gracefully");
                    break;
                }
            }
        }
    });
}

/// Run a single reconciliation cycle
async fn run_reconciliation(ctx: &ReconciliationContext) -> Result<()> {
    info!("Starting reconciliation run");

    // Step 1: Retry pending batches (dataset + collection)
    let pending_retried = retry_pending_batches(ctx).await?;
    if pending_retried > 0 {
        record_scanner_pending_batch_recovery("mixed", pending_retried as u64);
    }

    // Step 2: Recover failed dataset transform batches (#6)
    let failed_recovered = recover_failed_batches(ctx).await.unwrap_or_else(|e| {
        error!(error = %e, "Failed batch recovery encountered error");
        0
    });

    // Step 3: Cleanup orphaned pending batches older than 24 hours (#7)
    let orphaned_cleaned = cleanup_orphaned_batches(ctx).await.unwrap_or_else(|e| {
        error!(error = %e, "Orphaned batch cleanup encountered error");
        0
    });

    // Step 4: Cleanup old published/expired pending batch records
    let cleaned_up = pending_batches::cleanup_old_batches(&ctx.pool).await?;

    // Step 5: Detect stuck/stale batches (#18)
    detect_stuck_batches(ctx).await;

    info!(
        pending_retried = pending_retried,
        failed_recovered = failed_recovered,
        orphaned_cleaned = orphaned_cleaned,
        old_records_cleaned = cleaned_up,
        "Reconciliation run completed"
    );

    Ok(())
}

/// Retry pending batches that failed to publish
async fn retry_pending_batches(ctx: &ReconciliationContext) -> Result<usize> {
    let pending =
        pending_batches::get_pending_batches_for_retry(&ctx.pool, ctx.config.max_pending_retries)
            .await?;

    if pending.is_empty() {
        return Ok(0);
    }

    info!(count = pending.len(), "Found pending batches to retry");

    let mut success_count = 0;

    for batch in pending {
        match retry_single_batch(ctx, &batch).await {
            Ok(true) => {
                success_count += 1;
                if let Err(e) = pending_batches::mark_batch_published(&ctx.pool, batch.id).await {
                    error!(batch_id = batch.id, error = %e, "Failed to mark batch as published");
                }
            }
            Ok(false) => {
                // Batch no longer valid (e.g., transform deleted), mark as expired
                warn!(
                    batch_id = batch.id,
                    "Batch no longer valid, marking as failed"
                );
                let _ = pending_batches::mark_batch_failed(
                    &ctx.pool,
                    batch.id,
                    "Batch no longer valid or transform deleted",
                )
                .await;
            }
            Err(e) => {
                // Retry failed, increment counter
                if batch.retry_count >= batch.max_retries - 1 {
                    error!(
                        batch_id = batch.id,
                        error = %e,
                        "Batch exceeded max retries, marking as failed"
                    );
                    let _ = pending_batches::mark_batch_failed(
                        &ctx.pool,
                        batch.id,
                        &format!("Max retries exceeded: {}", e),
                    )
                    .await;
                } else {
                    warn!(
                        batch_id = batch.id,
                        retry_count = batch.retry_count + 1,
                        error = %e,
                        "Retry failed, will try again later"
                    );
                    let _ = pending_batches::increment_retry(
                        &ctx.pool,
                        batch.id,
                        &e.to_string(),
                        batch.retry_count,
                    )
                    .await;
                }
            }
        }
    }

    Ok(success_count)
}

/// Retry publishing a single pending batch
async fn retry_single_batch(ctx: &ReconciliationContext, batch: &PendingBatch) -> Result<bool> {
    match batch.batch_type.as_str() {
        "dataset" => retry_dataset_batch(ctx, batch).await,
        "collection" => retry_collection_batch(ctx, batch).await,
        _ => {
            warn!(batch_type = %batch.batch_type, "Unknown batch type, skipping");
            Ok(false)
        }
    }
}

/// Retry a dataset transform pending batch
async fn retry_dataset_batch(ctx: &ReconciliationContext, batch: &PendingBatch) -> Result<bool> {
    // Deserialize job payload
    let job: DatasetTransformJob = serde_json::from_value(batch.job_payload.clone())
        .map_err(|e| anyhow::anyhow!("Invalid job payload: {}", e))?;

    // Verify the embedded dataset still exists
    if let Some(ed_id) = batch.embedded_dataset_id {
        match embedded_datasets::get_embedded_dataset_by_id(&ctx.pool, ed_id).await {
            Ok(_) => {} // Dataset exists, continue
            Err(_) => {
                // Dataset was deleted, batch is no longer valid
                return Ok(false);
            }
        }
    }

    // Re-publish to NATS
    let payload = serde_json::to_vec(&job)?;
    let msg_id = format!("dt-recovery-{}-{}", batch.id, batch.batch_key);

    match semantic_explorer_core::nats::publish_with_retry(
        &ctx.nats_client,
        "workers.dataset-transform",
        &msg_id,
        payload,
        3,
    )
    .await
    {
        semantic_explorer_core::nats::PublishResult::Published => {
            info!(
                batch_id = batch.id,
                batch_key = %batch.batch_key,
                "Successfully recovered pending dataset batch"
            );

            // Track dispatched batch
            if let Some(dt_id) = batch.dataset_transform_id {
                let _ = dataset_transform_stats::increment_dispatched_batch(
                    &ctx.pool, dt_id, 0, // Unknown chunk count
                )
                .await;
            }

            Ok(true)
        }
        semantic_explorer_core::nats::PublishResult::Failed(e) => {
            Err(anyhow::anyhow!("Publish failed: {}", e))
        }
    }
}

/// Retry a collection transform pending batch
async fn retry_collection_batch(ctx: &ReconciliationContext, batch: &PendingBatch) -> Result<bool> {
    // Deserialize job payload
    let job: CollectionTransformJob = serde_json::from_value(batch.job_payload.clone())
        .map_err(|e| anyhow::anyhow!("Invalid collection job payload: {}", e))?;

    // Re-publish to NATS
    let payload = serde_json::to_vec(&job)?;
    let msg_id = format!("ct-recovery-{}-{}", batch.id, batch.batch_key);

    match semantic_explorer_core::nats::publish_with_retry(
        &ctx.nats_client,
        "workers.collection-transform",
        &msg_id,
        payload,
        3,
    )
    .await
    {
        semantic_explorer_core::nats::PublishResult::Published => {
            info!(
                batch_id = batch.id,
                batch_key = %batch.batch_key,
                "Successfully recovered pending collection batch"
            );
            Ok(true)
        }
        semantic_explorer_core::nats::PublishResult::Failed(e) => {
            Err(anyhow::anyhow!("Publish failed: {}", e))
        }
    }
}

/// Recover failed dataset transform batches by re-publishing jobs (#6)
async fn recover_failed_batches(ctx: &ReconciliationContext) -> Result<usize> {
    use crate::storage::postgres::dataset_transforms::get_active_dataset_transforms_privileged;

    let transforms = get_active_dataset_transforms_privileged(&ctx.pool).await?;
    let mut total_recovered = 0;

    for transform in transforms {
        let (failed_batches, _count) = dataset_transform_batches::list_batches_by_status(
            &ctx.pool,
            transform.dataset_transform_id,
            "failed",
            50, // limit
            0,
            "processed_at",
            "desc",
        )
        .await?;

        for batch in failed_batches {
            // Check if batch file still exists in S3
            match s3_storage::file_exists(&ctx.s3_client, &ctx.s3_bucket_name, &batch.batch_key)
                .await
            {
                Ok(true) => {
                    // Re-create and publish the job
                    // Get embedded datasets for this transform to find the right one
                    let embedded_datasets_list =
                        embedded_datasets::get_embedded_datasets_for_transform(
                            &ctx.pool,
                            transform.dataset_transform_id,
                        )
                        .await?;

                    if let Some(ed) = embedded_datasets_list.first() {
                        // Look up actual embedder config for recovery
                        let user = AuthenticatedUser(transform.owner_display_name.clone());
                        let embedder = match embedders::get_embedder(
                            &ctx.pool,
                            &user,
                            ed.embedder_id,
                            &ctx.encryption,
                        )
                        .await
                        {
                            Ok(embedder) => embedder,
                            Err(e) => {
                                warn!(
                                    batch_key = %batch.batch_key,
                                    error = %e,
                                    "Failed to look up embedder for batch recovery, skipping"
                                );
                                continue;
                            }
                        };

                        let model = embedder
                            .config
                            .get("model")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let embedder_config = semantic_explorer_core::models::EmbedderConfig::new(
                            embedder.provider.clone(),
                            embedder.base_url.clone(),
                            embedder.api_key.clone(),
                            model,
                            embedder.config.clone(),
                            embedder.batch_size,
                            embedder.max_input_tokens,
                        );

                        info!(
                            dataset_transform_id = transform.dataset_transform_id,
                            batch_key = %batch.batch_key,
                            "Re-publishing failed batch for recovery"
                        );

                        let msg_id = format!(
                            "dt-failed-recovery-{}-{}",
                            transform.dataset_transform_id, batch.batch_key
                        );
                        let job = DatasetTransformJob {
                            job_id: uuid::Uuid::new_v4(),
                            batch_file_key: batch.batch_key.clone(),
                            bucket: ctx.s3_bucket_name.clone(),
                            dataset_id: transform.source_dataset_id,
                            dataset_transform_id: transform.dataset_transform_id,
                            embedded_dataset_id: ed.embedded_dataset_id,
                            owner_id: transform.owner_id.clone(),
                            embedder_config,
                            qdrant_config: ctx.qdrant_config.clone(),
                            collection_name: ed.collection_name.clone(),
                            batch_size: Some(embedder.batch_size as usize),
                        };

                        let payload = serde_json::to_vec(&job)?;
                        match semantic_explorer_core::nats::publish_with_retry(
                            &ctx.nats_client,
                            "workers.dataset-transform",
                            &msg_id,
                            payload,
                            3,
                        )
                        .await
                        {
                            semantic_explorer_core::nats::PublishResult::Published => {
                                total_recovered += 1;
                            }
                            semantic_explorer_core::nats::PublishResult::Failed(e) => {
                                warn!(
                                    batch_key = %batch.batch_key,
                                    error = %e,
                                    "Failed to re-publish failed batch"
                                );
                            }
                        }
                    }
                }
                Ok(false) => {
                    // Batch file missing from S3, mark as permanently failed
                    warn!(
                        batch_key = %batch.batch_key,
                        "Failed batch file not found in S3, cannot recover"
                    );
                }
                Err(e) => {
                    warn!(
                        batch_key = %batch.batch_key,
                        error = %e,
                        "Error checking S3 for failed batch file"
                    );
                }
            }
        }
    }

    if total_recovered > 0 {
        info!(total_recovered, "Recovered failed batches");
        record_scanner_failed_batch_recovery("dataset", total_recovered as u64);
    }

    Ok(total_recovered)
}

/// Cleanup orphaned pending batches older than 24 hours (#7)
async fn cleanup_orphaned_batches(ctx: &ReconciliationContext) -> Result<usize> {
    let orphaned = pending_batches::get_orphaned_pending_batches(&ctx.pool, 24).await?;

    if orphaned.is_empty() {
        return Ok(0);
    }

    info!(
        count = orphaned.len(),
        "Found orphaned pending batches to clean up"
    );

    let mut cleaned = 0;
    for batch in &orphaned {
        // Try to delete the S3 file if it exists
        if let Err(e) =
            s3_storage::delete_file_by_key(&ctx.s3_client, &batch.s3_bucket, &batch.batch_key).await
        {
            warn!(
                batch_id = batch.id,
                batch_key = %batch.batch_key,
                error = %e,
                "Failed to delete orphaned batch file from S3 (may already be deleted)"
            );
        }

        // Mark the batch as expired
        if let Err(e) = pending_batches::mark_batch_failed(
            &ctx.pool,
            batch.id,
            "Orphaned batch cleaned up after 24 hours",
        )
        .await
        {
            error!(batch_id = batch.id, error = %e, "Failed to mark orphaned batch as failed");
        } else {
            cleaned += 1;
        }
    }

    info!(cleaned, "Cleaned up orphaned pending batches");
    if cleaned > 0 {
        record_scanner_orphaned_batch_cleanup(cleaned as u64);
    }
    Ok(cleaned)
}

/// Detect stuck/stale batches that have been in "processing" state too long
///
/// This helps identify potential deadlocks where a worker picked up a batch
/// but never completed it (e.g., worker crash, OOM, network partition).
async fn detect_stuck_batches(ctx: &ReconciliationContext) {
    let stuck_threshold_hours = ctx.config.stuck_batch_threshold_hours;

    match dataset_transform_batches::list_stuck_batches(
        &ctx.pool,
        "processing",
        stuck_threshold_hours,
        100,
    )
    .await
    {
        Ok(stuck_batches) => {
            if !stuck_batches.is_empty() {
                for batch in &stuck_batches {
                    let age_hours = (chrono::Utc::now() - batch.created_at).num_hours();
                    warn!(
                        batch_key = %batch.batch_key,
                        dataset_transform_id = batch.dataset_transform_id,
                        age_hours = age_hours,
                        "Detected stuck batch in processing state"
                    );
                }
                warn!(
                    stuck_count = stuck_batches.len(),
                    threshold_hours = stuck_threshold_hours,
                    "Found stuck batches in processing state - workers may be unhealthy"
                );
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to check for stuck batches");
        }
    }
}
