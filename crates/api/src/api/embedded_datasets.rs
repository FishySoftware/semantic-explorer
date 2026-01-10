use crate::auth::extract_username;
use crate::embedded_datasets::{
    EmbeddedDataset, EmbeddedDatasetProcessedBatch, EmbeddedDatasetStats,
    EmbeddedDatasetWithDetails,
};
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::embedded_datasets;

use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpResponse, Responder, delete, get, patch, post};
use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::{PointId, ScrollPointsBuilder};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tracing::{error, info};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema, Debug)]
pub struct BatchStatsRequest {
    pub embedded_dataset_ids: Vec<i32>,
}

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

#[utoipa::path(
    get,
    path = "/api/embedded-datasets",
    tag = "Embedded Datasets",
    responses(
        (status = 200, description = "List of embedded datasets", body = Vec<EmbeddedDataset>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets")]
#[tracing::instrument(name = "get_embedded_datasets", skip(auth, postgres_pool))]
pub async fn get_embedded_datasets(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match embedded_datasets::get_embedded_datasets(&postgres_pool, &username).await {
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
#[tracing::instrument(name = "get_embedded_dataset", skip(auth, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_embedded_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match embedded_datasets::get_embedded_dataset_with_details(
        &postgres_pool,
        &username,
        path.into_inner(),
    )
    .await
    {
        Ok(dataset) => HttpResponse::Ok().json(dataset),
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
#[tracing::instrument(name = "delete_embedded_dataset", skip(auth, postgres_pool, qdrant_client), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn delete_embedded_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_id = path.into_inner();

    // First get the embedded dataset to retrieve the collection name
    let embedded_dataset = match embedded_datasets::get_embedded_dataset(
        &postgres_pool,
        &username,
        embedded_dataset_id,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            error!("Failed to find embedded dataset: {}", e);
            return not_found(format!("Embedded dataset not found: {}", e));
        }
    };

    // Delete the Qdrant collection
    info!(
        "Deleting Qdrant collection: {}",
        embedded_dataset.collection_name
    );
    if let Err(e) = qdrant_client
        .delete_collection(&embedded_dataset.collection_name)
        .await
    {
        error!(
            "Failed to delete Qdrant collection {}: {}",
            embedded_dataset.collection_name, e
        );
        // Continue with database deletion even if Qdrant deletion fails
        // This prevents orphaned database records
    }

    // Delete the database entry
    match embedded_datasets::delete_embedded_dataset(&postgres_pool, &username, embedded_dataset_id)
        .await
    {
        Ok(_) => {
            info!(
                "Successfully deleted embedded dataset {} and Qdrant collection {}",
                embedded_dataset_id, embedded_dataset.collection_name
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
#[tracing::instrument(name = "get_embedded_dataset_stats", skip(auth, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_embedded_dataset_stats(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_id = path.into_inner();

    match embedded_datasets::get_embedded_dataset(&postgres_pool, &username, embedded_dataset_id)
        .await
    {
        Ok(_) => {
            match embedded_datasets::get_embedded_dataset_stats(&postgres_pool, embedded_dataset_id)
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
    post,
    path = "/api/embedded-datasets/batch-stats",
    tag = "Embedded Datasets",
    request_body = BatchStatsRequest,
    responses(
        (status = 200, description = "Stats for multiple embedded datasets", body = HashMap<i32, EmbeddedDatasetStats>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/embedded-datasets/batch-stats")]
#[tracing::instrument(name = "get_batch_embedded_dataset_stats", skip(auth, postgres_pool))]
pub async fn get_batch_embedded_dataset_stats(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    body: Json<BatchStatsRequest>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_ids = &body.embedded_dataset_ids;

    // Verify all embedded datasets belong to the user
    for &id in embedded_dataset_ids {
        match embedded_datasets::get_embedded_dataset(&postgres_pool, &username, id).await {
            Ok(_) => {}
            Err(_) => {
                return not_found(format!("Embedded dataset {} not found", id));
            }
        }
    }

    match embedded_datasets::get_batch_embedded_dataset_stats(&postgres_pool, embedded_dataset_ids)
        .await
    {
        Ok(stats_map) => HttpResponse::Ok().json(stats_map),
        Err(e) => {
            error!("Failed to get batch stats: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get batch stats: {}", e)
            }))
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
#[tracing::instrument(name = "get_embedded_dataset_points", skip(auth, postgres_pool, qdrant_client), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_embedded_dataset_points(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<i32>,
    query: Query<PointsQuery>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_id = path.into_inner();

    // Verify embedded dataset exists and belongs to user
    let embedded_dataset = match embedded_datasets::get_embedded_dataset(
        &postgres_pool,
        &username,
        embedded_dataset_id,
    )
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

    // next_page_offset is Option<PointId>, we'll convert it to u64 if it's a numeric ID
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
#[tracing::instrument(name = "get_point_vector", skip(auth, postgres_pool, qdrant_client))]
pub async fn get_point_vector(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let (embedded_dataset_id, point_id) = path.into_inner();

    // Verify embedded dataset exists and belongs to user
    let embedded_dataset = match embedded_datasets::get_embedded_dataset(
        &postgres_pool,
        &username,
        embedded_dataset_id,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            error!("Embedded dataset not found: {}", e);
            return not_found(format!("Embedded dataset not found: {}", e));
        }
    };

    // Retrieve specific point with vector
    use qdrant_client::qdrant::GetPointsBuilder;

    // Convert the string point_id to a PointId
    // Try to parse as UUID first, then as u64
    let qdrant_point_id = if let Ok(uuid) = uuid::Uuid::parse_str(&point_id) {
        PointId {
            point_id_options: Some(PointIdOptions::Uuid(
                uuid.to_string(),
            )),
        }
    } else if let Ok(numeric_id) = point_id.parse::<u64>() {
        PointId {
            point_id_options: Some(PointIdOptions::Num(
                numeric_id,
            )),
        }
    } else {
        error!("Failed to parse point_id as UUID or u64: {}", point_id);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Invalid point_id format: must be a valid UUID or numeric ID")
        }));
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
                    // VectorOutput has the data field - clone the entire vector
                    #[allow(deprecated)]
                    Some(vector_output.data.clone())
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
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    responses(
        (status = 200, description = "Processed batches", body = Vec<EmbeddedDatasetProcessedBatch>),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}/processed-batches")]
#[tracing::instrument(name = "get_processed_batches", skip(auth, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_processed_batches(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_id = path.into_inner();

    match embedded_datasets::get_embedded_dataset(&postgres_pool, &username, embedded_dataset_id)
        .await
    {
        Ok(_) => {
            match embedded_datasets::get_processed_batches(&postgres_pool, embedded_dataset_id)
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
        ("dataset_id" = i32, Path, description = "Dataset ID")
    ),
    responses(
        (status = 200, description = "Embedded datasets for dataset (across all transforms)", body = Vec<EmbeddedDataset>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/datasets/{dataset_id}/embedded-datasets")]
#[tracing::instrument(name = "get_embedded_datasets_for_dataset", skip(auth, postgres_pool), fields(dataset_id = %path.as_ref()))]
pub async fn get_embedded_datasets_for_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match embedded_datasets::get_embedded_datasets_for_dataset(
        &postgres_pool,
        &username,
        path.into_inner(),
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateEmbeddedDatasetRequest {
    pub title: String,
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
#[tracing::instrument(name = "update_embedded_dataset", skip(auth, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn update_embedded_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateEmbeddedDatasetRequest>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_id = path.into_inner();

    // Validate title
    if body.title.trim().is_empty() {
        return bad_request("Title cannot be empty");
    }

    match embedded_datasets::update_embedded_dataset_title(
        &postgres_pool,
        &username,
        embedded_dataset_id,
        body.title.trim(),
    )
    .await
    {
        Ok(dataset) => {
            info!(
                "Updated embedded dataset {} with new title",
                embedded_dataset_id
            );
            HttpResponse::Ok().json(dataset)
        }
        Err(e) => {
            error!("Failed to update embedded dataset: {}", e);
            not_found("Embedded dataset not found or not owned by this user")
        }
    }
}
