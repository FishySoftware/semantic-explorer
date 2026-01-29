//! Dataset transform result listener
//!
//! Handles results from dataset transform workers (embedding generation).
//!
//! ## Reliability Guarantees
//!
//! This listener is designed for high reliability in distributed environments:
//!
//! 1. **Idempotency**: Results for already-completed batches are skipped (no duplicate stats)
//! 2. **Atomic Transactions**: All batch tracking and stats updates happen in a single transaction
//! 3. **ACK-after-commit**: NATS messages are only acknowledged AFTER the transaction commits
//! 4. **Distributed Tracing**: Trace context is propagated for end-to-end visibility

use async_nats::{Client as NatsClient, jetstream};
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{Instrument, error, info, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::storage::postgres::dataset_transform_batches::{
    CreateBatchRequest, create_batch_tx, update_batch_status_tx,
};
use crate::storage::postgres::embedded_datasets;
use crate::storage::s3::delete_file;
use semantic_explorer_core::models::DatasetTransformResult;
use semantic_explorer_core::nats::extract_otel_context;
use semantic_explorer_core::storage::delete_file_by_key;

use super::super::listeners::publish_transform_status;

/// Context for transform result handling
#[derive(Clone)]
pub(crate) struct DatasetListenerContext {
    pub pool: Pool<Postgres>,
    pub s3_client: S3Client,
    pub s3_bucket_name: String,
    pub nats_client: NatsClient,
}

/// Result of handling a batch result message
enum HandleResult {
    /// Successfully processed and committed
    Success,
    /// Skipped due to idempotency (already processed)
    Skipped,
    /// Resource was deleted - ACK and discard (don't retry)
    ResourceDeleted,
    /// Failed - should NAK for retry
    Failed(String),
}

/// Start the dataset transform result listener
pub(crate) fn start_result_listener(context: DatasetListenerContext) {
    let nats_client = context.nats_client.clone();

    actix_web::rt::spawn(async move {
        // Use JetStream durable consumer for reliable message delivery
        // Subject format: transforms.dataset.status.{owner}.{dataset_id}.{transform_id}
        let subject = "transforms.dataset.status.>";
        let stream_name = "TRANSFORM_STATUS";
        let consumer_name = "dataset-status-listener";

        let jetstream = jetstream::new(nats_client.clone());

        let stream = match jetstream.get_stream(stream_name).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get stream {}: {}", stream_name, e);
                return;
            }
        };

        // Create or get durable consumer
        let consumer = match stream.get_consumer(consumer_name).await {
            Ok(c) => c,
            Err(_) => {
                let config = jetstream::consumer::pull::Config {
                    durable_name: Some(consumer_name.to_string()),
                    description: Some("Dataset transform status listener".to_string()),
                    filter_subject: subject.to_string(),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    ack_wait: Duration::from_secs(120), // 2 minutes - enough for transaction
                    max_deliver: 5,
                    ..Default::default()
                };
                match stream.create_consumer(config).await {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to create consumer {}: {}", consumer_name, e);
                        return;
                    }
                }
            }
        };

        info!(
            "Dataset result listener started with durable consumer: {}",
            consumer_name
        );

        let mut messages = match consumer.messages().await {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to get message stream: {}", e);
                return;
            }
        };

        while let Some(msg) = messages.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to receive message: {}", e);
                    continue;
                }
            };

            // Extract trace context for distributed tracing
            let parent_context = extract_otel_context(msg.headers.as_ref());
            let span = info_span!(
                "handle_dataset_transform_result",
                subject = %msg.subject,
            );
            let _ = span.set_parent(parent_context);

            let result: DatasetTransformResult = match serde_json::from_slice(&msg.payload) {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to deserialize dataset transform result: {}", e);
                    // Acknowledge bad messages to prevent reprocessing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to acknowledge bad message: {}", ack_err);
                    }
                    continue;
                }
            };

            // Process within the trace span
            let handle_result = async { handle_result_atomic(result.clone(), &context).await }
                .instrument(span.clone())
                .await;

            // ACK/NAK based on result - CRITICAL: only after transaction commits
            match handle_result {
                HandleResult::Success | HandleResult::Skipped | HandleResult::ResourceDeleted => {
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                HandleResult::Failed(reason) => {
                    warn!("Batch handling failed, will retry: {}", reason);
                    // NAK with delay for retry
                    if let Err(e) = msg
                        .ack_with(async_nats::jetstream::AckKind::Nak(Some(
                            Duration::from_secs(30),
                        )))
                        .await
                    {
                        error!("Failed to NAK message: {}", e);
                    }
                }
            }
        }
    });
}

