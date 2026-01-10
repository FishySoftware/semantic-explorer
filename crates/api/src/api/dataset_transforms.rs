use crate::auth::extract_username;
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::{dataset_transforms, embedded_datasets};
use crate::transforms::dataset::models::{CreateDatasetTransform, DatasetTransform, DatasetTransformStats, UpdateDatasetTransform};


use actix_web::web::{Data, Json, Path};
use actix_web::{HttpResponse, Responder, delete, get, patch, post};
use actix_web_openidconnect::openid_middleware::Authenticated;
use async_nats::Client as NatsClient;
use qdrant_client::Qdrant;
use sqlx::{Pool, Postgres};
use tracing::{error, info};
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/api/dataset-transforms",
    tag = "Dataset Transforms",
    responses(
        (status = 200, description = "List of dataset transforms", body = Vec<DatasetTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/dataset-transforms")]
#[tracing::instrument(name = "get_dataset_transforms", skip(auth, postgres_pool))]
pub async fn get_dataset_transforms(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match dataset_transforms::get_dataset_transforms(&postgres_pool, &username).await {
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
#[tracing::instrument(name = "get_dataset_transform", skip(auth, postgres_pool), fields(dataset_transform_id = %path.as_ref()))]
pub async fn get_dataset_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match dataset_transforms::get_dataset_transform(&postgres_pool, &username, path.into_inner())
        .await
    {
        Ok(transform) => HttpResponse::Ok().json(transform),
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
#[tracing::instrument(name = "create_dataset_transform", skip(auth, postgres_pool, nats_client, body), fields(title = %body.title, embedder_count = %body.embedder_ids.len()))]
pub async fn create_dataset_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    body: Json<CreateDatasetTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

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
        &username,
        &job_config,
    )
    .await
    {
        Ok((transform, embedded_datasets)) => {
            // Enqueue scan as a background job instead of processing synchronously
            let dataset_transform_id = transform.dataset_transform_id;
            let scan_job = semantic_explorer_core::models::DatasetTransformScanJob {
                job_id: Uuid::new_v4(),
                dataset_transform_id,
                owner: username.clone(),
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
#[tracing::instrument(name = "update_dataset_transform", skip(auth, postgres_pool, body), fields(dataset_transform_id = %path.as_ref()))]
pub async fn update_dataset_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateDatasetTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    if let Some(ref embedder_ids) = body.embedder_ids
        && embedder_ids.is_empty()
    {
        return bad_request("At least one embedder must be specified");
    }

    match dataset_transforms::update_dataset_transform(
        &postgres_pool,
        &username,
        path.into_inner(),
        body.title.as_deref(),
        body.is_enabled,
        body.embedder_ids.as_deref(),
        body.job_config.as_ref(),
    )
    .await
    {
        Ok((transform, embedded_datasets)) => {
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
#[tracing::instrument(name = "delete_dataset_transform", skip(auth, postgres_pool, qdrant_client), fields(dataset_transform_id = %path.as_ref()))]
pub async fn delete_dataset_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

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

    match dataset_transforms::delete_dataset_transform(
        &postgres_pool,
        &username,
        dataset_transform_id,
    )
    .await
    {
        Ok(_) => HttpResponse::NoContent().finish(),
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
#[tracing::instrument(name = "trigger_dataset_transform", skip(auth, postgres_pool), fields(dataset_transform_id = %path.as_ref()))]
pub async fn trigger_dataset_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let dataset_transform_id = path.into_inner();

    match dataset_transforms::get_dataset_transform(&postgres_pool, &username, dataset_transform_id)
        .await
    {
        Ok(transform) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Dataset transform triggered for all embedders",
            "dataset_transform_id": dataset_transform_id,
            "embedder_count": transform.embedder_ids.len()
        })),
        Err(e) => {
            error!("Dataset transform not found: {}", e);
            not_found(format!("Dataset transform not found: {}", e))
        }
    }
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
#[tracing::instrument(name = "get_dataset_transform_stats", skip(auth, postgres_pool), fields(dataset_transform_id = %path.as_ref()))]
pub async fn get_dataset_transform_stats(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let dataset_transform_id = path.into_inner();

    match dataset_transforms::get_dataset_transform(&postgres_pool, &username, dataset_transform_id)
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
                        "Transform stats: batches={}, completed={}, chunks_embedded={}, chunks_to_process={}",
                        stats.total_batches_processed,
                        stats.successful_batches,
                        stats.total_chunks_embedded,
                        stats.total_chunks_to_process
                    );
                    let response = serde_json::json!({
                        "dataset_transform_id": stats.dataset_transform_id,
                        "embedder_count": stats.embedder_count,
                        "total_batches_processed": stats.total_batches_processed,
                        "successful_batches": stats.successful_batches,
                        "failed_batches": stats.failed_batches,
                        "total_chunks_embedded": stats.total_chunks_embedded,
                        "total_chunks_processing": stats.total_chunks_processing,
                        "total_chunks_failed": stats.total_chunks_failed,
                        "total_chunks_to_process": stats.total_chunks_to_process,
                        "status": stats.status(),
                        "last_run_at": stats.last_run_at,
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
#[tracing::instrument(name = "get_dataset_transforms_for_dataset", skip(auth, postgres_pool), fields(dataset_id = %path.as_ref()))]
pub async fn get_dataset_transforms_for_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match dataset_transforms::get_dataset_transforms_for_dataset(
        &postgres_pool,
        &username,
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
