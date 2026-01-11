use crate::audit::{ResourceType, events};
use crate::auth::AuthenticatedUser;
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::{dataset_transforms, embedded_datasets};
use crate::transforms::dataset::models::{
    CreateDatasetTransform, DatasetTransform, DatasetTransformStats, UpdateDatasetTransform,
};
use semantic_explorer_core::models::PaginatedResponse;
use semantic_explorer_core::validation;

use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, patch, post};
use async_nats::Client as NatsClient;
use qdrant_client::Qdrant;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct SortParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_direction")]
    pub sort_direction: String,
}

fn default_limit() -> i64 {
    10
}

fn default_sort_by() -> String {
    "created_at".to_string()
}

fn default_sort_direction() -> String {
    "desc".to_string()
}

#[utoipa::path(
    get,
    path = "/api/dataset-transforms",
    tag = "Dataset Transforms",
    params(
        ("limit" = i64, Query, description = "Number of results per page", example = 10),
        ("offset" = i64, Query, description = "Number of results to skip", example = 0),
        ("sort_by" = String, Query, description = "Field to sort by: title, is_enabled, created_at, updated_at", example = "created_at"),
        ("sort_direction" = String, Query, description = "Sort direction: asc or desc", example = "desc"),
    ),
    responses(
        (status = 200, description = "Paginated list of dataset transforms", body = PaginatedResponse<DatasetTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/dataset-transforms")]
#[tracing::instrument(name = "get_dataset_transforms", skip(user, postgres_pool, params))]
pub async fn get_dataset_transforms(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    params: Query<SortParams>,
) -> impl Responder {
    match dataset_transforms::get_dataset_transforms_paginated(
        &postgres_pool,
        &user,
        params.limit,
        params.offset,
        &params.sort_by,
        &params.sort_direction,
    )
    .await
    {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to fetch dataset transforms: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch dataset transforms"
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/dataset-transforms/{id}",
    tag = "Dataset Transforms",
    params(
        ("id" = i32, Path, description = "Dataset Transform ID")
    ),
    responses(
        (status = 200, description = "Dataset transform details", body = DatasetTransform),
        (status = 404, description = "Dataset transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/dataset-transforms/{id}")]
#[tracing::instrument(name = "get_dataset_transform", skip(user, postgres_pool), fields(dataset_transform_id = %path.as_ref()))]
pub async fn get_dataset_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match dataset_transforms::get_dataset_transform(&postgres_pool, &user, id).await {
        Ok(transform) => {
            events::resource_read(&user, ResourceType::Transform, &id.to_string());
            HttpResponse::Ok().json(transform)
        }
        Err(e) => {
            error!("Dataset transform not found: {}", e);
            not_found(format!("Dataset transform not found: {}", e))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/dataset-transforms",
    tag = "Dataset Transforms",
    request_body = CreateDatasetTransform,
    responses(
        (status = 201, description = "Dataset transform created (with N embedded datasets)", body = DatasetTransform),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/dataset-transforms")]
#[tracing::instrument(name = "create_dataset_transform", skip(user, postgres_pool, nats_client, body, req), fields(title = %body.title, embedder_count = %body.embedder_ids.len()))]
pub async fn create_dataset_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    body: Json<CreateDatasetTransform>,
) -> impl Responder {
    // Validate input
    if let Err(e) = validation::validate_title(&body.title) {
        return bad_request(e);
    }
    if body.embedder_ids.is_empty() {
        return bad_request("At least one embedder must be specified");
    }

    let job_config = serde_json::json!({
        "embedding_batch_size": body.embedding_batch_size.unwrap_or(100),
        "wipe_collection": body.wipe_collection,
    });

    match dataset_transforms::create_dataset_transform(
        &postgres_pool,
        &body.title,
        body.source_dataset_id,
        &body.embedder_ids,
        &user,
        &job_config,
    )
    .await
    {
        Ok((transform, embedded_datasets)) => {
            // Enqueue scan as a background job instead of processing synchronously
            let dataset_transform_id = transform.dataset_transform_id;
            events::resource_created_with_request(
                &req,
                &user,
                ResourceType::Transform,
                &dataset_transform_id.to_string(),
            );
            let scan_job = semantic_explorer_core::models::DatasetTransformScanJob {
                job_id: Uuid::new_v4(),
                dataset_transform_id,
                owner: user.to_string(),
            };

            if let Ok(payload) = serde_json::to_vec(&scan_job) {
                // Use JetStream with message ID for deduplication
                let msg_id = format!("scan-{}", dataset_transform_id);
                let jetstream = async_nats::jetstream::new(nats_client.as_ref().clone());
                if let Err(e) = jetstream
                    .publish_with_headers(
                        "workers.dataset-transform-scan".to_string(),
                        {
                            let mut headers = async_nats::HeaderMap::new();
                            headers.insert("Nats-Msg-Id", msg_id.as_str());
                            headers
                        },
                        payload.into(),
                    )
                    .await
                {
                    error!(
                        "Failed to enqueue dataset transform scan for {}: {}",
                        dataset_transform_id, e
                    );
                    // Log but don't fail the response
                }
            }

            HttpResponse::Created().json(serde_json::json!({
                "transform": transform,
                "embedded_datasets": embedded_datasets,
                "message": format!("Created dataset transform with {} embedded datasets. Batches are being generated in the background.", embedded_datasets.len())
            }))
        }
        Err(e) => {
            error!("Failed to create dataset transform: {}", e);
            bad_request(format!("Failed to create dataset transform: {}", e))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/dataset-transforms/{id}",
    tag = "Dataset Transforms",
    params(
        ("id" = i32, Path, description = "Dataset Transform ID")
    ),
    request_body = UpdateDatasetTransform,
    responses(
        (status = 200, description = "Dataset transform updated", body = DatasetTransform),
        (status = 404, description = "Dataset transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[patch("/api/dataset-transforms/{id}")]
#[tracing::instrument(name = "update_dataset_transform", skip(user, postgres_pool, body), fields(dataset_transform_id = %path.as_ref()))]
pub async fn update_dataset_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateDatasetTransform>,
) -> impl Responder {
    // Validate input if title is provided
    if let Some(ref title) = body.title
        && let Err(e) = validation::validate_title(title)
    {
        return bad_request(e);
    }
    if let Some(ref embedder_ids) = body.embedder_ids
        && embedder_ids.is_empty()
    {
        return bad_request("At least one embedder must be specified");
    }

    let id = path.into_inner();
    match dataset_transforms::update_dataset_transform(
        &postgres_pool,
        &user,
        id,
        body.title.as_deref(),
        body.is_enabled,
        body.embedder_ids.as_deref(),
        body.job_config.as_ref(),
    )
    .await
    {
        Ok((transform, embedded_datasets)) => {
            events::resource_updated(&user, ResourceType::Transform, &id.to_string());
            HttpResponse::Ok().json(serde_json::json!({
                "transform": transform,
                "embedded_datasets": embedded_datasets,
                "message": format!("Updated dataset transform, now has {} embedded datasets", embedded_datasets.len())
            }))
        }
        Err(e) => {
            error!("Failed to update dataset transform: {}", e);
            not_found(format!("Failed to update dataset transform: {}", e))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/dataset-transforms/{id}",
    tag = "Dataset Transforms",
    params(
        ("id" = i32, Path, description = "Dataset Transform ID")
    ),
    responses(
        (status = 204, description = "Dataset transform deleted"),
        (status = 404, description = "Dataset transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[delete("/api/dataset-transforms/{id}")]
#[tracing::instrument(name = "delete_dataset_transform", skip(user, postgres_pool, qdrant_client, req), fields(dataset_transform_id = %path.as_ref()))]
pub async fn delete_dataset_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
) -> impl Responder {
    let dataset_transform_id = path.into_inner();

    // Get all embedded datasets for this transform so we can delete their Qdrant collections
    let embedded_datasets_list = match embedded_datasets::get_embedded_datasets_for_transform(
        &postgres_pool,
        dataset_transform_id,
    )
    .await
    {
        Ok(datasets) => datasets,
        Err(e) => {
            error!("Failed to fetch embedded datasets for deletion: {}", e);
            return not_found(format!("Failed to fetch embedded datasets: {}", e));
        }
    };

    // Delete Qdrant collections for all embedded datasets
    for embedded_dataset in embedded_datasets_list {
        if let Err(e) = qdrant_client
            .delete_collection(&embedded_dataset.collection_name)
            .await
        {
            error!(
                "Failed to delete Qdrant collection {} for embedded dataset {}: {}",
                embedded_dataset.collection_name, embedded_dataset.embedded_dataset_id, e
            );
            // Continue with other collections even if one fails
        }
    }

    match dataset_transforms::delete_dataset_transform(&postgres_pool, &user, dataset_transform_id)
        .await
    {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
                &user,
                ResourceType::Transform,
                &dataset_transform_id.to_string(),
            );
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            error!("Failed to delete dataset transform: {}", e);
            not_found(format!("Failed to delete dataset transform: {}", e))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/dataset-transforms/{id}/trigger",
    tag = "Dataset Transforms",
    params(
        ("id" = i32, Path, description = "Dataset Transform ID")
    ),
    responses(
        (status = 200, description = "Dataset transform triggered for all embedders"),
        (status = 404, description = "Dataset transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/dataset-transforms/{id}/trigger")]
#[tracing::instrument(name = "trigger_dataset_transform", skip(user, postgres_pool, nats_client, s3_client), fields(dataset_transform_id = %path.as_ref()))]
pub async fn trigger_dataset_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    s3_client: Data<aws_sdk_s3::Client>,
    path: Path<i32>,
) -> impl Responder {
    let dataset_transform_id = path.into_inner();

    // Verify the transform exists
    let transform = match dataset_transforms::get_dataset_transform(
        &postgres_pool,
        &user,
        dataset_transform_id,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            error!("Dataset transform not found: {}", e);
            return not_found(format!("Dataset transform not found: {}", e));
        }
    };

    // Actually trigger the scan
    if let Err(e) = crate::transforms::dataset::scanner::trigger_dataset_transform_scan(
        &postgres_pool,
        &nats_client,
        &s3_client,
        dataset_transform_id,
        &user,
    )
    .await
    {
        error!("Failed to trigger dataset transform scan: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to trigger dataset transform: {}", e)
        }));
    }

    info!(
        "Dataset transform {} triggered successfully",
        dataset_transform_id
    );
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Dataset transform triggered for all embedders",
        "dataset_transform_id": dataset_transform_id,
        "embedder_count": transform.embedder_ids.len()
    }))
}

#[utoipa::path(
    get,
    path = "/api/dataset-transforms/{id}/stats",
    tag = "Dataset Transforms",
    params(
        ("id" = i32, Path, description = "Dataset Transform ID")
    ),
    responses(
        (status = 200, description = "Aggregate statistics across all embedded datasets", body = DatasetTransformStats),
        (status = 404, description = "Dataset transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/dataset-transforms/{id}/stats")]
#[tracing::instrument(name = "get_dataset_transform_stats", skip(user, postgres_pool), fields(dataset_transform_id = %path.as_ref()))]
pub async fn get_dataset_transform_stats(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let dataset_transform_id = path.into_inner();

    match dataset_transforms::get_dataset_transform(&postgres_pool, &user, dataset_transform_id)
        .await
    {
        Ok(_) => {
            match dataset_transforms::get_dataset_transform_stats(
                &postgres_pool,
                dataset_transform_id,
            )
            .await
            {
                Ok(stats) => {
                    info!(
                        "Transform stats: batches={}, completed={}, processing={}, chunks_embedded={}, chunks_to_process={}",
                        stats.total_batches_processed,
                        stats.successful_batches,
                        stats.processing_batches,
                        stats.total_chunks_embedded,
                        stats.total_chunks_to_process
                    );
                    let response = serde_json::json!({
                        "dataset_transform_id": stats.dataset_transform_id,
                        "embedder_count": stats.embedder_count,
                        "total_batches_processed": stats.total_batches_processed,
                        "successful_batches": stats.successful_batches,
                        "failed_batches": stats.failed_batches,
                        "processing_batches": stats.processing_batches,
                        "total_chunks_embedded": stats.total_chunks_embedded,
                        "total_chunks_processing": stats.total_chunks_processing,
                        "total_chunks_failed": stats.total_chunks_failed,
                        "total_chunks_to_process": stats.total_chunks_to_process,
                        "status": stats.status(),
                        "is_processing": stats.is_processing(),
                        "last_run_at": stats.last_run_at,
                        "first_processing_at": stats.first_processing_at,
                    });
                    HttpResponse::Ok().json(response)
                }
                Err(e) => {
                    error!("Failed to get stats: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get stats: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Dataset transform not found: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Dataset transform not found: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/datasets/{dataset_id}/transforms",
    tag = "Dataset Transforms",
    params(
        ("dataset_id" = i32, Path, description = "Dataset ID")
    ),
    responses(
        (status = 200, description = "Dataset transforms for dataset", body = Vec<DatasetTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/datasets/{dataset_id}/transforms")]
#[tracing::instrument(name = "get_dataset_transforms_for_dataset", skip(user, postgres_pool), fields(dataset_id = %path.as_ref()))]
pub async fn get_dataset_transforms_for_dataset(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    match dataset_transforms::get_dataset_transforms_for_dataset(
        &postgres_pool,
        &user,
        path.into_inner(),
    )
    .await
    {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => {
            error!("Failed to fetch dataset transforms: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch dataset transforms: {}", e)
            }))
        }
    }
}
#[utoipa::path(
    get,
    path = "/api/dataset-transforms/{id}/detailed-stats",
    tag = "Dataset Transforms",
    params(
        ("id" = i32, Path, description = "Dataset Transform ID")
    ),
    responses(
        (status = 200, description = "Detailed statistics per embedded dataset"),
        (status = 404, description = "Dataset transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/dataset-transforms/{id}/detailed-stats")]
#[tracing::instrument(name = "get_dataset_transform_detailed_stats", skip(user, postgres_pool), fields(dataset_transform_id = %path.as_ref()))]
pub async fn get_dataset_transform_detailed_stats(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let dataset_transform_id = path.into_inner();

    // Verify the transform exists and user has access
    let transform = match dataset_transforms::get_dataset_transform(
        &postgres_pool,
        &user,
        dataset_transform_id,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            error!("Dataset transform not found: {}", e);
            return not_found(format!("Dataset transform not found: {}", e));
        }
    };

    // Get all embedded datasets for this transform
    let embedded_datasets_list = match embedded_datasets::get_embedded_datasets_for_transform(
        &postgres_pool,
        dataset_transform_id,
    )
    .await
    {
        Ok(eds) => eds,
        Err(e) => {
            error!("Failed to get embedded datasets: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get embedded datasets: {}", e)
            }));
        }
    };

    // Get stats for each embedded dataset
    let mut per_embedder_stats = Vec::new();
    for ed in &embedded_datasets_list {
        match embedded_datasets::get_embedded_dataset_stats(&postgres_pool, ed.embedded_dataset_id)
            .await
        {
            Ok(stats) => {
                per_embedder_stats.push(serde_json::json!({
                    "embedded_dataset_id": ed.embedded_dataset_id,
                    "embedder_id": ed.embedder_id,
                    "collection_name": ed.collection_name,
                    "title": ed.title,
                    "total_batches_processed": stats.total_batches_processed,
                    "successful_batches": stats.successful_batches,
                    "failed_batches": stats.failed_batches,
                    "processing_batches": stats.processing_batches,
                    "total_chunks_embedded": stats.total_chunks_embedded,
                    "total_chunks_failed": stats.total_chunks_failed,
                    "total_chunks_processing": stats.total_chunks_processing,
                    "last_run_at": stats.last_run_at,
                    "first_processing_at": stats.first_processing_at,
                    "avg_processing_duration_ms": stats.avg_processing_duration_ms,
                    "is_processing": stats.processing_batches > 0,
                }));
            }
            Err(e) => {
                error!(
                    "Failed to get stats for embedded dataset {}: {}",
                    ed.embedded_dataset_id, e
                );
                per_embedder_stats.push(serde_json::json!({
                    "embedded_dataset_id": ed.embedded_dataset_id,
                    "embedder_id": ed.embedder_id,
                    "collection_name": ed.collection_name,
                    "title": ed.title,
                    "error": format!("Failed to get stats: {}", e),
                }));
            }
        }
    }

    // Also get the aggregate stats
    let aggregate_stats =
        match dataset_transforms::get_dataset_transform_stats(&postgres_pool, dataset_transform_id)
            .await
        {
            Ok(s) => Some(serde_json::json!({
                "dataset_transform_id": s.dataset_transform_id,
                "embedder_count": s.embedder_count,
                "total_batches_processed": s.total_batches_processed,
                "successful_batches": s.successful_batches,
                "failed_batches": s.failed_batches,
                "processing_batches": s.processing_batches,
                "total_chunks_embedded": s.total_chunks_embedded,
                "total_chunks_processing": s.total_chunks_processing,
                "total_chunks_failed": s.total_chunks_failed,
                "total_chunks_to_process": s.total_chunks_to_process,
                "status": s.status(),
                "is_processing": s.is_processing(),
                "last_run_at": s.last_run_at,
                "first_processing_at": s.first_processing_at,
            })),
            Err(e) => {
                error!("Failed to get aggregate stats: {}", e);
                None
            }
        };

    HttpResponse::Ok().json(serde_json::json!({
        "dataset_transform_id": dataset_transform_id,
        "title": transform.title,
        "aggregate": aggregate_stats,
        "per_embedder": per_embedder_stats,
    }))
}
