//! Collection transform result listener
//!
//! Handles results from collection transform workers (file processing).

use async_nats::{Client as NatsClient, jetstream};
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info, warn};

use crate::datasets::models::ChunkWithMetadata;
use crate::storage::postgres::{collection_transforms, datasets};
use semantic_explorer_core::models::CollectionTransformResult;
use semantic_explorer_core::storage::{delete_file_by_key, get_file_with_size_check};

use super::super::listeners::publish_transform_status;

/// Context for transform result handling
#[derive(Clone)]
pub(crate) struct CollectionListenerContext {
    pub pool: Pool<Postgres>,
    pub s3_client: S3Client,
    pub s3_bucket_name: String,
    pub nats_client: NatsClient,
}

/// Start the collection transform result listener
pub(crate) fn start(context: CollectionListenerContext) {
    let nats_client = context.nats_client.clone();

    actix_web::rt::spawn(async move {
        // Use JetStream durable consumer for reliable message delivery
        // Subject format: transforms.collection.status.{owner}.{collection_id}.{transform_id}
        let subject = "transforms.collection.status.>";
        let stream_name = "TRANSFORM_STATUS";
        let consumer_name = "collection-status-listener";

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
                    description: Some("Collection transform status listener".to_string()),
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
            "Collection result listener started with durable consumer: {}",
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

            info!("Received file result message on subject: {}", msg.subject);
            match serde_json::from_slice::<CollectionTransformResult>(&msg.payload) {
                Ok(result) => {
                    handle_result(result, &context).await;
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to deserialize file result: {}", e);
                    // Acknowledge bad messages to prevent reprocessing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to acknowledge bad message: {}", ack_err);
                    }
                }
            }
        }
    });
}

