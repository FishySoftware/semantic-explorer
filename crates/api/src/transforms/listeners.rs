use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

use semantic_explorer_core::models::{
    CollectionTransformResult, DatasetTransformResult, VisualizationTransformResult,
};
use semantic_explorer_core::storage::get_file_with_size_check;

use crate::datasets::models::ChunkWithMetadata;
use crate::storage::postgres::collection_transforms;
use crate::storage::postgres::datasets;
use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::visualization_transforms::{
    get_visualization_run, update_visualization_run,
};

#[derive(Clone)]
struct TransformContext {
    postgres_pool: Pool<Postgres>,
    s3_client: S3Client,
}

pub(crate) async fn start_result_listeners(
    postgres_pool: Pool<Postgres>,
    s3_client: S3Client,
    nats_client: NatsClient,
) -> Result<()> {
    let context = TransformContext {
        postgres_pool: postgres_pool.clone(),
        s3_client: s3_client.clone(),
    };

    start_file_result_listener(context.clone(), nats_client.clone());
    start_vector_result_listener(context.clone(), nats_client.clone());
    start_visualization_result_listener(context.clone(), nats_client.clone());

    Ok(())
}

fn start_file_result_listener(context: TransformContext, nats_client: NatsClient) {
    actix_web::rt::spawn(async move {
        let mut subscriber = match nats_client
            .subscribe("worker.result.file".to_string())
            .await
        {
            Ok(sub) => sub,
            Err(e) => {
                error!("failed to subscribe to file results: {}", e);
                return;
            }
        };

        while let Some(msg) = subscriber.next().await {
            info!("Received file result message");
            if let Ok(result) = serde_json::from_slice::<CollectionTransformResult>(&msg.payload) {
                handle_file_result(result, &context).await;
            } else {
                error!("Failed to deserialize file result");
            }
        }
    });
}

fn start_vector_result_listener(context: TransformContext, nats_client: NatsClient) {
    actix_web::rt::spawn(async move {
        let mut subscriber = match nats_client
            .subscribe("worker.result.vector".to_string())
            .await
        {
            Ok(sub) => sub,
            Err(e) => {
                error!("failed to subscribe to vector results: {}", e);
                return;
            }
        };

        while let Some(msg) = subscriber.next().await {
            if let Ok(result) = serde_json::from_slice::<DatasetTransformResult>(&msg.payload) {
                handle_vector_result(result, &context).await;
            }
        }
    });
}

fn start_visualization_result_listener(context: TransformContext, nats_client: NatsClient) {
    actix_web::rt::spawn(async move {
        let mut subscriber = match nats_client
            .subscribe("worker.result.visualization".to_string())
            .await
        {
            Ok(sub) => sub,
            Err(e) => {
                error!("failed to subscribe to visualization results: {}", e);
                return;
            }
        };

        info!("Visualization result listener started");
        while let Some(msg) = subscriber.next().await {
            if let Ok(result) = serde_json::from_slice::<VisualizationTransformResult>(&msg.payload)
            {
                handle_visualization_result(result, &context).await;
            } else {
                error!("Failed to deserialize visualization result");
            }
        }
    });
}