/// Handle a batch result atomically with proper idempotency and transaction boundaries
async fn handle_result_atomic(
    result: DatasetTransformResult,
    ctx: &DatasetListenerContext,
) -> HandleResult {
    info!(
        batch_key = %result.batch_file_key,
        status = %result.status,
        chunk_count = result.chunk_count,
        embedded_dataset_id = result.embedded_dataset_id,
        "Handling dataset transform batch result"
    );

    // Validate ownership by fetching the embedded dataset
    let embedded_dataset = match embedded_datasets::get_embedded_dataset(
        &ctx.pool,
        &result.owner_id,
        result.embedded_dataset_id,
    )
    .await
    {
        Ok(ed) => ed,
        Err(e) => {
            // Check if this is a "not found" error (resource deleted) vs transient error.
            // When a dataset or transform is deleted, the embedded_dataset is also deleted
            // via CASCADE. Jobs already in the queue will fail here - we should ACK and
            // clean up rather than retry forever.
            let error_str = e.to_string().to_lowercase();
            if error_str.contains("no rows") || error_str.contains("not found") {
                info!(
                    "Embedded dataset {} not found (likely deleted), discarding batch {}: {}",
                    result.embedded_dataset_id, result.batch_file_key, e
                );
                // Clean up S3 batch file since the transform is gone
                cleanup_orphaned_batch(ctx, &result).await;
                return HandleResult::ResourceDeleted;
            }
            // For other errors (connection issues, etc.), retry
            error!(
                "Failed to fetch embedded dataset {} for owner {}: {}",
                result.embedded_dataset_id, result.owner_id, e
            );
            return HandleResult::Failed(format!("Embedded dataset fetch error: {}", e));
        }
    };

    // Get the current status from transform_processed_files for idempotency check
    let previous_status = embedded_datasets::get_batch_previous_status(
        &ctx.pool,
        result.embedded_dataset_id,
        &result.batch_file_key,
    )
    .await;

    // IDEMPOTENCY GUARD: Skip if already successfully processed
    // This prevents duplicate stats when NATS redelivers messages
    if let Some(ref prev) = previous_status
        && (prev == "completed" || prev == "success")
    {
        info!(
            batch_key = %result.batch_file_key,
            previous_status = %prev,
            "Batch already completed, skipping (idempotency guard)"
        );
        return HandleResult::Skipped;
    }

    match result.status.as_str() {
        "processing" => {
            handle_processing_status_atomic(
                &result,
                &embedded_dataset,
                previous_status.as_deref(),
                ctx,
            )
            .await
        }
        "failed" => {
            handle_failed_status_atomic(&result, &embedded_dataset, previous_status.as_deref(), ctx)
                .await
        }
        "success" => {
            handle_success_status_atomic(
                &result,
                &embedded_dataset,
                previous_status.as_deref(),
                ctx,
            )
            .await
        }
        _ => {
            error!(
                "Unknown status '{}' for batch {}",
                result.status, result.batch_file_key
            );
            HandleResult::Failed(format!("Unknown status: {}", result.status))
        }
    }
}

