use crate::auth::extract_username;
use crate::embedded_datasets::{
    EmbeddedDataset, EmbeddedDatasetProcessedBatch, EmbeddedDatasetStats,
    EmbeddedDatasetWithDetails,
};
use crate::storage::postgres::embedded_datasets;

use actix_web::web::{Data, Path};
use actix_web::{HttpResponse, Responder, delete, get};
use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::Qdrant;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

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
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Embedded dataset not found: {}", e)
            }))
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
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Embedded dataset not found: {}", e)
            }));
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
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Embedded dataset not found: {}", e)
            }))
        }
    }
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
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Embedded dataset not found: {}", e)
            }))
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
