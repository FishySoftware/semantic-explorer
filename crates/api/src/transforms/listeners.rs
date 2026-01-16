use anyhow::Result;
use async_nats::{Client as NatsClient, jetstream};
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info, warn};

use semantic_explorer_core::models::{
    CollectionTransformResult, DatasetTransformResult, VisualizationTransformResult,
};
use semantic_explorer_core::storage::get_file_with_size_check;

use semantic_explorer_core::encryption::EncryptionService;

use crate::datasets::models::ChunkWithMetadata;
use crate::storage::postgres::collection_transforms;
use crate::storage::postgres::dataset_transform_batches::{self, CreateBatchRequest};
use crate::storage::postgres::datasets;
use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::visualization_transforms::{
    get_visualization, get_visualization_transform_by_id, update_visualization,
    update_visualization_transform_status,
};
use crate::storage::s3::delete_file;
use crate::transforms::dataset::scanner::trigger_dataset_transform_scan;

#[derive(Clone)]
struct TransformContext {
    postgres_pool: Pool<Postgres>,
    s3_client: S3Client,
    nats_client: NatsClient,
    encryption: EncryptionService,
}

/// Status update payload for SSE streams
#[derive(serde::Serialize)]
struct TransformStatusUpdate {
    /// The type of transform: "collection", "dataset", or "visualization"
    transform_type: String,
    /// The ID of the transform
    transform_id: i32,
    /// The ID of the related resource (collection_id, dataset_id, or embedded_dataset_id)
    resource_id: i32,
    /// Current status: "processing", "completed", "failed"
    status: String,
    /// Optional error message for failed status
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Timestamp of the status update
    timestamp: String,
}

/// Publish a transform status update to NATS for SSE streaming.
/// Subject format: sse.transforms.{type}.status.{owner}.{resource_id}.{transform_id}
/// NOTE: Uses sse. prefix to avoid being captured by JetStream TRANSFORM_STATUS stream,
/// which is used for worker results (CollectionTransformResult, etc.)
async fn publish_transform_status(
    nats: &NatsClient,
    transform_type: &str,
    owner: &str,
    resource_id: i32,
    transform_id: i32,
    status: &str,
    error: Option<&str>,
) {
    let subject = format!(
        "sse.transforms.{}.status.{}.{}.{}",
        transform_type, owner, resource_id, transform_id
    );

    let update = TransformStatusUpdate {
        transform_type: transform_type.to_string(),
        transform_id,
        resource_id,
        status: status.to_string(),
        error: error.map(|e| e.to_string()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    match serde_json::to_vec(&update) {
        Ok(payload) => {
            if let Err(e) = nats.publish(subject.clone(), payload.into()).await {
                warn!("Failed to publish transform status to {}: {}", subject, e);
            } else {
                info!("Published transform status to {}", subject);
            }
        }
        Err(e) => {
            error!("Failed to serialize transform status update: {}", e);
        }
    }
}

pub(crate) async fn start_result_listeners(
    postgres_pool: Pool<Postgres>,
    s3_client: S3Client,
    nats_client: NatsClient,
    encryption: EncryptionService,
) -> Result<()> {
    let context = TransformContext {
        postgres_pool: postgres_pool.clone(),
        s3_client: s3_client.clone(),
        nats_client: nats_client.clone(),
        encryption,
    };

    start_file_result_listener(context.clone(), nats_client.clone());
    start_vector_result_listener(context.clone(), nats_client.clone());
    start_visualization_result_listener(context.clone(), nats_client.clone());
    start_dataset_transform_scan_listener(context.clone(), nats_client.clone());

    Ok(())
}

fn start_file_result_listener(context: TransformContext, nats_client: NatsClient) {
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
            "File result listener started with durable consumer: {}",
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
                    handle_file_result(result, &context).await;
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

fn start_vector_result_listener(context: TransformContext, nats_client: NatsClient) {
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
            "Vector result listener started with durable consumer: {}",
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
                "Received dataset result message on subject: {}",
                msg.subject
            );
            match serde_json::from_slice::<DatasetTransformResult>(&msg.payload) {
                Ok(result) => {
                    handle_vector_result(result, &context).await;
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to deserialize dataset result: {}", e);
                    // Acknowledge bad messages to prevent reprocessing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to acknowledge bad message: {}", ack_err);
                    }
                }
            }
        }
    });
}

