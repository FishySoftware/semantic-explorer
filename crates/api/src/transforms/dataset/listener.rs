//! Dataset transform result listener
//!
//! Handles results from dataset transform workers (embedding generation).

use async_nats::{Client as NatsClient, jetstream};
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info, warn};

use crate::storage::postgres::dataset_transform_batches::CreateBatchRequest;
use crate::storage::postgres::{dataset_transform_batches, embedded_datasets};
use crate::storage::s3::delete_file;
use semantic_explorer_core::models::DatasetTransformResult;

use super::super::listeners::publish_transform_status;

/// Context for transform result handling
#[derive(Clone)]
pub(crate) struct DatasetListenerContext {
    pub pool: Pool<Postgres>,
    pub s3_client: S3Client,
    pub nats_client: NatsClient,
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
                    ack_wait: Duration::from_secs(60),
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

            info!(
                "Received dataset transform result on subject: {}",
                msg.subject
            );
            match serde_json::from_slice::<DatasetTransformResult>(&msg.payload) {
                Ok(result) => {
                    handle_result(result, &context).await;
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to deserialize dataset transform result: {}", e);
                    // Acknowledge bad messages to prevent reprocessing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to acknowledge bad message: {}", ack_err);
                    }
                }
            }
        }
    });
}

#[tracing::instrument(name = "handle_dataset_transform_result", skip(ctx))]
async fn handle_result(result: DatasetTransformResult, ctx: &DatasetListenerContext) {
    info!(
        "Handling dataset transform batch result for: {} (status: {})",
        result.batch_file_key, result.status
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
            error!(
                "Embedded dataset {} not found or access denied for owner {}: {}",
                result.embedded_dataset_id, result.owner_id, e
            );
            return;
        }
    };

    // Get the current status from transform_processed_files to handle state transitions
    let previous_status = embedded_datasets::get_batch_previous_status(
        &ctx.pool,
        result.embedded_dataset_id,
        &result.batch_file_key,
    )
    .await;

    match result.status.as_str() {
        "processing" => {
            handle_processing_status(&result, &embedded_dataset, previous_status.as_deref(), ctx)
                .await;
        }
        "failed" => {
            handle_failed_status(&result, &embedded_dataset, previous_status.as_deref(), ctx).await;
        }
        "success" => {
            handle_success_status(&result, &embedded_dataset, previous_status.as_deref(), ctx)
                .await;
        }
        _ => {
            error!(
                "Unknown status '{}' for batch {}",
                result.status, result.batch_file_key
            );
        }
    }
}

async fn handle_processing_status(
    result: &DatasetTransformResult,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    previous_status: Option<&str>,
    ctx: &DatasetListenerContext,
) {
    // Start a transaction for atomic updates
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to begin transaction: {}", e);
            return;
        }
    };

    // Record that processing has started
    info!(
        "Marking batch as processing: {} (ed_id={}, chunks={})",
        result.batch_file_key, result.embedded_dataset_id, result.chunk_count
    );
    if let Err(e) = embedded_datasets::record_processed_batch(
        &ctx.pool,
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
        return;
    }

    // Record in dataset_transform_batches for tracking at transform level
    if let Err(e) = dataset_transform_batches::create_batch(
        &ctx.pool,
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
        return;
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
        return;
    }

    if let Err(e) = tx.commit().await {
        error!("Failed to commit transaction: {}", e);
    }
}

async fn handle_failed_status(
    result: &DatasetTransformResult,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    previous_status: Option<&str>,
    ctx: &DatasetListenerContext,
) {
    // Start a transaction for atomic updates
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to begin transaction: {}", e);
            return;
        }
    };

    error!(
        "Dataset transform batch failed for {}: {:?}",
        result.batch_file_key, result.error
    );

    // Clone error before using it multiple times
    let error_msg = result
        .error
        .clone()
        .unwrap_or_else(|| "Unknown error".to_string());

    if let Err(e) = embedded_datasets::record_processed_batch(
        &ctx.pool,
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
        return;
    }

    // Also update batch record at transform level
    if let Err(e) = dataset_transform_batches::update_batch_status(
        &ctx.pool,
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
        return;
    }

    // If transitioning from processing to failed, decrement processing and increment failed
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
        return;
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
        return;
    }

    if let Err(e) = tx.commit().await {
        error!("Failed to commit transaction: {}", e);
        return;
    }

    // Publish failed status for SSE streaming
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
}

async fn handle_success_status(
    result: &DatasetTransformResult,
    embedded_dataset: &crate::embedded_datasets::EmbeddedDataset,
    previous_status: Option<&str>,
    ctx: &DatasetListenerContext,
) {
    // Start a transaction for atomic updates
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to begin transaction: {}", e);
            return;
        }
    };

    info!(
        "Marking batch as completed: {} with {} chunks (ed_id={}, duration_ms={})",
        result.batch_file_key,
        result.chunk_count,
        result.embedded_dataset_id,
        result.processing_duration_ms.unwrap_or(0)
    );
    if let Err(e) = embedded_datasets::record_processed_batch(
        &ctx.pool,
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
        return;
    }

    // Also update batch record at transform level
    if let Err(e) = dataset_transform_batches::update_batch_status(
        &ctx.pool,
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
        return;
    }

    // If transitioning from processing to success, decrement processing
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
        return;
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
        return;
    }

    if let Err(e) = tx.commit().await {
        error!("Failed to commit transaction: {}", e);
        return;
    }

    // Clean up the batch file from S3 after successful processing and database recording
    // Use single-bucket architecture
    let s3_bucket =
        std::env::var("S3_BUCKET_NAME").unwrap_or_else(|_| "semantic-explorer-local".to_string());

    // Extract prefix and file key from batch_file_key
    // Format: embedded-datasets/embedded-dataset-{id}/batch-{uuid}.jsonl
    let parts: Vec<&str> = result.batch_file_key.rsplitn(2, '/').collect();
    let (prefix, file_key) = if parts.len() == 2 {
        (parts[1], parts[0])
    } else {
        ("", result.batch_file_key.as_str())
    };

    if let Err(e) = delete_file(&ctx.s3_client, &s3_bucket, prefix, file_key).await {
        // Log the error but don't fail the overall operation
        // The batch was successfully processed and recorded, cleanup failure is non-critical
        warn!(
            "Failed to cleanup batch file s3://{}/{}: {}. Manual cleanup may be required.",
            s3_bucket, result.batch_file_key, e
        );
    } else {
        info!(
            "Cleaned up batch file s3://{}/{}",
            s3_bucket, result.batch_file_key
        );
    }

    // Publish completed status for SSE streaming
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
        "Successfully processed dataset transform batch {} with {} chunks",
        result.batch_file_key, result.chunk_count
    );
}