async fn handle_result(result: CollectionTransformResult, ctx: &CollectionListenerContext) {
    info!("Handling file result for: {}", result.source_file_key);

    // Fetch the transform first to get collection_id for status updates
    let transform = match collection_transforms::get_collection_transform(
        &ctx.pool,
        &result.owner_id,
        result.collection_transform_id,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            // Transform was deleted while job was in progress - this is expected when
            // a collection or dataset is deleted. Log at info level and clean up.
            info!(
                "Collection transform {} not found (likely deleted), discarding result for file {}: {}",
                result.collection_transform_id, result.source_file_key, e
            );
            // Clean up any S3 artifacts from the worker
            cleanup_orphaned_chunks(ctx, &result).await;
            return;
        }
    };

    // Check if this file was already successfully processed to avoid duplicates
    match collection_transforms::is_file_already_processed(
        &ctx.pool,
        result.collection_transform_id,
        &result.source_file_key,
    )
    .await
    {
        Ok(true) => {
            info!(
                "File {} was already successfully processed, skipping to avoid duplicates",
                result.source_file_key
            );
            return;
        }
        Ok(false) => {
            // File not yet processed, continue
        }
        Err(e) => {
            error!(
                "Failed to check if file {} was already processed: {}. Proceeding with caution.",
                result.source_file_key, e
            );
            // Continue anyway - better to risk a duplicate than lose data
        }
    }

    if result.status != "success" {
        let error_msg = result.error.clone().unwrap_or_default();
        error!(
            "File transform failed for {}: {:?}",
            result.source_file_key, result.error
        );
        if let Err(e) = collection_transforms::record_processed_file(
            &ctx.pool,
            result.collection_transform_id,
            &result.source_file_key,
            0,
            "failed",
            Some(&error_msg),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record file processing failure: {}", e);
        }

        // Publish failed status for SSE streaming
        publish_transform_status(
            &ctx.nats_client,
            "collection",
            &result.owner_id,
            transform.collection_id,
            result.collection_transform_id,
            "failed",
            Some(&error_msg),
        )
        .await;

        return;
    }

    // Use the chunk count directly from the worker - it already verified the chunks exist
    let chunk_count = result.chunk_count as i32;

    // Validate that the worker reported at least one chunk
    if chunk_count == 0 {
        error!(
            "Worker reported 0 chunks for {}: text extraction likely failed or resulted in empty content",
            result.source_file_key
        );
        if let Err(e) = collection_transforms::record_processed_file(
            &ctx.pool,
            result.collection_transform_id,
            &result.source_file_key,
            0,
            "failed",
            Some("No chunks generated - text extraction may have failed or produced empty content"),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record file processing failure: {}", e);
        }

        // Publish failed status for SSE streaming
        publish_transform_status(
            &ctx.nats_client,
            "collection",
            &result.owner_id,
            transform.collection_id,
            result.collection_transform_id,
            "failed",
            Some("No chunks generated"),
        )
        .await;

        return;
    }

    // Validate that the file key (title) is not empty or whitespace-only
    let title = result.source_file_key.trim();
    if title.is_empty() {
        error!("File key is empty or contains only whitespace, cannot create dataset item");
        if let Err(e) = collection_transforms::record_processed_file(
            &ctx.pool,
            result.collection_transform_id,
            &result.source_file_key,
            0,
            "failed",
            Some("File title cannot be empty or contain only whitespace"),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record file processing failure: {}", e);
        }

        // Publish failed status for SSE streaming
        publish_transform_status(
            &ctx.nats_client,
            "collection",
            &result.owner_id,
            transform.collection_id,
            result.collection_transform_id,
            "failed",
            Some("Empty file title"),
        )
        .await;

        return;
    }

    info!("Downloading chunks for: {}", result.source_file_key);

    let full_chunks_key = format!(
        "transforms/collection-transforms/{}/{}",
        result.collection_transform_id, result.chunks_file_key
    );

    let chunks_content =
        match get_file_with_size_check(&ctx.s3_client, &ctx.s3_bucket_name, &full_chunks_key).await
        {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "Failed to download chunks for {} from {}: {}",
                    result.source_file_key, full_chunks_key, e
                );
                return;
            }
        };

    let chunks: Vec<ChunkWithMetadata> = match serde_json::from_slice(&chunks_content) {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Failed to parse chunks for {}: {}",
                result.source_file_key, e
            );
            return;
        }
    };

    let metadata = serde_json::json!({
        "source_file": result.source_file_key,
        "collection_transform_id": result.collection_transform_id,
        "chunk_count": chunk_count,
    });

    info!("Creating dataset item for: {}", title);
    if let Err(e) = datasets::create_dataset_item(
        &ctx.pool,
        &result.owner_id,
        transform.dataset_id,
        title,
        &chunks,
        metadata,
    )
    .await
    {
        let error_msg = format!("Failed to create dataset item: {}", e);
        error!("{}", error_msg);
        if let Err(e) = collection_transforms::record_processed_file(
            &ctx.pool,
            result.collection_transform_id,
            &result.source_file_key,
            0,
            "failed",
            Some(&error_msg),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record file processing failure: {}", e);
        }

        // Publish failed status for SSE streaming
        publish_transform_status(
            &ctx.nats_client,
            "collection",
            &result.owner_id,
            transform.collection_id,
            result.collection_transform_id,
            "failed",
            Some(&error_msg),
        )
        .await;

        return;
    }

    info!(
        "Marking file as processed: {} with {} chunks",
        result.source_file_key, chunk_count
    );
    if let Err(e) = collection_transforms::record_processed_file(
        &ctx.pool,
        result.collection_transform_id,
        &result.source_file_key,
        chunk_count,
        "completed",
        None,
        result.processing_duration_ms,
    )
    .await
    {
        error!("Failed to record file processing: {}", e);
    }

    // Publish status update for SSE streaming
    publish_transform_status(
        &ctx.nats_client,
        "collection",
        &result.owner_id,
        transform.collection_id,
        result.collection_transform_id,
        "completed",
        None,
    )
    .await;

    info!(
        "Successfully processed file {} with {} chunks",
        result.source_file_key, chunk_count
    );
}

/// Clean up orphaned chunk files from S3 when a transform has been deleted.
/// This prevents accumulating orphan files when collections/datasets are deleted
/// while transforms are still processing.
async fn cleanup_orphaned_chunks(
    ctx: &CollectionListenerContext,
    result: &CollectionTransformResult,
) {
    if result.chunks_file_key.is_empty() {
        return;
    }

    let full_chunks_key = format!(
        "transforms/collection-transforms/{}/{}",
        result.collection_transform_id, result.chunks_file_key
    );

    // Best-effort cleanup - don't fail if this doesn't work
    if let Err(e) = delete_file_by_key(&ctx.s3_client, &ctx.s3_bucket_name, &full_chunks_key).await
    {
        warn!(
            "Failed to clean up orphaned chunks file {}: {} (non-critical)",
            full_chunks_key, e
        );
    } else {
        info!("Cleaned up orphaned chunks file: {}", full_chunks_key);
    }
}