fn start_visualization_result_listener(context: TransformContext, nats_client: NatsClient) {
    actix_web::rt::spawn(async move {
        // Use JetStream durable consumer for reliable message delivery
        // Subject format: transforms.visualization.status.{owner}.{embedded_dataset_id}.{transform_id}
        let subject = "transforms.visualization.status.>";
        let stream_name = "TRANSFORM_STATUS";
        let consumer_name = "visualization-status-listener";

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
                    description: Some("Visualization transform status listener".to_string()),
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
            "Visualization result listener started with durable consumer: {}",
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
                "Received visualization result message on subject: {}",
                msg.subject
            );
            match serde_json::from_slice::<VisualizationTransformResult>(&msg.payload) {
                Ok(result) => {
                    handle_visualization_result(result, &context).await;
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                Err(e) => {
                    let payload_str = String::from_utf8_lossy(&msg.payload);
                    error!(
                        "Failed to deserialize visualization result: {}. Payload: {}",
                        e,
                        payload_str.chars().take(500).collect::<String>()
                    );
                    // Acknowledge bad messages to prevent reprocessing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to acknowledge bad message: {}", ack_err);
                    }
                }
            }
        }
    });
}

#[tracing::instrument(name = "handle_file_result", skip(ctx))]
async fn handle_file_result(result: CollectionTransformResult, ctx: &TransformContext) {
    info!("Handling file result for: {}", result.source_file_key);

    // Fetch the transform first to get collection_id for status updates
    let transform = match collection_transforms::get_collection_transform(
        &ctx.postgres_pool,
        &result.owner_id,
        result.collection_transform_id,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            error!(
                "Failed to get collection transform {} for owner {}: {}",
                result.collection_transform_id, result.owner_id, e
            );
            return;
        }
    };

    // Check if this file was already successfully processed to avoid duplicates
    match collection_transforms::is_file_already_processed(
        &ctx.postgres_pool,
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
            &ctx.postgres_pool,
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
            &ctx.postgres_pool,
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
            &ctx.postgres_pool,
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

    let s3_bucket_name = match std::env::var("S3_BUCKET_NAME") {
        Ok(bucket) => bucket,
        Err(_) => {
            error!("S3_BUCKET_NAME environment variable not set");
            return;
        }
    };

    let full_chunks_key = format!("collections/{}/{}", result.bucket, result.chunks_file_key);

    let chunks_content =
        match get_file_with_size_check(&ctx.s3_client, &s3_bucket_name, &full_chunks_key).await {
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
        &ctx.postgres_pool,
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
            &ctx.postgres_pool,
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
        &ctx.postgres_pool,
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

#[tracing::instrument(name = "handle_vector_result", skip(ctx))]
async fn handle_vector_result(result: DatasetTransformResult, ctx: &TransformContext) {
    info!(
        "Handling vector batch result for: {} (status: {})",
        result.batch_file_key, result.status
    );

    // Validate ownership by fetching the embedded dataset
    let embedded_dataset = match embedded_datasets::get_embedded_dataset(
        &ctx.postgres_pool,
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

    match result.status.as_str() {
        "processing" => {
            // Record that processing has started
            info!(
                "Marking batch as processing: {} (ed_id={}, chunks={})",
                result.batch_file_key, result.embedded_dataset_id, result.chunk_count
            );
            if let Err(e) = embedded_datasets::record_processed_batch(
                &ctx.postgres_pool,
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
            }

            // Also record in dataset_transform_batches for tracking at transform level
            if let Err(e) = dataset_transform_batches::create_batch(
                &ctx.postgres_pool,
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
            }
        }
        "failed" => {
            error!(
                "Vector batch failed for {}: {:?}",
                result.batch_file_key, result.error
            );

            // Clone error before using it multiple times
            let error_msg = result
                .error
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            if let Err(e) = embedded_datasets::record_processed_batch(
                &ctx.postgres_pool,
                result.embedded_dataset_id,
                &result.batch_file_key,
                0,
                "failed",
                Some(&error_msg),
                result.processing_duration_ms,
            )
            .await
            {
                error!("Failed to record batch processing failure: {}", e);
            }

            // Also update batch record at transform level
            if let Err(e) = dataset_transform_batches::update_batch_status(
                &ctx.postgres_pool,
                embedded_dataset.dataset_transform_id,
                &result.batch_file_key,
                "failed",
                Some(&error_msg),
                result.processing_duration_ms,
                0,
            )
            .await
            {
                error!("Failed to update dataset transform batch status: {}", e);
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
        "success" => {
            info!(
                "Marking batch as completed: {} with {} chunks (ed_id={}, duration_ms={})",
                result.batch_file_key,
                result.chunk_count,
                result.embedded_dataset_id,
                result.processing_duration_ms.unwrap_or(0)
            );
            if let Err(e) = embedded_datasets::record_processed_batch(
                &ctx.postgres_pool,
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
                return;
            }

            // Also update batch record at transform level
            if let Err(e) = dataset_transform_batches::update_batch_status(
                &ctx.postgres_pool,
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
            }

            // Clean up the batch file from S3 after successful processing and database recording
            // The bucket name is derived from the embedded dataset ID (S3-safe)
            let bucket = format!("embedded-dataset-{}", embedded_dataset.embedded_dataset_id);
            if let Err(e) = delete_file(&ctx.s3_client, &bucket, &result.batch_file_key).await {
                // Log the error but don't fail the overall operation
                // The batch was successfully processed and recorded, cleanup failure is non-critical
                warn!(
                    "Failed to cleanup batch file {} from bucket {}: {}. Manual cleanup may be required.",
                    result.batch_file_key, bucket, e
                );
            } else {
                info!(
                    "Cleaned up batch file {} from bucket {}",
                    result.batch_file_key, bucket
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
                "Successfully processed vector batch {} with {} chunks",
                result.batch_file_key, result.chunk_count
            );
        }
        _ => {
            error!(
                "Unknown status '{}' for batch {}",
                result.status, result.batch_file_key
            );
        }
    }
}

#[tracing::instrument(name = "handle_visualization_result", skip(ctx))]
async fn handle_visualization_result(result: VisualizationTransformResult, ctx: &TransformContext) {
    info!(
        "Handling visualization result for transform {} visualization {} (status: {})",
        result.visualization_transform_id, result.visualization_id, result.status
    );

    // Verify visualization exists
    if let Err(e) = get_visualization(&ctx.postgres_pool, result.visualization_id).await {
        error!("Visualization {} not found: {}", result.visualization_id, e);
        return;
    }

    // Fetch the transform to get embedded_dataset_id for status updates
    let visualization_transform = match get_visualization_transform_by_id(
        &ctx.postgres_pool,
        result.visualization_transform_id,
    )
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            error!(
                "Visualization transform {} not found",
                result.visualization_transform_id
            );
            return;
        }
        Err(e) => {
            error!(
                "Failed to get visualization transform {}: {}",
                result.visualization_transform_id, e
            );
            return;
        }
    };

    let now = sqlx::types::chrono::Utc::now();

    // Handle progress updates differently from final results
    if result.status == "processing" {
        // This is a progress update - merge progress info into stats
        let stats = result.stats_json.clone().unwrap_or_default();

        // Update visualization with progress info (keep existing data, just update stats)
        if let Err(e) = update_visualization(
            &ctx.postgres_pool,
            result.visualization_id,
            Some("processing"),
            None, // Don't override started_at
            None, // Don't set completed_at yet
            None, // Don't override html_s3_key
            None, // Don't override point_count
            None, // Don't override cluster_count
            None, // Don't override error_message
            Some(&stats),
        )
        .await
        {
            error!(
                "Failed to update visualization {} progress: {}",
                result.visualization_id, e
            );
        } else {
            tracing::debug!(
                "Updated visualization {} progress: {:?}",
                result.visualization_id,
                stats
            );
        }

        // Also update the transform's status so the UI can show progress
        if let Err(e) = update_visualization_transform_status(
            &ctx.postgres_pool,
            result.visualization_transform_id,
            Some("processing"),
            Some(now),
            None,
            Some(&stats),
        )
        .await
        {
            error!(
                "Failed to update visualization transform {} status: {}",
                result.visualization_transform_id, e
            );
        }

        return;
    }

    // Handle completed or failed status
    let status = if result.status == "completed" {
        "completed"
    } else {
        "failed"
    };

    let error_message = result.error_message.clone();

    // Build stats JSON - include any existing stats plus the final results
    let mut stats = result.stats_json.unwrap_or_default();
    if let Some(point_count) = result.point_count {
        stats["point_count"] = serde_json::json!(point_count);
    }
    if let Some(cluster_count) = result.cluster_count {
        stats["cluster_count"] = serde_json::json!(cluster_count);
    }
    if let Some(duration) = result.processing_duration_ms {
        stats["processing_duration_ms"] = serde_json::json!(duration);
    }

    // Update visualization record with results
    if let Err(e) = update_visualization(
        &ctx.postgres_pool,
        result.visualization_id,
        Some(status),
        Some(now),
        Some(now),
        result.html_s3_key.as_deref(),
        result.point_count.map(|p| p as i32),
        result.cluster_count,
        error_message.as_deref(),
        Some(&stats),
    )
    .await
    {
        error!(
            "Failed to update visualization {}: {}",
            result.visualization_id, e
        );
        return;
    }

    // Also update the transform's status
    if let Err(e) = update_visualization_transform_status(
        &ctx.postgres_pool,
        result.visualization_transform_id,
        Some(status),
        Some(now),
        error_message.as_deref(),
        Some(&stats),
    )
    .await
    {
        error!(
            "Failed to update visualization transform {} status: {}",
            result.visualization_transform_id, e
        );
    }

    // Publish status update for SSE streaming
    publish_transform_status(
        &ctx.nats_client,
        "visualization",
        &result.owner_id,
        visualization_transform.embedded_dataset_id,
        result.visualization_transform_id,
        status,
        error_message.as_deref(),
    )
    .await;

    if status == "completed" {
        info!(
            "Successfully completed visualization {} with {} points in {} clusters (processing time: {}ms)",
            result.visualization_id,
            result.point_count.unwrap_or(0),
            result.cluster_count.unwrap_or(0),
            result.processing_duration_ms.unwrap_or(0)
        );
    } else {
        error!(
            "Visualization {} failed: {}",
            result.visualization_id,
            error_message.unwrap_or_else(|| "Unknown error".to_string())
        );
    }
}

fn start_dataset_transform_scan_listener(context: TransformContext, nats_client: NatsClient) {
    actix_web::rt::spawn(async move {
        let mut subscriber = match nats_client
            .subscribe("workers.dataset-transform-scan".to_string())
            .await
        {
            Ok(sub) => sub,
            Err(e) => {
                error!("Failed to subscribe to dataset transform scan jobs: {}", e);
                return;
            }
        };

        info!("Started dataset transform scan listener");

        while let Some(message) = subscriber.next().await {
            let payload = message.payload.to_vec();
            let scan_job: Result<semantic_explorer_core::models::DatasetTransformScanJob, _> =
                serde_json::from_slice(&payload);

            match scan_job {
                Ok(job) => {
                    info!(
                        "Received dataset transform scan job {} for transform {}",
                        job.job_id, job.dataset_transform_id
                    );

                    // Spawn the actual processing in a separate task to avoid blocking the listener
                    let postgres_pool = context.postgres_pool.clone();
                    let s3_client = context.s3_client.clone();
                    let nats = nats_client.clone();
                    let encryption = context.encryption.clone();
                    let owner = job.owner_id.clone();
                    let dataset_transform_id = job.dataset_transform_id;

                    actix_web::rt::spawn(async move {
                        if let Err(e) = trigger_dataset_transform_scan(
                            &postgres_pool,
                            &nats,
                            &s3_client,
                            dataset_transform_id,
                            &owner,
                            &encryption,
                        )
                        .await
                        {
                            error!(
                                "Failed to process dataset transform scan for {}: {}",
                                dataset_transform_id, e
                            );
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to deserialize dataset transform scan job: {}", e);
                }
            }
        }
    });
}
