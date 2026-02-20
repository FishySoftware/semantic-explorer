use crate::audit::{ResourceType, events};
use crate::auth::AuthenticatedUser;
use crate::embedded_datasets::{
    CreateStandaloneEmbeddedDatasetRequest, EmbeddedDataset, EmbeddedDatasetListQuery,
    EmbeddedDatasetProcessedBatch, EmbeddedDatasetStats, EmbeddedDatasetWithDetails,
    PaginatedEmbeddedDatasetList, PushVectorsRequest, PushVectorsResponse,
};
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::embedded_datasets;
use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, patch, post};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, GetPointsBuilder, PointStruct, UpsertPointsBuilder,
    VectorParams,
};
use qdrant_client::qdrant::{PointId, ScrollPointsBuilder};
use semantic_explorer_core::validation;
use serde::{Deserialize, Serialize};

const MAX_EMBEDDED_DATASETS_LIMIT: i64 = 200;
const DEFAULT_EMBEDDED_DATASETS_LIMIT: i64 = 50;
const MAX_PROCESSED_BATCHES_LIMIT: i64 = 5000;
const DEFAULT_PROCESSED_BATCHES_LIMIT: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct EmbeddedDatasetsPaginationParams {
    #[serde(default = "default_embedded_datasets_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_embedded_datasets_limit() -> i64 {
    DEFAULT_EMBEDDED_DATASETS_LIMIT
}

