use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

use semantic_explorer_core::jobs::{FileTransformResult, VectorBatchResult};
use semantic_explorer_core::storage::get_file;

use crate::datasets::models::ChunkWithMetadata;
use crate::storage::postgres::datasets;
use crate::storage::postgres::transforms::{
    get_transform_by_id, mark_file_failed, mark_file_processed,
};
use crate::transforms::ScanCollectionJob;

use super::scanner::{TransformContext, process_collection_scan, process_vector_scan};

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
    start_scan_job_listener(context.clone(), nats_client.clone());

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
            if let Ok(result) = serde_json::from_slice::<FileTransformResult>(&msg.payload) {
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
            if let Ok(result) = serde_json::from_slice::<VectorBatchResult>(&msg.payload) {
                handle_vector_result(result, &context).await;
            }
        }
    });
}

fn start_scan_job_listener(context: TransformContext, nats_client: NatsClient) {
    actix_web::rt::spawn(async move {
        let mut subscriber = match nats_client.subscribe("worker.scan".to_string()).await {
            Ok(sub) => sub,
            Err(e) => {
                error!("failed to subscribe to scan jobs: {}", e);
                return;
            }
        };

        while let Some(msg) = subscriber.next().await {
            if let Ok(job) = serde_json::from_slice::<ScanCollectionJob>(&msg.payload) {
                info!("Received scan job for transform {}", job.transform_id);
                match get_transform_by_id(&context.postgres_pool, job.transform_id).await {
                    Ok(transform) => {
                        // Route to appropriate handler based on job type
                        let result = match transform.job_type.as_str() {
                            "collection_to_dataset" => {
                                process_collection_scan(
                                    &context.postgres_pool,
                                    &nats_client,
                                    &context.s3_client,
                                    &transform,
                                )
                                .await
                            }
                            "dataset_to_vector_storage" => {
                                process_vector_scan(
                                    &context.postgres_pool,
                                    &nats_client,
                                    &context.s3_client,
                                    &transform,
                                )
                                .await
                            }
                            _ => {
                                error!("Unknown job type: {}", transform.job_type);
                                continue;
                            }
                        };

                        if let Err(e) = result {
                            error!(
                                "Failed to process scan for transform {}: {}",
                                job.transform_id, e
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to get transform {}: {}", job.transform_id, e);
                    }
                }
            }
        }
    });
}

#[tracing::instrument(name = "handle_file_result", skip(ctx))]
async fn handle_file_result(result: FileTransformResult, ctx: &TransformContext) {
    info!("Handling file result for: {}", result.source_file_key);
    if result.status != "success" {
        error!(
            "File transform failed for {}: {:?}",
            result.source_file_key, result.error
        );
        let _ = mark_file_failed(
            &ctx.postgres_pool,
            result.transform_id,
            &result.source_file_key,
            &result.error.unwrap_or_default(),
        )
        .await;
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

    let transform = match get_transform_by_id(&ctx.postgres_pool, result.transform_id).await {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to get transform {}: {}", result.transform_id, e);
            return;
        }
    };

    let chunk_count = chunks.len() as i32;
    let metadata = serde_json::json!({
        "source_file": result.source_file_key,
        "transform_id": result.transform_id,
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
        return;
    }

    info!(
        "Marking file as processed: {} with {} chunks",
        result.source_file_key, chunk_count
    );
    let _ = mark_file_processed(
        &ctx.postgres_pool,
        result.transform_id,
        &result.source_file_key,
        chunk_count,
        result.processing_duration_ms,
    )
    .await;

    info!(
        "Successfully processed file {} with {} chunks",
        result.source_file_key, chunk_count
    );
}

#[tracing::instrument(name = "handle_vector_result", skip(ctx))]
async fn handle_vector_result(result: VectorBatchResult, ctx: &TransformContext) {
    info!(
        "Handling vector batch result for: {}",
        result.batch_file_key
    );

    if result.status != "success" {
        error!(
            "Vector batch failed for {}: {:?}",
            result.batch_file_key, result.error
        );
        let _ = mark_file_failed(
            &ctx.postgres_pool,
            result.transform_id,
            &result.batch_file_key,
            &result.error.unwrap_or_default(),
        )
        .await;
        return;
    }

    info!(
        "Marking batch as processed: {} with {} chunks",
        result.batch_file_key, result.chunk_count
    );
    if let Err(e) = mark_file_processed(
        &ctx.postgres_pool,
        result.transform_id,
        &result.batch_file_key,
        result.chunk_count as i32,
        result.processing_duration_ms,
    )
    .await
    {
        error!("Failed to mark batch as processed: {}", e);
        return;
    }

    info!(
        "Successfully processed vector batch {} with {} chunks",
        result.batch_file_key, result.chunk_count
    );
}