/// Handle "processing" status - batch has started processing
async fn handle_processing_status_atomic(
    result: &DatasetTransformResult,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    previous_status: Option<&str>,
    ctx: &DatasetListenerContext,
) -> HandleResult {
    // Start a transaction for atomic updates
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to begin transaction: {}", e);
            return HandleResult::Failed(format!("Transaction begin failed: {}", e));
        }
    };

    info!(
        batch_key = %result.batch_file_key,
        embedded_dataset_id = result.embedded_dataset_id,
        chunk_count = result.chunk_count,
        "Marking batch as processing"
    );

    // Record that processing has started (in transaction)
    if let Err(e) = embedded_datasets::record_processed_batch_tx(
        &mut tx,
        result.embedded_dataset_id,
        &result.batch_file_key,
        result.chunk_count as i32,
        "processing",
        None,
        None,
    )
    .await
    {
        error!("Failed to record batch processing start: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Record batch failed: {}", e));
    }

    // Record in dataset_transform_batches for tracking at transform level (in transaction)
    if let Err(e) = create_batch_tx(
        &mut tx,
        CreateBatchRequest {
            dataset_transform_id: embedded_dataset.dataset_transform_id,
            batch_key: result.batch_file_key.clone(),
            status: "processing".to_string(),
            chunk_count: result.chunk_count as i32,
            error_message: None,
            processing_duration_ms: None,
        },
    )
    .await
    {
        error!("Failed to create dataset transform batch record: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Create batch failed: {}", e));
    }

    // Atomically increment processing stats (only if not already processing)
    if previous_status != Some("processing")
        && let Err(e) =
            crate::storage::postgres::dataset_transform_stats::increment_processing_batch(
                &mut tx,
                embedded_dataset.dataset_transform_id,
                result.chunk_count as i64,
                chrono::Utc::now(),
            )
            .await
    {
        error!("Failed to increment processing stats: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Increment stats failed: {}", e));
    }

    if let Err(e) = tx.commit().await {
        error!("Failed to commit transaction: {}", e);
        return HandleResult::Failed(format!("Transaction commit failed: {}", e));
    }

    HandleResult::Success
}

/// Handle "failed" status - batch processing failed
async fn handle_failed_status_atomic(
    result: &DatasetTransformResult,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    previous_status: Option<&str>,
    ctx: &DatasetListenerContext,
) -> HandleResult {
    // Start a transaction for atomic updates
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to begin transaction: {}", e);
            return HandleResult::Failed(format!("Transaction begin failed: {}", e));
        }
    };

    let error_msg = result
        .error
        .clone()
        .unwrap_or_else(|| "Unknown error".to_string());

    error!(
        batch_key = %result.batch_file_key,
        error = %error_msg,
        chunk_count = result.chunk_count,
        "Dataset transform batch failed"
    );

    // Record failure in transform_processed_files (in transaction)
    if let Err(e) = embedded_datasets::record_processed_batch_tx(
        &mut tx,
        result.embedded_dataset_id,
        &result.batch_file_key,
        result.chunk_count as i32,
        "failed",
        Some(&error_msg),
        result.processing_duration_ms,
    )
    .await
    {
        error!("Failed to record batch processing failure: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Record batch failed: {}", e));
    }

    // Update batch record at transform level (in transaction)
    if let Err(e) = update_batch_status_tx(
        &mut tx,
        embedded_dataset.dataset_transform_id,
        &result.batch_file_key,
        "failed",
        Some(&error_msg),
        result.processing_duration_ms,
        result.chunk_count as i32,
    )
    .await
    {
        error!("Failed to update dataset transform batch status: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Update batch status failed: {}", e));
    }

    // If transitioning from processing to failed, decrement processing counter
    if previous_status == Some("processing")
        && let Err(e) =
            crate::storage::postgres::dataset_transform_stats::decrement_processing_batch(
                &mut tx,
                embedded_dataset.dataset_transform_id,
                result.chunk_count as i64,
            )
            .await
    {
        error!("Failed to decrement processing stats: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Decrement stats failed: {}", e));
    }

    // Increment failed stats
    if let Err(e) = crate::storage::postgres::dataset_transform_stats::increment_failed_batch(
        &mut tx,
        embedded_dataset.dataset_transform_id,
        result.chunk_count as i64,
        chrono::Utc::now(),
    )
    .await
    {
        error!("Failed to increment failed stats: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Increment failed stats failed: {}", e));
    }

    if let Err(e) = tx.commit().await {
        error!("Failed to commit transaction: {}", e);
        return HandleResult::Failed(format!("Transaction commit failed: {}", e));
    }

    // Publish failed status for SSE streaming (non-critical, after commit)
    publish_transform_status(
        &ctx.nats_client,
        "dataset",
        &result.owner_id,
        embedded_dataset.source_dataset_id,
        result.dataset_transform_id,
        "failed",
        Some(&error_msg),
    )
    .await;

    HandleResult::Success
}

