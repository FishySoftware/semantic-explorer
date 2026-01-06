use crate::auth::extract_username;
use crate::storage;
use crate::transforms::enqueue_scan_job;
use crate::transforms::models::{
    CreateTransform, CreateTransformConfig, ProcessedFile, ScanCollectionJob, Transform,
    TransformStats, TransformStatsWithTotal, TriggerTransformRequest, UpdateTransform,
};

use actix_web::web::{Data, Json, Path};
use actix_web::{HttpResponse, Responder, delete, get, patch, post};
use actix_web_openidconnect::openid_middleware::Authenticated;
use async_nats::Client as NatsClient;
use qdrant_client::Qdrant;
use sqlx::{Pool, Postgres};
use storage::postgres::{collections, datasets, embedders, transforms};
use tracing::error;

#[utoipa::path(
    get,
    path = "/api/transforms",
    tag = "Transforms",
    responses(
        (status = 200, description = "List of transforms", body = Vec<Transform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/transforms")]
#[tracing::instrument(name = "get_transforms", skip(auth, postgres_pool))]
pub(crate) async fn get_transforms(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match transforms::get_transforms(&postgres_pool, &username).await {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to fetch transforms: {}", e)
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/transforms/{transform_id}",
    tag = "Transforms",
    params(
        ("transform_id" = i32, Path, description = "Transform ID")
    ),
    responses(
        (status = 200, description = "Transform details", body = Transform),
        (status = 404, description = "Transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/transforms/{transform_id}")]
#[tracing::instrument(name = "get_transform", skip(auth, postgres_pool), fields(transform_id = %path.as_ref()))]
pub(crate) async fn get_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match transforms::get_transform(&postgres_pool, &username, path.into_inner()).await {
        Ok(transform) => HttpResponse::Ok().json(transform),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Transform not found: {}", e)
        })),
    }
}