#[derive(Debug, Deserialize)]
pub struct ProcessedBatchesPaginationParams {
    #[serde(default = "default_processed_batches_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_processed_batches_limit() -> i64 {
    DEFAULT_PROCESSED_BATCHES_LIMIT
}
use sqlx::{Pool, Postgres};
use tracing::{error, info, warn};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema, Debug)]
pub struct PointsQuery {
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

fn default_limit() -> u64 {
    10
}

#[derive(Serialize, ToSchema)]
pub struct QdrantPoint {
    pub id: String,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

#[derive(Serialize, ToSchema)]
pub struct PointsResponse {
    pub points: Vec<QdrantPoint>,
    pub total_count: u64,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateEmbeddedDatasetRequest {
    pub title: String,
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets",
    tag = "Embedded Datasets",
    params(
        ("search" = Option<String>, Query, description = "Search by title"),
        ("limit" = i64, Query, description = "Number of items per page"),
        ("offset" = i64, Query, description = "Pagination offset"),
    ),
    responses(
        (status = 200, description = "Paginated list of embedded datasets", body = PaginatedEmbeddedDatasetList),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets")]
#[tracing::instrument(name = "get_embedded_datasets", skip(user, pool))]
pub async fn get_embedded_datasets(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    query: Query<EmbeddedDatasetListQuery>,
) -> impl Responder {
    let search_query = query.search.as_ref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let limit = query.limit.clamp(1, 100);
    let offset = query.offset.max(0);

    let result = if let Some(q) = search_query {
        embedded_datasets::get_embedded_datasets_with_search(
            &pool,
            &user.as_owner(),
            q,
            limit,
            offset,
        )
        .await
    } else {
        embedded_datasets::get_embedded_datasets_paginated(&pool, &user.as_owner(), limit, offset)
            .await
    };

    match result {
        Ok((datasets, total_count)) => {
            let paginated_response = PaginatedEmbeddedDatasetList {
                embedded_datasets: datasets,
                total_count,
                limit,
                offset,
            };
            HttpResponse::Ok().json(paginated_response)
        }
        Err(e) => {
            error!("Failed to fetch embedded datasets: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch embedded datasets: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{id}",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    responses(
        (status = 200, description = "Embedded dataset details with enriched info", body = EmbeddedDatasetWithDetails),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}")]
#[tracing::instrument(name = "get_embedded_dataset", skip(user, pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_embedded_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match embedded_datasets::get_embedded_dataset_with_details(&pool, &user.as_owner(), id).await {
        Ok(dataset) => {
            events::resource_read(
                &user.as_owner(),
                &user,
                ResourceType::Dataset,
                &id.to_string(),
            );
            HttpResponse::Ok().json(dataset)
        }
        Err(e) => {
            error!("Embedded dataset not found: {}", e);
            not_found(format!("Embedded dataset not found: {}", e))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/embedded-datasets/{id}",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    responses(
        (status = 204, description = "Embedded dataset deleted"),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[delete("/api/embedded-datasets/{id}")]
#[tracing::instrument(name = "delete_embedded_dataset", skip(user, pool, qdrant_client, req), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn delete_embedded_dataset(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();

    // First get the embedded dataset to retrieve the collection name
    let embedded_dataset =
        match embedded_datasets::get_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
            .await
        {
            Ok(dataset) => dataset,
            Err(e) => {
                error!("Failed to find embedded dataset: {}", e);
                return not_found(format!("Embedded dataset not found: {}", e));
            }
        };

    // Delete the Qdrant collection
    if let Err(e) = qdrant_client
        .delete_collection(&embedded_dataset.collection_name)
        .await
    {
        error!(
            "Failed to delete Qdrant collection {}: {}",
            embedded_dataset.collection_name, e
        );
    }

    // Delete the database entry
    match embedded_datasets::delete_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
        .await
    {
        Ok(_) => {
            info!(
                "Successfully deleted embedded dataset {} and Qdrant collection {}",
                embedded_dataset_id, embedded_dataset.collection_name
            );
            events::resource_deleted_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::Dataset,
                &embedded_dataset_id.to_string(),
            );
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            error!("Failed to delete embedded dataset from database: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to delete embedded dataset: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{id}/stats",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    responses(
        (status = 200, description = "Embedded dataset statistics", body = EmbeddedDatasetStats),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}/stats")]
#[tracing::instrument(name = "get_embedded_dataset_stats", skip(user, pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_embedded_dataset_stats(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();

    match embedded_datasets::get_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
        .await
    {
        Ok(_) => {
            match embedded_datasets::get_embedded_dataset_stats(
                &pool,
                &user.as_owner(),
                embedded_dataset_id,
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
            error!("Embedded dataset not found: {}", e);
            not_found(format!("Embedded dataset not found: {}", e))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{id}/points",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID"),
        ("limit" = Option<u64>, Query, description = "Number of points per page (default: 10)"),
        ("offset" = Option<String>, Query, description = "Offset for pagination"),
    ),
    responses(
        (status = 200, description = "Qdrant points with pagination", body = PointsResponse),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}/points")]
#[tracing::instrument(name = "get_embedded_dataset_points", skip(user, pool, qdrant_client), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_embedded_dataset_points(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
    query: Query<PointsQuery>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();

    // Verify embedded dataset exists and belongs to user
    let embedded_dataset =
        match embedded_datasets::get_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
            .await
        {
            Ok(dataset) => dataset,
            Err(e) => {
                error!("Embedded dataset not found: {}", e);
                return not_found(format!("Embedded dataset not found: {}", e));
            }
        };

    // Get collection info for total count
    let collection_info = match qdrant_client
        .collection_info(&embedded_dataset.collection_name)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to get collection info: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get collection info: {}", e)
            }));
        }
    };

    let total_count = collection_info
        .result
        .and_then(|r| r.points_count)
        .unwrap_or(0);

    // Scroll through points with pagination
    let scroll_result = match qdrant_client
        .scroll(
            ScrollPointsBuilder::new(&embedded_dataset.collection_name)
                .limit(query.limit as u32)
                .offset(query.offset)
                .with_payload(true)
                .with_vectors(false),
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to scroll points: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to scroll points: {}", e)
            }));
        }
    };

    let points: Vec<QdrantPoint> = scroll_result
        .result
        .iter()
        .map(|point| {
            let payload_json =
                serde_json::to_value(&point.payload).unwrap_or(serde_json::json!({}));
            let id_str = match &point.id {
                Some(id) => match &id.point_id_options {
                    Some(PointIdOptions::Uuid(uuid)) => uuid.clone(),
                    Some(PointIdOptions::Num(num)) => num.to_string(),
                    None => String::from("unknown"),
                },
                None => String::from("unknown"),
            };
            QdrantPoint {
                id: id_str,
                payload: payload_json,
                vector: None,
            }
        })
        .collect();

    let next_offset = scroll_result.next_page_offset.and_then(|offset_id| {
        // Try to extract numeric ID if possible
        match offset_id.point_id_options {
            Some(PointIdOptions::Num(num)) => Some(num),
            _ => None,
        }
    });

    HttpResponse::Ok().json(PointsResponse {
        points,
        total_count,
        next_offset,
    })
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{id}/points/{point_id}/vector",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID"),
        ("point_id" = String, Path, description = "Point ID"),
    ),
    responses(
        (status = 200, description = "Point with vector", body = QdrantPoint),
        (status = 404, description = "Point or embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}/points/{point_id}/vector")]
#[tracing::instrument(name = "get_point_vector", skip(user, pool, qdrant_client))]
pub async fn get_point_vector(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let (embedded_dataset_id, point_id) = path.into_inner();

    // Verify embedded dataset exists and belongs to user
    let embedded_dataset =
        match embedded_datasets::get_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
            .await
        {
            Ok(dataset) => dataset,
            Err(e) => {
                error!("Embedded dataset not found: {}", e);
                return not_found(format!("Embedded dataset not found: {}", e));
            }
        };

    // Convert the string point_id to a PointId
    // Try to parse as UUID first, then as u64
    let qdrant_point_id = if let Ok(uuid) = uuid::Uuid::parse_str(&point_id) {
        PointId {
            point_id_options: Some(PointIdOptions::Uuid(uuid.to_string())),
        }
    } else if let Ok(numeric_id) = point_id.parse::<u64>() {
        PointId {
            point_id_options: Some(PointIdOptions::Num(numeric_id)),
        }
    } else {
        error!("Failed to parse point_id as UUID or u64: {}", point_id);
        return bad_request("Invalid point_id format: must be a valid UUID or numeric ID");
    };

    let points = match qdrant_client
        .get_points(
            GetPointsBuilder::new(&embedded_dataset.collection_name, vec![qdrant_point_id])
                .with_payload(true)
                .with_vectors(true),
        )
        .await
    {
        Ok(result) => result.result,
        Err(e) => {
            error!("Failed to get point: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get point: {}", e)
            }));
        }
    };

    if points.is_empty() {
        return not_found(format!("Point {} not found", point_id));
    }

    let point = &points[0];
    let payload_json = serde_json::to_value(&point.payload).unwrap_or(serde_json::json!({}));
    let vector = point.vectors.as_ref().and_then(|vectors_output| {
        vectors_output
            .vectors_options
            .as_ref()
            .and_then(|vo| match vo {
                qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(vector_output) => {
                    match vector_output.clone().into_vector() {
                        qdrant_client::qdrant::vector_output::Vector::Dense(dense) => {
                            Some(dense.data)
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
    });

    let id_str = match &point.id {
        Some(id) => format!("{:?}", id),
        None => String::from("unknown"),
    };

    HttpResponse::Ok().json(QdrantPoint {
        id: id_str,
        payload: payload_json,
        vector,
    })
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{id}/processed-batches",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID"),
        ("limit" = Option<i64>, Query, description = "Max batches to return (1-5000, default 100)"),
        ("offset" = Option<i64>, Query, description = "Number of batches to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "Processed batches", body = Vec<EmbeddedDatasetProcessedBatch>),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}/processed-batches")]
#[tracing::instrument(name = "get_processed_batches", skip(user, pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_processed_batches(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    params: Query<ProcessedBatchesPaginationParams>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();
    let limit = params.limit.clamp(1, MAX_PROCESSED_BATCHES_LIMIT);
    let offset = params.offset.max(0);

    match embedded_datasets::get_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
        .await
    {
        Ok(_) => {
            match embedded_datasets::get_processed_batches(
                &pool,
                embedded_dataset_id,
                limit,
                offset,
            )
            .await
            {
                Ok(batches) => HttpResponse::Ok().json(batches),
                Err(e) => {
                    error!("Failed to get processed batches: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get processed batches: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Embedded dataset not found: {}", e);
            not_found(format!("Embedded dataset not found: {}", e))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/datasets/{dataset_id}/embedded-datasets",
    tag = "Embedded Datasets",
    params(
        ("dataset_id" = i32, Path, description = "Dataset ID"),
        ("limit" = Option<i64>, Query, description = "Max results to return (1-200, default 50)"),
        ("offset" = Option<i64>, Query, description = "Number of results to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "Embedded datasets for dataset (across all transforms)", body = Vec<EmbeddedDataset>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/datasets/{dataset_id}/embedded-datasets")]
#[tracing::instrument(name = "get_embedded_datasets_for_dataset", skip(user, pool), fields(dataset_id = %path.as_ref()))]
pub async fn get_embedded_datasets_for_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    params: Query<EmbeddedDatasetsPaginationParams>,
) -> impl Responder {
    let limit = params.limit.clamp(1, MAX_EMBEDDED_DATASETS_LIMIT);
    let offset = params.offset.max(0);
    match embedded_datasets::get_embedded_datasets_for_dataset(
        &pool,
        &user.as_owner(),
        path.into_inner(),
        limit,
        offset,
    )
    .await
    {
        Ok(datasets) => HttpResponse::Ok().json(datasets),
        Err(e) => {
            error!("Failed to fetch embedded datasets: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch embedded datasets: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/embedded-datasets/{id}",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    request_body = UpdateEmbeddedDatasetRequest,
    responses(
        (status = 200, description = "Embedded dataset updated", body = EmbeddedDataset),
        (status = 400, description = "Bad request - invalid input"),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[patch("/api/embedded-datasets/{id}")]
#[tracing::instrument(name = "update_embedded_dataset", skip(user, pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn update_embedded_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateEmbeddedDatasetRequest>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();

    // Validate title
    if let Err(e) = validation::validate_title(&body.title) {
        return bad_request(&e);
    }

    match embedded_datasets::update_embedded_dataset_title(
        &pool,
        &user.as_owner(),
        embedded_dataset_id,
        body.title.trim(),
    )
    .await
    {
        Ok(dataset) => {
            events::resource_updated(
                &user.as_owner(),
                &user,
                ResourceType::Dataset,
                &embedded_dataset_id.to_string(),
            );
            HttpResponse::Ok().json(dataset)
        }
        Err(e) => {
            error!("Failed to update embedded dataset: {}", e);
            not_found("Embedded dataset not found or not owned by this user")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/embedded-datasets/standalone",
    tag = "Embedded Datasets",
    request_body = CreateStandaloneEmbeddedDatasetRequest,
    responses(
        (status = 201, description = "Standalone embedded dataset created", body = EmbeddedDataset),
        (status = 400, description = "Bad request - invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
)]
#[post("/api/embedded-datasets/standalone")]
#[tracing::instrument(
    name = "create_standalone_embedded_dataset",
    skip(user, pool, qdrant_client, req)
)]
pub async fn create_standalone_embedded_dataset(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    body: Json<CreateStandaloneEmbeddedDatasetRequest>,
) -> impl Responder {
    // Validate title
    if let Err(e) = validation::validate_title(&body.title) {
        return bad_request(&e);
    }

    // Validate dimensions
    if body.dimensions < 1 || body.dimensions > 65536 {
        return bad_request("Dimensions must be between 1 and 65536");
    }

    let owner = user.to_owner_info();

    // Create the embedded dataset in the database
    let embedded_dataset = match embedded_datasets::create_standalone_embedded_dataset(
        &pool,
        &owner,
        body.title.trim(),
        body.dimensions,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            error!("Failed to create standalone embedded dataset: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to create embedded dataset: {}", e)
            }));
        }
    };

    // Create the Qdrant collection with the specified dimensions
    let create_collection = CreateCollectionBuilder::new(&embedded_dataset.collection_name)
        .vectors_config(VectorParams {
            size: body.dimensions as u64,
            distance: Distance::Cosine.into(),
            on_disk: Some(true),
            ..Default::default()
        })
        .on_disk_payload(true)
        .build();

    match qdrant_client.create_collection(create_collection).await {
        Ok(_) => {
            info!(
                "Created Qdrant collection {} with {} dimensions",
                embedded_dataset.collection_name, body.dimensions
            );
        }
        Err(e) => {
            // If Qdrant collection creation fails, we should clean up the database entry
            error!(
                "Failed to create Qdrant collection {}: {}",
                embedded_dataset.collection_name, e
            );
            // Try to delete the database entry
            if let Err(del_err) = embedded_datasets::delete_embedded_dataset(
                &pool,
                &user.as_owner(),
                embedded_dataset.embedded_dataset_id,
            )
            .await
            {
                error!(
                    "Failed to cleanup database entry after Qdrant failure: {}",
                    del_err
                );
            }
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to create Qdrant collection: {}", e)
            }));
        }
    }

    events::resource_created_with_request(
        &req,
        &user.as_owner(),
        &user,
        ResourceType::Dataset,
        &embedded_dataset.embedded_dataset_id.to_string(),
    );

    HttpResponse::Created().json(embedded_dataset)
}

#[utoipa::path(
    post,
    path = "/api/embedded-datasets/{id}/push-vectors",
    tag = "Embedded Datasets",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    request_body = PushVectorsRequest,
    responses(
        (status = 200, description = "Vectors pushed successfully", body = PushVectorsResponse),
        (status = 400, description = "Bad request - invalid vectors or dimensions mismatch"),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
)]
#[post("/api/embedded-datasets/{id}/push-vectors")]
#[tracing::instrument(name = "push_vectors_to_embedded_dataset", skip(user, pool, qdrant_client, body), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn push_vectors_to_embedded_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
    body: Json<PushVectorsRequest>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();

    // Validate request
    if body.points.is_empty() {
        return bad_request("At least one point must be provided");
    }

    if body.points.len() > 1000 {
        return bad_request("Maximum 1000 points per request");
    }

    // Get the embedded dataset to verify ownership and get collection info
    let embedded_dataset =
        match embedded_datasets::get_embedded_dataset(&pool, &user.as_owner(), embedded_dataset_id)
            .await
        {
            Ok(dataset) => dataset,
            Err(e) => {
                error!("Embedded dataset not found: {}", e);
                return not_found(format!("Embedded dataset not found: {}", e));
            }
        };

    // Check if this is a standalone dataset
    if !embedded_dataset.is_standalone() {
        return bad_request(
            "Cannot push vectors to transform-based embedded datasets. Use standalone datasets instead.",
        );
    }

    // Get the expected dimensions
    let expected_dimensions = match embedded_dataset.dimensions {
        Some(dims) => dims as usize,
        None => {
            error!(
                "Standalone dataset {} has no dimensions set",
                embedded_dataset_id
            );
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Dataset dimensions not configured"
            }));
        }
    };

    // Validate all vectors have correct dimensions
    for (i, point) in body.points.iter().enumerate() {
        if point.vector.len() != expected_dimensions {
            return bad_request(format!(
                "Point {} has {} dimensions, expected {}",
                i,
                point.vector.len(),
                expected_dimensions
            ));
        }
    }

    // Ensure the Qdrant collection exists (create if it doesn't)
    let collection_exists = qdrant_client
        .collection_info(&embedded_dataset.collection_name)
        .await
        .is_ok();

    if !collection_exists {
        warn!(
            "Collection {} not found, creating it",
            embedded_dataset.collection_name
        );
        let create_collection = CreateCollectionBuilder::new(&embedded_dataset.collection_name)
            .vectors_config(VectorParams {
                size: expected_dimensions as u64,
                distance: Distance::Cosine.into(),
                on_disk: Some(true),
                ..Default::default()
            })
            .on_disk_payload(true)
            .build();

        if let Err(e) = qdrant_client.create_collection(create_collection).await {
            let error_str = e.to_string();
            if !error_str.contains("already exists") {
                error!(
                    "Failed to create collection {}: {}",
                    embedded_dataset.collection_name, e
                );
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to create collection: {}", e)
                }));
            }
        }
    }

    // Convert points to Qdrant format
    let points: Vec<PointStruct> = body
        .points
        .iter()
        .map(|point| {
            // Convert serde_json::Value to serde_json::Map for Payload
            let payload_map = match &point.payload {
                serde_json::Value::Object(map) => map.clone(),
                _ => serde_json::Map::new(),
            };
            PointStruct::new(
                point.id.clone(),
                point.vector.clone(),
                qdrant_client::Payload::from(payload_map),
            )
        })
        .collect();

    let points_count = points.len();

    // Upsert points to Qdrant
    if let Err(e) = qdrant_client
        .upsert_points(
            UpsertPointsBuilder::new(&embedded_dataset.collection_name, points).wait(true),
        )
        .await
    {
        error!("Failed to upsert points to Qdrant: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to push vectors: {}", e)
        }));
    }

    info!(
        "Pushed {} vectors to {}",
        points_count, embedded_dataset.collection_name
    );

    HttpResponse::Ok().json(PushVectorsResponse {
        points_inserted: points_count,
        collection_name: embedded_dataset.collection_name,
    })
}
