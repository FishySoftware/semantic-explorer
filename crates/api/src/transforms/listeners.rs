use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

use semantic_explorer_core::models::{
    CollectionTransformResult, DatasetTransformResult, VisualizationTransformResult,
};
use semantic_explorer_core::storage::get_file;

use crate::datasets::models::ChunkWithMetadata;
use crate::storage::postgres::collection_transforms;
use crate::storage::postgres::datasets;
use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::visualization_transforms::{
    update_visualization_transform_status_completed, update_visualization_transform_status_failed,
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
        match get_file(&ctx.s3_client, &result.bucket, &result.chunks_file_key).await {
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

    let transform = match collection_transforms::get_collection_transform_by_id(
        &ctx.postgres_pool,
        result.collection_transform_id,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            error!(
                "Failed to get collection transform {}: {}",
                result.collection_transform_id, e
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

    info!("Creating dataset item for: {}", result.source_file_key);
    if let Err(e) = datasets::create_dataset_item(
        &ctx.postgres_pool,
        transform.dataset_id,
        &result.source_file_key,
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
        "Handling visualization result for transform {}",
        result.visualization_transform_id
    );

    if result.status != "completed" {
        error!(
            "Visualization failed for transform {}: {:?}",
            result.visualization_transform_id, result.error
        );

        // Update database with failure status
        let error_message = result.error.unwrap_or_else(|| "Unknown error".to_string());
        if let Err(e) = update_visualization_transform_status_failed(
            &ctx.postgres_pool,
            result.visualization_transform_id,
            &error_message,
        )
        .await
        {
            error!(
                "Failed to update failure status for visualization transform {}: {}",
                result.visualization_transform_id, e
            );
        }
        return;
    }

    info!(
        "Visualization completed for transform {} with {} points in {} clusters (processing time: {}ms)",
        result.visualization_transform_id,
        result.n_points,
        result.n_clusters,
        result.processing_duration_ms.unwrap_or(0)
    );

    // Build stats JSON
    let stats = serde_json::json!({
        "n_points": result.n_points,
        "n_clusters": result.n_clusters,
        "processing_duration_ms": result.processing_duration_ms
    });

    // Update the database with collection names and success status
    if let Err(e) = update_visualization_transform_status_completed(
        &ctx.postgres_pool,
        result.visualization_transform_id,
        &result.output_collection_reduced,
        &result.output_collection_topics,
        &stats,
    )
    .await
    {
        error!(
            "Failed to update collection names and status for visualization transform {}: {}",
            result.visualization_transform_id, e
        );
        return;
    }

    info!(
        "Successfully completed visualization transform {} and updated collection names",
        result.visualization_transform_id
    );
}