#[utoipa::path(
    post,
    path = "/api/transforms",
    tag = "Transforms",
    request_body = CreateTransform,
    responses(
        (status = 201, description = "Transform created", body = Transform),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/transforms")]
#[tracing::instrument(name = "create_transform", skip(auth, postgres_pool, nats_client, body), fields(transform_title = %body.title))]
pub(crate) async fn create_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    body: Json<CreateTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let (
        collection_id,
        dataset_id,
        chunk_size,
        job_type,
        source_dataset_id,
        target_dataset_id,
        source_transform_id,
        embedder_ids,
        job_config,
        collection_mappings,
    ) = match &body.config {
        CreateTransformConfig::CollectionToDataset {
            collection_id,
            dataset_id,
            chunk_size,
            job_config,
        } => {
            if collections::get_collection(&postgres_pool, &username, *collection_id)
                .await
                .is_err()
            {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "collection not found or access denied"
                }));
            }
            if datasets::get_dataset(&postgres_pool, &username, *dataset_id)
                .await
                .is_err()
            {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "dataset not found or access denied"
                }));
            }
            (
                Some(*collection_id),
                *dataset_id,
                *chunk_size,
                "collection_to_dataset",
                None,
                None,
                None,
                None,
                job_config.clone(),
                serde_json::json!({}),
            )
        }
        CreateTransformConfig::DatasetToVectorStorage {
            dataset_id,
            embedder_ids,
            embedding_batch_size,
            wipe_collection,
        } => {
            if datasets::get_dataset(&postgres_pool, &username, *dataset_id)
                .await
                .is_err()
            {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "dataset not found or access denied"
                }));
            }
            let found_embedders = match embedders::get_embedders_by_ids(
                &postgres_pool,
                &username,
                embedder_ids,
            )
            .await
            {
                Ok(e) => e,
                Err(_) => {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "failed to validate embedders"
                    }));
                }
            };

            let found_ids: std::collections::HashSet<i32> =
                found_embedders.iter().map(|e| e.embedder_id).collect();
            for embedder_id in embedder_ids {
                if !found_ids.contains(embedder_id) {
                    return HttpResponse::NotFound().json(serde_json::json!({
                        "error": format!("embedder {} not found or access denied", embedder_id)
                    }));
                }
            }

            let mut config = serde_json::json!({});
            if let Some(size) = embedding_batch_size {
                config["embedding_batch_size"] = serde_json::json!(size);
            }
            if *wipe_collection {
                config["wipe_collection"] = serde_json::json!(true);
            }

            (
                None,
                *dataset_id,
                0,
                "dataset_to_vector_storage",
                Some(*dataset_id),
                None,
                None,
                Some(embedder_ids.clone()),
                config,
                serde_json::json!({}),
            )
        }
        CreateTransformConfig::DatasetVisualizationTransform {
            source_transform_id,
            source_embedder_id,
            dataset_id,
            visualization_config,
        } => {
            // Validate dataset exists
            if datasets::get_dataset(&postgres_pool, &username, *dataset_id)
                .await
                .is_err()
            {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "dataset not found or access denied"
                }));
            }

            // Validate source transform exists and is a dataset_to_vector_storage type
            let source_transform =
                match transforms::get_transform(&postgres_pool, &username, *source_transform_id)
                    .await
                {
                    Ok(t) => t,
                    Err(_) => {
                        return HttpResponse::NotFound().json(serde_json::json!({
                            "error": "source transform not found or access denied"
                        }));
                    }
                };

            if source_transform.job_type != "dataset_to_vector_storage" {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "source transform must be of type dataset_to_vector_storage"
                }));
            }

            // Validate embedder is configured in source transform
            if let Some(ref embedder_ids) = source_transform.embedder_ids {
                if !embedder_ids.contains(source_embedder_id) {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "embedder not configured in source transform"
                    }));
                }
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "source transform has no embedders configured"
                }));
            }

            // Store visualization config in job_config
            let config =
                serde_json::to_value(visualization_config).unwrap_or(serde_json::json!({}));

            (
                None,
                *dataset_id,
                0,
                "dataset_visualization_transform",
                None,
                None,
                Some(*source_transform_id),
                Some(vec![*source_embedder_id]),
                config,
                serde_json::json!({}),
            )
        }
    };

    let mut transform = match transforms::create_transform(
        &postgres_pool,
        storage::postgres::transforms::CreateTransformParams {
            title: &body.title,
            collection_id,
            dataset_id,
            owner: &username,
            chunk_size,
            job_type,
            source_dataset_id,
            target_dataset_id,
            source_transform_id,
            embedder_ids,
            job_config: &job_config,
            collection_mappings: &collection_mappings,
        },
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to create transform: {}", e)
            }));
        }
    };

    if job_type == "dataset_to_vector_storage"
        && let Some(ref embedder_ids) = transform.embedder_ids
    {
        let mut collection_mappings = serde_json::Map::new();
        for embedder_id in embedder_ids {
            let collection_name = crate::transforms::models::Transform::generate_collection_name(
                transform.dataset_id,
                *embedder_id,
                transform.transform_id,
                &username,
            );
            collection_mappings.insert(embedder_id.to_string(), serde_json::json!(collection_name));
        }

        transform = match transforms::update_collection_mappings(
            &postgres_pool,
            transform.transform_id,
            &username,
            &serde_json::Value::Object(collection_mappings),
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("failed to update collection mappings: {}", e);
                transform
            }
        };
    }

    if job_type == "dataset_visualization_transform"
        && let Some(ref embedder_ids) = transform.embedder_ids
        && !embedder_ids.is_empty()
    {
        let embedder_id = embedder_ids[0]; // We store the embedder_id in the first position
        let mut collection_mappings = serde_json::Map::new();

        // Generate names for reduced vectors and topics collections
        let reduced_collection =
            crate::transforms::models::Transform::generate_collection_name_with_suffix(
                transform.dataset_id,
                embedder_id,
                transform.transform_id,
                &username,
                Some("reduced"),
            );
        let topics_collection =
            crate::transforms::models::Transform::generate_collection_name_with_suffix(
                transform.dataset_id,
                embedder_id,
                transform.transform_id,
                &username,
                Some("reduced-topics"),
            );

        collection_mappings.insert("reduced".to_string(), serde_json::json!(reduced_collection));
        collection_mappings.insert("topics".to_string(), serde_json::json!(topics_collection));

        transform = match transforms::update_collection_mappings(
            &postgres_pool,
            transform.transform_id,
            &username,
            &serde_json::Value::Object(collection_mappings),
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("failed to update collection mappings: {}", e);
                transform
            }
        };
    }

    match job_type {
        "collection_to_dataset" => {
            let scan_job = ScanCollectionJob {
                transform_id: transform.transform_id,
            };
            if let Err(e) = enqueue_scan_job(&nats_client, scan_job).await {
                tracing::warn!("failed to enqueue initial scan job: {}", e);
            }
        }
        "dataset_to_vector_storage" => {
            let scan_job = ScanCollectionJob {
                transform_id: transform.transform_id,
            };
            if let Err(e) = enqueue_scan_job(&nats_client, scan_job).await {
                tracing::warn!("failed to enqueue initial scan job: {}", e);
            }
        }
        _ => {}
    }

    HttpResponse::Created().json(transform)
}

