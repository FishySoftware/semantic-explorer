use crate::audit::{ResourceType, events};
use crate::auth::AuthenticatedUser;
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::collection_transforms;
use crate::transforms::collection::models::{
    CollectionTransform, CollectionTransformStats, CreateCollectionTransform, ProcessedFile,
    UpdateCollectionTransform,
};
use crate::transforms::collection::scanner::trigger_collection_transform_scan;
use semantic_explorer_core::validation;

use actix_web::web::{Data, Json, Path};
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, patch, post};
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use tracing::error;

#[utoipa::path(
    get,
    path = "/api/collection-transforms",
    tag = "Collection Transforms",
    responses(
        (status = 200, description = "List of collection transforms", body = Vec<CollectionTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms")]
#[tracing::instrument(name = "get_collection_transforms", skip(user, postgres_pool))]
pub async fn get_collection_transforms(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match collection_transforms::get_collection_transforms(&postgres_pool, &user).await {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => {
            error!("Failed to fetch collection transforms: {e:?}");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch collection transforms: {e:?}")
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
#[tracing::instrument(name = "get_collection_transform", skip(user, postgres_pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn get_collection_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match collection_transforms::get_collection_transform(&postgres_pool, &user, id).await {
        Ok(transform) => {
            events::resource_read(&user, ResourceType::Transform, &id.to_string());
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
#[tracing::instrument(name = "create_collection_transform", skip(user, postgres_pool, nats_client, s3_client, body, req), fields(title = %body.title))]
pub async fn create_collection_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    s3_client: Data<S3Client>,
    body: Json<CreateCollectionTransform>,
) -> impl Responder {
    // Validate input
    if let Err(e) = validation::validate_title(&body.title) {
        return bad_request(e);
    }

    match collection_transforms::create_collection_transform(
        &postgres_pool,
        &body.title,
        body.collection_id,
        body.dataset_id,
        &user,
        body.chunk_size,
        &body.job_config,
    )
    .await
    {
        Ok(transform) => {
            // Trigger the scan immediately upon creation
            let collection_transform_id = transform.collection_transform_id;
            if let Err(e) = trigger_collection_transform_scan(
                &postgres_pool,
                &nats_client,
                &s3_client,
                collection_transform_id,
                &user,
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
#[tracing::instrument(name = "update_collection_transform", skip(user, postgres_pool, body), fields(collection_transform_id = %path.as_ref()))]
pub async fn update_collection_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateCollectionTransform>,
) -> impl Responder {
    // Validate input if title is provided
    if let Some(ref title) = body.title
        && let Err(e) = validation::validate_title(title)
    {
        return bad_request(e);
    }

    let id = path.into_inner();
    match collection_transforms::update_collection_transform(
        &postgres_pool,
        &user,
        id,
        body.title.as_deref(),
        body.is_enabled,
        body.chunk_size,
        body.job_config.as_ref(),
    )
    .await
    {
        Ok(transform) => {
            events::resource_updated(&user, ResourceType::Transform, &id.to_string());
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
#[tracing::instrument(name = "delete_collection_transform", skip(user, postgres_pool, req), fields(collection_transform_id = %path.as_ref()))]
pub async fn delete_collection_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match collection_transforms::delete_collection_transform(&postgres_pool, &user, id).await {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
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
#[tracing::instrument(name = "trigger_collection_transform", skip(user, postgres_pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn trigger_collection_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let collection_transform_id = path.into_inner();

    match collection_transforms::get_collection_transform(
        &postgres_pool,
        &user,
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
#[tracing::instrument(name = "get_collection_transform_stats", skip(user, postgres_pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn get_collection_transform_stats(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let collection_transform_id = path.into_inner();

    match collection_transforms::get_collection_transform(
        &postgres_pool,
        &user,
        collection_transform_id,
    )
    .await
    {
        Ok(_) => {
            match collection_transforms::get_collection_transform_stats(
                &postgres_pool,
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
    get,
    path = "/api/collection-transforms/{id}/processed-files",
    tag = "Collection Transforms",
    params(
        ("id" = i32, Path, description = "Collection Transform ID")
    ),
    responses(
        (status = 200, description = "Processed files", body = Vec<ProcessedFile>),
        (status = 404, description = "Collection transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/collection-transforms/{id}/processed-files")]
#[tracing::instrument(name = "get_processed_files", skip(user, postgres_pool), fields(collection_transform_id = %path.as_ref()))]
pub async fn get_processed_files(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let collection_transform_id = path.into_inner();

    match collection_transforms::get_collection_transform(
        &postgres_pool,
        &user,
        collection_transform_id,
    )
    .await
    {
        Ok(_) => {
            match collection_transforms::get_processed_files(
                &postgres_pool,
                collection_transform_id,
            )
            .await
            {
                Ok(files) => HttpResponse::Ok().json(files),
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
#[tracing::instrument(name = "get_collection_transforms_for_collection", skip(user, postgres_pool), fields(collection_id = %path.as_ref()))]
pub async fn get_collection_transforms_for_collection(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    match collection_transforms::get_collection_transforms_for_collection(
        &postgres_pool,
        &user,
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
