use crate::audit::{ResourceType, events};
use crate::auth::AuthenticatedUser;
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::collection_transforms;
use crate::transforms::collection::models::{
    CollectionTransform, CollectionTransformStats, CreateCollectionTransform, ProcessedFile,
    UpdateCollectionTransform,
};
use crate::transforms::collection::scanner::trigger_collection_transform_scan;
use semantic_explorer_core::config::S3Config;
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::PaginatedResponse;
use semantic_explorer_core::validation;

use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, patch, post};
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::error;

use actix_web::http::header;
use futures_util::stream::StreamExt;
use std::time::Duration;
use tokio::time::interval;

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
    pub search: Option<String>,
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

#[derive(Deserialize, utoipa::ToSchema, Debug)]
pub struct BatchCollectionTransformStatsRequest {
    pub collection_transform_ids: Vec<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SSEStreamQuery {
    pub collection_id: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/collection-transforms",
    tag = "Collection Transforms",
    params(
        ("limit" = i64, Query, description = "Number of results per page", example = 10),
        ("offset" = i64, Query, description = "Number of results to skip", example = 0),
        ("sort_by" = String, Query, description = "Field to sort by: title, is_enabled, created_at, updated_at", example = "created_at"),
        ("sort_direction" = String, Query, description = "Sort direction: asc or desc", example = "desc"),
        ("search" = Option<String>, Query, description = "Search term to filter transforms by title"),
    ),
    responses(
        (status = 200, description = "Paginated list of collection transforms", body = PaginatedResponse<CollectionTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms")]
#[tracing::instrument(name = "get_collection_transforms", skip(user, pool, params))]
pub async fn get_collection_transforms(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    params: Query<SortParams>,
) -> impl Responder {
    match collection_transforms::get_collection_transforms_paginated(
        &pool,
        &user.as_owner(),
        params.limit,
        params.offset,
        &params.sort_by,
        &params.sort_direction,
        params.search.as_deref(),
    )
    .await
    {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to fetch collection transforms: {e:?}");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch collection transforms"
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/collection-transforms/{id}",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID")
    ),
    responses(
        (status = 200, description = "Collection transform details", body = CollectionTransform),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms/{id}")]
#[tracing::instrument(name = "get_collection_transform", skip(user, pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn get_collection_transform(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match collection_transforms::get_collection_transform(&pool, &user.as_owner(), id).await {
        Ok(transform) => {
            events::resource_read(
                &user.as_owner(),
                &user,
                ResourceType::Transform,
                &id.to_string(),
            );
            HttpResponse::Ok().json(transform)
        }
        Err(e) => {
            error!("Collection transform not found: {}", e);
            not_found(format!("Collection transform not found: {}", e))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/collection-transforms",
    tag = "Collection Transforms",
    request_body = CreateCollectionTransform,
    responses(
        (status = 201, description = "Collection transform created", body = CollectionTransform),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/collection-transforms")]
#[tracing::instrument(name = "create_collection_transform", skip(user, pool, nats_client, s3_client, encryption, body, req), fields(title = %body.title))]
#[allow(clippy::too_many_arguments)] // Actix-web handler with dependency injection
pub async fn create_collection_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    s3_client: Data<S3Client>,
    s3_config: Data<S3Config>,
    encryption: Data<EncryptionService>,
    body: Json<CreateCollectionTransform>,
) -> impl Responder {
    if let Err(e) = validation::validate_title(&body.title) {
        return bad_request(e);
    }

    let owner = user.to_owner_info();
    match collection_transforms::create_collection_transform(
        &pool,
        &body.title,
        body.collection_id,
        body.dataset_id,
        &owner,
        body.chunk_size,
        &body.job_config,
    )
    .await
    {
        Ok(transform) => {
            let collection_transform_id = transform.collection_transform_id;
            if let Err(e) = trigger_collection_transform_scan(
                &pool,
                &nats_client,
                &s3_client,
                &s3_config.bucket_name,
                collection_transform_id,
                &user.as_owner(),
                &encryption,
            )
            .await
            {
                error!(
                    "Failed to trigger collection transform scan for newly created transform {}: {}",
                    collection_transform_id, e
                );
            }
            events::resource_created_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::Transform,
                &collection_transform_id.to_string(),
            );
            HttpResponse::Created().json(transform)
        }
        Err(e) => {
            error!("Failed to create collection transform: {}", e);
            bad_request(format!("Failed to create collection transform: {}", e))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/collection-transforms/{id}",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID")
    ),
    request_body = UpdateCollectionTransform,
    responses(
        (status = 200, description = "Collection transform updated", body = CollectionTransform),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[patch("/api/collection-transforms/{id}")]
#[tracing::instrument(name = "update_collection_transform", skip(user, pool, body), fields(collection_transform_id = %path.as_ref()))]
pub async fn update_collection_transform(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateCollectionTransform>,
) -> impl Responder {
    if let Some(ref title) = body.title
        && let Err(e) = validation::validate_title(title)
    {
        return bad_request(e);
    }

    let id = path.into_inner();
    match collection_transforms::update_collection_transform(
        &pool,
        &user.as_owner(),
        id,
        body.title.as_deref(),
        body.is_enabled,
        body.chunk_size,
        body.job_config.as_ref(),
    )
    .await
    {
        Ok(transform) => {
            events::resource_updated(
                &user.as_owner(),
                &user,
                ResourceType::Transform,
                &id.to_string(),
            );
            HttpResponse::Ok().json(transform)
        }
        Err(e) => {
            error!("Failed to update collection transform: {}", e);
            not_found(format!("Failed to update collection transform: {}", e))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/collection-transforms/{id}",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID")
    ),
    responses(
        (status = 204, description = "Collection transform deleted"),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[delete("/api/collection-transforms/{id}")]
#[tracing::instrument(name = "delete_collection_transform", skip(user, pool, req), fields(collection_transform_id = %path.as_ref()))]
pub async fn delete_collection_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match collection_transforms::delete_collection_transform(&pool, &user.as_owner(), id).await {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::Transform,
                &id.to_string(),
            );
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            error!("Failed to delete collection transform: {}", e);
            not_found(format!("Failed to delete collection transform: {}", e))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/collection-transforms/{id}/trigger",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID")
    ),
    responses(
        (status = 200, description = "Collection transform triggered"),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/collection-transforms/{id}/trigger")]
#[tracing::instrument(name = "trigger_collection_transform", skip(user, pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn trigger_collection_transform(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let collection_transform_id = path.into_inner();

    match collection_transforms::get_collection_transform(
        &pool,
        &user.as_owner(),
        collection_transform_id,
    )
    .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Collection transform triggered",
            "collection_transform_id": collection_transform_id
        })),
        Err(e) => {
            error!("Collection transform not found: {}", e);
            not_found(format!("Collection transform not found: {}", e))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/collection-transforms/{id}/stats",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID")
    ),
    responses(
        (status = 200, description = "Collection transform statistics", body = CollectionTransformStats),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms/{id}/stats")]
#[tracing::instrument(name = "get_collection_transform_stats", skip(user, pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn get_collection_transform_stats(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let collection_transform_id = path.into_inner();

    match collection_transforms::get_collection_transform(
        &pool,
        &user.as_owner(),
        collection_transform_id,
    )
    .await
    {
        Ok(_) => {
            match collection_transforms::get_collection_transform_stats(
                &pool,
                collection_transform_id,
            )
            .await
            {
                Ok(stats) => HttpResponse::Ok().json(stats),
                Err(e) => {
                    error!("Failed to get stats: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get stats: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Collection transform not found: {}", e);
            not_found(format!("Collection transform not found: {}", e))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/collection-transforms/batch-stats",
    tag = "Collection Transforms",
    request_body = BatchCollectionTransformStatsRequest,
    responses(
        (status = 200, description = "Batch transform statistics"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Collection transform not found"),
    ),
)]
#[post("/api/collection-transforms/batch-stats")]
#[tracing::instrument(name = "get_batch_collection_transform_stats", skip(user, pool))]
pub async fn get_batch_collection_transform_stats(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    body: Json<BatchCollectionTransformStatsRequest>,
) -> impl Responder {
    let transform_ids = &body.collection_transform_ids;

    // Verify ownership of all transforms
    for &id in transform_ids {
        match collection_transforms::get_collection_transform(&pool, &user.as_owner(), id).await {
            Ok(_) => {}
            Err(_) => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": format!("Collection transform {} not found", id)
                }));
            }
        }
    }

    match collection_transforms::get_batch_collection_transform_stats(&pool, transform_ids).await {
        Ok(stats_map) => HttpResponse::Ok().json(stats_map),
        Err(e) => {
            error!("Failed to get batch stats: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch batch statistics"
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/collection-transforms/{id}/processed-files",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID"),
        ("limit" = i64, Query, description = "Number of results per page", example = 10),
        ("offset" = i64, Query, description = "Number of results to skip", example = 0),
    ),
    responses(
        (status = 200, description = "Processed files", body = PaginatedResponse<ProcessedFile>),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms/{id}/processed-files")]
#[tracing::instrument(name = "get_processed_files", skip(user, pool, params), fields(collection_transform_id = %path.as_ref()))]
pub async fn get_processed_files(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    params: Query<SortParams>,
) -> impl Responder {
    let collection_transform_id = path.into_inner();

    match collection_transforms::get_collection_transform(
        &pool,
        &user.as_owner(),
        collection_transform_id,
    )
    .await
    {
        Ok(_) => {
            match collection_transforms::get_processed_files(&pool, collection_transform_id).await {
                Ok(files) => {
                    let total_count = files.len() as i64;
                    let offset = params.offset as usize;
                    let limit = params.limit as usize;
                    let paginated_files: Vec<ProcessedFile> =
                        files.into_iter().skip(offset).take(limit).collect();

                    let response = PaginatedResponse {
                        items: paginated_files,
                        total_count,
                        limit: params.limit,
                        offset: params.offset,
                    };
                    HttpResponse::Ok().json(response)
                }
                Err(e) => {
                    error!("Failed to get processed files: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get processed files: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Collection transform not found: {}", e);
            not_found(format!("Collection transform not found: {}", e))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/collections/{collection_id}/transforms",
    tag = "Collection Transforms",
    params(
        ("collection_id" = i32, Path, description = "Collection ID")
    ),
    responses(
        (status = 200, description = "Collection transforms for collection", body = Vec<CollectionTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collections/{collection_id}/transforms")]
#[tracing::instrument(name = "get_collection_transforms_for_collection", skip(user, pool), fields(collection_id = %path.as_ref()))]
pub async fn get_collection_transforms_for_collection(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    match collection_transforms::get_collection_transforms_for_collection(
        &pool,
        &user.as_owner(),
        path.into_inner(),
    )
    .await
    {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => {
            error!("Failed to fetch collection transforms: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch collection transforms: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/datasets/{dataset_id}/collection-transforms",
    tag = "Collection Transforms",
    params(
        ("dataset_id" = i32, Path, description = "Dataset ID")
    ),
    responses(
        (status = 200, description = "Collection transforms targeting this dataset", body = Vec<CollectionTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/datasets/{dataset_id}/collection-transforms")]
#[tracing::instrument(name = "get_collection_transforms_for_dataset", skip(user, pool), fields(dataset_id = %path.as_ref()))]
pub async fn get_collection_transforms_for_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    match collection_transforms::get_collection_transforms_for_dataset(
        &pool,
        &user.as_owner(),
        path.into_inner(),
    )
    .await
    {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => {
            error!("Failed to fetch collection transforms for dataset: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch collection transforms: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/collection-transforms/stream",
    tag = "Collection Transforms",
    params(
        ("collection_id" = Option<i32>, Query, description = "Optional collection ID to filter updates"),
    ),
    responses(
        (status = 200, description = "Server-Sent Events stream of collection transform status updates", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms/stream")]
#[tracing::instrument(name = "stream_collection_transform_status", skip(user, nats_client))]
pub async fn stream_collection_transform_status(
    user: AuthenticatedUser,
    nats_client: Data<NatsClient>,
    query: Query<SSEStreamQuery>,
) -> impl Responder {
    let owner = user.to_string();
    let nats = nats_client.get_ref().clone();
    let collection_id_filter = query.collection_id;

    // Create SSE stream
    let stream = async_stream::stream! {
        // Subscribe to collection transform status updates
        // Subject format: sse.transforms.collection.status.{owner}.{collection_id}.{transform_id}
        // Uses sse. prefix to receive only SSE updates (not JetStream worker results)
        let subject = match collection_id_filter {
            Some(collection_id) => format!("sse.transforms.collection.status.{}.{}.*", owner, collection_id),
            None => format!("sse.transforms.collection.status.{}.>", owner),
        };

        let mut subscriber = match nats.subscribe(subject.clone()).await {
            Ok(sub) => sub,
            Err(e) => {
                error!("Failed to subscribe to NATS subject '{}': {}", subject, e);
                yield Err(actix_web::error::ErrorInternalServerError(e));
                return;
            }
        };

        // Send initial connection message
        yield Ok::<_, actix_web::Error>(actix_web::web::Bytes::from("event: connected\ndata: {\"status\":\"connected\"}\n\n"));

        // Heartbeat interval (30 seconds)
        let mut heartbeat = interval(Duration::from_secs(30));
        heartbeat.tick().await; // Skip first immediate tick

        loop {
            tokio::select! {
                // Handle NATS messages
                msg_result = subscriber.next() => {
                    match msg_result {
                        Some(msg) => {
                            match String::from_utf8(msg.payload.to_vec()) {
                                Ok(payload) => {
                                    yield Ok(actix_web::web::Bytes::from(format!("event: status\ndata: {}\n\n", payload)));
                                }
                                Err(e) => {
                                    error!("Failed to parse message payload: {}", e);
                                }
                            }
                        }
                        None => {
                            // Subscription closed
                            yield Ok(actix_web::web::Bytes::from("event: closed\ndata: {\"status\":\"subscription_closed\"}\n\n"));
                            break;
                        }
                    }
                }
                // Send heartbeat
                _ = heartbeat.tick() => {
                    yield Ok(actix_web::web::Bytes::from(": heartbeat\n\n"));
                }
            }
        }
    };

    HttpResponse::Ok()
        .insert_header(header::ContentType(mime::TEXT_EVENT_STREAM))
        .insert_header(header::CacheControl(vec![header::CacheDirective::NoCache]))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}