#[utoipa::path(
    patch,
    path = "/api/transforms/{transform_id}",
    tag = "Transforms",
    params(
        ("transform_id" = i32, Path, description = "Transform ID")
    ),
    request_body = UpdateTransform,
    responses(
        (status = 200, description = "Transform updated", body = Transform),
        (status = 404, description = "Transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[patch("/api/transforms/{transform_id}")]
#[tracing::instrument(name = "update_transform", skip(auth, postgres_pool, body), fields(transform_id = %path.as_ref()))]
pub(crate) async fn update_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match transforms::update_transform(
        &postgres_pool,
        path.into_inner(),
        &username,
        body.title.as_deref(),
        body.is_enabled,
        body.chunk_size,
        body.embedder_ids.clone(),
    )
    .await
    {
        Ok(transform) => HttpResponse::Ok().json(transform),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("failed to update transform: {}", e)
        })),
    }
}

#[utoipa::path(
    delete,
    path = "/api/transforms/{transform_id}",
    tag = "Transforms",
    params(
        ("transform_id" = i32, Path, description = "Transform ID")
    ),
    responses(
        (status = 204, description = "Transform deleted"),
        (status = 404, description = "Transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[delete("/api/transforms/{transform_id}")]
#[tracing::instrument(name = "delete_transform", skip(auth, postgres_pool, qdrant_client), fields(transform_id = %path.as_ref()))]
pub(crate) async fn delete_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let transform_id = path.into_inner();

    let transform = match transforms::get_transform(&postgres_pool, &username, transform_id).await {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("transform not found or access denied: {}", e)
            }));
        }
    };

    if transform.job_type == "dataset_to_vector_storage"
        && let Some(embedder_ids) = &transform.embedder_ids
    {
        for embedder_id in embedder_ids {
            if let Some(collection_name) = transform.get_collection_name(*embedder_id) {
                match qdrant_client.delete_collection(&collection_name).await {
                    Ok(_) => {
                        tracing::info!("Deleted Qdrant collection: {}", collection_name);
                    }
                    Err(e) => {
                        error!(
                            "Failed to delete Qdrant collection {}: {}. Continuing with transform deletion.",
                            collection_name, e
                        );
                    }
                }
            }
        }
    }

    match transforms::delete_transform(&postgres_pool, transform_id, &username).await {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to delete transform from database: {}", e)
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/transforms/{transform_id}/stats",
    tag = "Transforms",
    params(
        ("transform_id" = i32, Path, description = "Transform ID")
    ),
    responses(
        (status = 200, description = "Transform statistics", body = TransformStats),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Transform not found"),
    ),
)]
#[get("/api/transforms/{transform_id}/stats")]
#[tracing::instrument(name = "get_transform_stats", skip(auth, s3_client, postgres_pool), fields(transform_id = %path.as_ref()))]
pub(crate) async fn get_transform_stats(
    auth: Authenticated,
    s3_client: Data<aws_sdk_s3::Client>,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let transform_id = path.into_inner();

    let transform = match transforms::get_transform(&postgres_pool, &username, transform_id).await {
        Ok(t) => {
            tracing::debug!(job_type = %t.job_type, "retrieved transform");
            t
        }
        Err(e) => {
            tracing::warn!(error = %e, "transform not found or access denied");
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "transform not found or access denied"
            }));
        }
    };

    // For dataset_to_vector_storage transforms, show enhanced stats with chunk counts
    if transform.job_type == "dataset_to_vector_storage" {
        tracing::debug!("fetching enhanced stats for embedding transform");
        match transforms::get_transform_stats_enhanced(&postgres_pool, transform_id).await {
            Ok(Some(stats)) => {
                tracing::info!(
                    successful_items = stats.successful_items,
                    failed_items = stats.failed_items,
                    "returning enhanced transform stats"
                );
                return HttpResponse::Ok().json(serde_json::json!({
                    "transform_id": stats.transform_id,
                    "total_items_processed": stats.total_items_processed,
                    "successful_items": stats.successful_items,
                    "failed_items": stats.failed_items,
                    "total_chunks_embedded": stats.total_chunks_embedded,
                    "total_chunks_failed": stats.total_chunks_failed,
                }));
            }
            Ok(None) => {
                tracing::debug!("no stats available yet");
                return HttpResponse::Ok().json(serde_json::json!({
                    "transform_id": transform_id,
                    "total_items_processed": 0,
                    "successful_items": 0,
                    "failed_items": 0,
                    "total_chunks_embedded": 0,
                    "total_chunks_failed": 0,
                }));
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch enhanced statistics");
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("failed to fetch statistics: {}", e)
                }));
            }
        }
    }

    if transform.job_type == "collection_to_dataset" {
        if let Some(collection_id) = transform.collection_id {
            tracing::debug!(
                collection_id = collection_id,
                "fetching collection file count"
            );

            match collections::get_collection(&postgres_pool, &username, collection_id).await {
                Ok(collection) => {
                    let total_files_in_collection = match storage::rustfs::count_files(
                        &s3_client,
                        &collection.bucket,
                    )
                    .await
                    {
                        Ok(count) => count,
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to count files in collection, defaulting to 0");
                            0
                        }
                    };

                    match transforms::get_transform_stats(&postgres_pool, transform_id).await {
                        Ok(Some(stats)) => {
                            tracing::info!(
                                total_files_in_collection = total_files_in_collection,
                                total_files_processed = stats.total_files_processed,
                                successful_files = stats.successful_files,
                                failed_files = stats.failed_files,
                                "returning collection_to_dataset transform stats with total"
                            );
                            HttpResponse::Ok().json(TransformStatsWithTotal {
                                transform_id: stats.transform_id,
                                total_files_in_collection,
                                total_files_processed: stats.total_files_processed,
                                successful_files: stats.successful_files,
                                failed_files: stats.failed_files,
                                total_items_created: stats.total_items_created,
                            })
                        }
                        Ok(None) => {
                            tracing::debug!("no stats available yet");
                            HttpResponse::Ok().json(TransformStatsWithTotal {
                                transform_id,
                                total_files_in_collection,
                                total_files_processed: 0,
                                successful_files: 0,
                                failed_files: 0,
                                total_items_created: 0,
                            })
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "failed to fetch statistics");
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": format!("failed to fetch statistics: {}", e)
                            }));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to fetch collection");
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("failed to fetch collection: {}", e)
                    }));
                }
            }
        } else {
            tracing::warn!("collection_to_dataset transform missing collection_id");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "collection_to_dataset transform missing collection_id"
            }));
        }
    } else {
        tracing::debug!(job_type = %transform.job_type, "fetching standard stats for transform");
        match transforms::get_transform_stats(&postgres_pool, transform_id).await {
            Ok(Some(stats)) => {
                tracing::info!(
                    successful_files = stats.successful_files,
                    failed_files = stats.failed_files,
                    "returning standard transform stats"
                );
                HttpResponse::Ok().json(stats)
            }
            Ok(None) => {
                tracing::debug!("no stats available yet");
                HttpResponse::Ok().json(serde_json::json!({
                    "transform_id": transform_id,
                    "total_files_processed": 0,
                    "successful_files": 0,
                    "failed_files": 0,
                    "total_items_created": 0
                }))
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch statistics");
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("failed to fetch statistics: {}", e)
                }))
            }
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/transforms/{transform_id}/processed-files",
    tag = "Transforms",
    params(
        ("transform_id" = i32, Path, description = "Transform ID")
    ),
    responses(
        (status = 200, description = "List of processed files", body = Vec<ProcessedFile>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Transform not found"),
    ),
)]
#[get("/api/transforms/{transform_id}/processed-files")]
#[tracing::instrument(name = "get_processed_files", skip(auth, postgres_pool), fields(transform_id = %path.as_ref()))]
pub(crate) async fn get_processed_files(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let transform_id = path.into_inner();

    if transforms::get_transform(&postgres_pool, &username, transform_id)
        .await
        .is_err()
    {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": "transform not found or access denied"
        }));
    }

    match transforms::get_processed_files(&postgres_pool, transform_id).await {
        Ok(files) => HttpResponse::Ok().json(files),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to fetch processed files: {}", e)
        })),
    }
}

