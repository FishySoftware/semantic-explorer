//! Dataset Transform Reconciliation Job
//!
//! This background job ensures reliability in distributed transform processing:
//!
//! 1. **Pending Batch Recovery**: Retries batches that failed to publish to NATS
//! 2. **Stats Reconciliation**: Corrects stats that may have drifted due to failures
//!
//! Run frequency: Every 5 minutes (configurable via RECONCILIATION_INTERVAL_SECS)

use anyhow::Result;
use async_nats::Client as NatsClient;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{Instrument, error, info, info_span, warn};

use crate::storage::postgres::dataset_transform_pending_batches::{
    self as pending_batches, PendingBatch,
};
use crate::storage::postgres::dataset_transform_stats;
use crate::storage::postgres::embedded_datasets;
use semantic_explorer_core::models::DatasetTransformJob;

/// Configuration for the reconciliation job
#[derive(Clone)]
pub struct ReconciliationConfig {
    /// How often to run reconciliation (default: 5 minutes)
    pub interval: Duration,
    /// Maximum pending batches to retry per run
    pub max_pending_retries: i64,
}

impl Default for ReconciliationConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(
                std::env::var("RECONCILIATION_INTERVAL_SECS")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .unwrap_or(300),
            ),
            max_pending_retries: 100,
        }
    }
}

/// Context for reconciliation operations
#[derive(Clone)]
pub struct ReconciliationContext {
    pub pool: Pool<Postgres>,
    pub nats_client: NatsClient,
    pub config: ReconciliationConfig,
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
            interval.tick().await;

            let span = info_span!("reconciliation_run");
            async {
                if let Err(e) = run_reconciliation(&ctx).await {
                    error!(error = %e, "Reconciliation run failed");
                }
            }
            .instrument(span)
            .await;
        }
    });
}

/// Run a single reconciliation cycle
async fn run_reconciliation(ctx: &ReconciliationContext) -> Result<()> {
    info!("Starting reconciliation run");

    // Step 1: Retry pending batches
    let pending_retried = retry_pending_batches(ctx).await?;

    // Step 2: Cleanup old published/expired pending batch records
    let cleaned_up = pending_batches::cleanup_old_batches(&ctx.pool).await?;

    info!(
        pending_retried = pending_retried,
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
                "Successfully recovered pending batch"
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