/// Handle "success" status - batch processing completed successfully
async fn handle_success_status_atomic(
    result: &DatasetTransformResult,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    previous_status: Option<&str>,
    ctx: &DatasetListenerContext,
) -> HandleResult {
    // Start a transaction for atomic updates
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to begin transaction: {}", e);
            return HandleResult::Failed(format!("Transaction begin failed: {}", e));
        }
    };

    info!(
        batch_key = %result.batch_file_key,
        chunk_count = result.chunk_count,
        embedded_dataset_id = result.embedded_dataset_id,
        duration_ms = result.processing_duration_ms.unwrap_or(0),
        "Marking batch as completed"
    );

    // Record success in transform_processed_files (in transaction)
    if let Err(e) = embedded_datasets::record_processed_batch_tx(
        &mut tx,
        result.embedded_dataset_id,
        &result.batch_file_key,
        result.chunk_count as i32,
        "completed",
        None,
        result.processing_duration_ms,
    )
    .await
    {
        error!("Failed to record batch processing: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Record batch failed: {}", e));
    }

    // Update batch record at transform level (in transaction)
    if let Err(e) = update_batch_status_tx(
        &mut tx,
        embedded_dataset.dataset_transform_id,
        &result.batch_file_key,
        "success",
        None,
        result.processing_duration_ms,
        result.chunk_count as i32,
    )
    .await
    {
        error!("Failed to update dataset transform batch status: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Update batch status failed: {}", e));
    }

    // If transitioning from processing to success, decrement processing counter
    if previous_status == Some("processing")
        && let Err(e) =
            crate::storage::postgres::dataset_transform_stats::decrement_processing_batch(
                &mut tx,
                embedded_dataset.dataset_transform_id,
                result.chunk_count as i64,
            )
            .await
    {
        error!("Failed to decrement processing stats: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Decrement stats failed: {}", e));
    }

    // Increment successful stats
    if let Err(e) = crate::storage::postgres::dataset_transform_stats::increment_successful_batch(
        &mut tx,
        embedded_dataset.dataset_transform_id,
        result.chunk_count as i64,
        chrono::Utc::now(),
    )
    .await
    {
        error!("Failed to increment successful stats: {}", e);
        let _ = tx.rollback().await;
        return HandleResult::Failed(format!("Increment success stats failed: {}", e));
    }

    if let Err(e) = tx.commit().await {
        error!("Failed to commit transaction: {}", e);
        return HandleResult::Failed(format!("Transaction commit failed: {}", e));
    }

    // Clean up the batch file from S3 after successful processing and database commit
    // Extract prefix and file key from batch_file_key
    // Format: embedded-datasets/embedded-dataset-{id}/batch-{uuid}.jsonl
    let parts: Vec<&str> = result.batch_file_key.rsplitn(2, '/').collect();
    let (prefix, file_key) = if parts.len() == 2 {
        (parts[1], parts[0])
    } else {
        ("", result.batch_file_key.as_str())
    };

    if let Err(e) = delete_file(&ctx.s3_client, &ctx.s3_bucket_name, prefix, file_key).await {
        // Log the error but don't fail - batch was successfully processed and recorded
        warn!(
            batch_key = %result.batch_file_key,
            error = %e,
            "Failed to cleanup batch file from S3. Manual cleanup may be required."
        );
    } else {
        info!(
            batch_key = %result.batch_file_key,
            "Cleaned up batch file from S3"
        );
    }

    // Publish completed status for SSE streaming (non-critical, after commit)
    publish_transform_status(
        &ctx.nats_client,
        "dataset",
        &result.owner_id,
        embedded_dataset.source_dataset_id,
        result.dataset_transform_id,
        "completed",
        None,
    )
    .await;

    info!(
        batch_key = %result.batch_file_key,
        chunk_count = result.chunk_count,
        "Successfully processed dataset transform batch"
    );

    HandleResult::Success
}

/// Clean up orphaned batch files from S3 when a transform has been deleted.
/// This prevents accumulating orphan files when datasets/transforms are deleted
/// while jobs are still processing.
async fn cleanup_orphaned_batch(ctx: &DatasetListenerContext, result: &DatasetTransformResult) {
    if result.batch_file_key.is_empty() {
        return;
    }

    // Best-effort cleanup using the full batch file key
    if let Err(e) =
        delete_file_by_key(&ctx.s3_client, &ctx.s3_bucket_name, &result.batch_file_key).await
    {
        warn!(
            batch_key = %result.batch_file_key,
            error = %e,
            "Failed to clean up orphaned batch file from S3 (non-critical)"
        );
    } else {
        info!(
            batch_key = %result.batch_file_key,
            "Cleaned up orphaned batch file from S3"
        );
    }
}