#[utoipa::path(
    post,
    path = "/api/transforms/trigger",
    tag = "Transforms",
    request_body = TriggerTransformRequest,
    responses(
        (status = 202, description = "Transform scan triggered"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Transform not found"),
    ),
)]
#[post("/api/transforms/trigger")]
#[tracing::instrument(name = "trigger_transform", skip(auth, postgres_pool, nats_client, body), fields(transform_id = body.transform_id))]
pub(crate) async fn trigger_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    body: Json<TriggerTransformRequest>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let transform =
        match transforms::get_transform(&postgres_pool, &username, body.transform_id).await {
            Ok(t) => t,
            Err(_) => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "transform not found or access denied"
                }));
            }
        };

    // Enqueue appropriate job based on transform type
    let result = match transform.job_type.as_str() {
        "collection_to_dataset" => {
            let scan_job = ScanCollectionJob {
                transform_id: body.transform_id,
            };
            enqueue_scan_job(&nats_client, scan_job).await
        }
        "dataset_to_vector_storage" => {
            let scan_job = ScanCollectionJob {
                transform_id: transform.transform_id,
            };
            enqueue_scan_job(&nats_client, scan_job).await
        }
        _ => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("transform type '{}' cannot be manually triggered", transform.job_type)
            }));
        }
    };

    match result {
        Ok(_) => HttpResponse::Accepted().json(serde_json::json!({
            "message": "transform job enqueued successfully"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to enqueue job: {}", e)
        })),
    }
}