#[tracing::instrument(name = "handle_file_result", skip(ctx))]
async fn handle_file_result(result: CollectionTransformResult, ctx: &TransformContext) {
    info!("Handling file result for: {}", result.source_file_key);

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
            Some(&result.error.unwrap_or_default()),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record file processing failure: {}", e);
        }
        return;
    }

    info!("Downloading chunks for: {}", result.source_file_key);
    let chunks_content =
        match get_file_with_size_check(&ctx.s3_client, &result.bucket, &result.chunks_file_key)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "Failed to download chunks for {}: {}",
                    result.source_file_key, e
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

    let transform = match collection_transforms::get_collection_transform(
        &ctx.postgres_pool,
        &result.owner,
        result.collection_transform_id,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            error!(
                "Failed to get collection transform {} for owner {}: {}",
                result.collection_transform_id, result.owner, e
            );
            return;
        }
    };

    let chunk_count = chunks.len() as i32;

    // Validate that we have at least one chunk
    if chunk_count == 0 {
        error!(
            "File extracted to 0 chunks for {}: text extraction likely failed or resulted in empty content",
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
        return;
    }

    let metadata = serde_json::json!({
        "source_file": result.source_file_key,
        "collection_transform_id": result.collection_transform_id,
        "chunk_count": chunk_count,
    });

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
        return;
    }

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
        error!("Failed to create dataset item: {}", e);
        if let Err(e) = collection_transforms::record_processed_file(
            &ctx.postgres_pool,
            result.collection_transform_id,
            &result.source_file_key,
            0,
            "failed",
            Some(&format!("Failed to create dataset item: {}", e)),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record file processing failure: {}", e);
        }
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

    info!(
        "Successfully processed file {} with {} chunks",
        result.source_file_key, chunk_count
    );
}

#[tracing::instrument(name = "handle_vector_result", skip(ctx))]
async fn handle_vector_result(result: DatasetTransformResult, ctx: &TransformContext) {
    info!(
        "Handling vector batch result for: {}",
        result.batch_file_key
    );

    // Validate ownership by fetching the embedded dataset
    if let Err(e) = embedded_datasets::get_embedded_dataset(
        &ctx.postgres_pool,
        &result.owner,
        result.embedded_dataset_id,
    )
    .await
    {
        error!(
            "Embedded dataset {} not found or access denied for owner {}: {}",
            result.embedded_dataset_id, result.owner, e
        );
        return;
    }

    if result.status != "success" {
        error!(
            "Vector batch failed for {}: {:?}",
            result.batch_file_key, result.error
        );
        if let Err(e) = embedded_datasets::record_processed_batch(
            &ctx.postgres_pool,
            result.embedded_dataset_id,
            &result.batch_file_key,
            0,
            "failed",
            Some(&result.error.unwrap_or_default()),
            result.processing_duration_ms,
        )
        .await
        {
            error!("Failed to record batch processing failure: {}", e);
        }
        return;
    }

    info!(
        "Marking batch as processed: {} with {} chunks",
        result.batch_file_key, result.chunk_count
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

    info!(
        "Successfully processed vector batch {} with {} chunks",
        result.batch_file_key, result.chunk_count
    );
}

#[tracing::instrument(name = "handle_visualization_result", skip(ctx))]
async fn handle_visualization_result(result: VisualizationTransformResult, ctx: &TransformContext) {
    info!(
        "Handling visualization result for transform {} run {}",
        result.visualization_transform_id, result.run_id
    );

    // Verify run exists
    if let Err(e) = get_visualization_run(&ctx.postgres_pool, result.run_id).await {
        error!("Visualization run {} not found: {}", result.run_id, e);
        return;
    }

    // Determine status and update run record
    let status = if result.status == "completed" {
        "completed"
    } else {
        "failed"
    };

    let error_message = result.error_message.clone();

    // Build stats JSON
    let stats = serde_json::json!({
        "point_count": result.point_count,
        "cluster_count": result.cluster_count,
        "processing_duration_ms": result.processing_duration_ms
    });

    // Update run record with results
    let now = sqlx::types::chrono::Utc::now();
    if let Err(e) = update_visualization_run(
        &ctx.postgres_pool,
        result.run_id,
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
            "Failed to update visualization run {}: {}",
            result.run_id, e
        );
        return;
    }

    if status == "completed" {
        info!(
            "Successfully completed visualization run {} with {} points in {} clusters (processing time: {}ms)",
            result.run_id,
            result.point_count.unwrap_or(0),
            result.cluster_count.unwrap_or(0),
            result.processing_duration_ms.unwrap_or(0)
        );
    } else {
        error!(
            "Visualization run {} failed: {}",
            result.run_id,
            error_message.unwrap_or_else(|| "Unknown error".to_string())
        );
    }
}
