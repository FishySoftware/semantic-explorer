use actix_web::{
    HttpResponse, Responder, delete, get, patch, post,
    web::{Data, Json, Path, Query},
};

use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, DeletePointsBuilder, FieldCondition, Filter, Match as QdrantMatch,
        condition::ConditionOneOf, r#match::MatchValue, points_selector::PointsSelectorOneOf,
    },
};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tracing::error;

use crate::{
    auth::extract_username,
    datasets::models::{
        CreateDataset, CreateDatasetItems, CreateDatasetItemsResponse, Dataset, DatasetItemChunks,
        DatasetWithStats, PaginatedDatasetItemSummaries, PaginatedDatasetItems, PaginationParams,
    },
    embedders::models::Embedder,
    errors::{bad_request, not_found},
    storage::postgres::{datasets, embedded_datasets, embedders},
};

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<DatasetWithStats>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets")]
#[tracing::instrument(name = "get_datasets", skip(auth, postgres_pool))]
pub(crate) async fn get_datasets(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let pool = postgres_pool.into_inner();

    // Get datasets
    let all_datasets = match datasets::get_datasets(&pool, &username).await {
        Ok(datasets) => datasets,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("error fetching datasets: {e:?}"));
        }
    };

    // Get stats
    let stats = match datasets::get_dataset_stats(&pool, &username).await {
        Ok(stats) => stats,
        Err(e) => {
            error!("error fetching dataset stats: {e:?}");
            // Continue without stats
            Vec::new()
        }
    };

    // Create a map of dataset_id -> stats
    let stats_map: HashMap<i32, (i64, i64)> = stats
        .into_iter()
        .map(|s| (s.dataset_id, (s.item_count, s.total_chunks)))
        .collect();

    // Merge datasets with stats
    let datasets_with_stats: Vec<DatasetWithStats> = all_datasets
        .into_iter()
        .map(|d| {
            let (item_count, total_chunks) = stats_map.get(&d.dataset_id).unwrap_or(&(0, 0));
            DatasetWithStats {
                dataset_id: d.dataset_id,
                title: d.title,
                details: d.details,
                owner: d.owner,
                tags: d.tags,
                is_public: d.is_public,
                created_at: d.created_at,
                updated_at: d.updated_at,
                item_count: *item_count,
                total_chunks: *total_chunks,
            }
        })
        .collect();

    HttpResponse::Ok().json(datasets_with_stats)
}

#[utoipa::path(
    params(
        ("dataset_id" = i32, Path, description = "The dataset ID"),
    ),
    responses(
        (status = 200, description = "OK", body = Dataset),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets/{dataset_id}")]
#[tracing::instrument(name = "get_dataset", skip(auth, postgres_pool))]
pub(crate) async fn get_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let dataset_id = path.into_inner();
    let pool = postgres_pool.into_inner();

    match datasets::get_dataset(&pool, &username, dataset_id).await {
        Ok(dataset) => HttpResponse::Ok().json(dataset),
        Err(e) => {
            error!("Failed to fetch dataset {}: {}", dataset_id, e);
            not_found(format!("Dataset not found: {}", e))
        }
    }
}

#[utoipa::path(
    request_body = CreateDataset,
    responses(
        (status = 201, description = "Created"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[post("/api/datasets")]
#[tracing::instrument(name = "create_dataset", skip(auth, postgres_pool, create_dataset), fields(dataset_title = %create_dataset.title))]
pub(crate) async fn create_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    Json(create_dataset): Json<CreateDataset>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let dataset = match datasets::create_dataset(
        &postgres_pool.into_inner(),
        &create_dataset.title,
        create_dataset.details.as_deref(),
        &username,
        &create_dataset.tags,
        create_dataset.is_public,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("error creating dataset due to: {e:?}"));
        }
    };

    HttpResponse::Created().json(dataset)
}

#[utoipa::path(
    request_body = CreateDataset,
    responses(
        (status = 200, description = "Updated"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[patch("/api/datasets/{dataset_id}")]
#[tracing::instrument(name = "update_dataset", skip(auth, postgres_pool, update_dataset), fields(dataset_id = %dataset_id.as_ref(), dataset_title = %update_dataset.title))]
pub(crate) async fn update_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Json(update_dataset): Json<CreateDataset>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &username, dataset_id)
        .await
        .is_err()
    {
        return HttpResponse::NotFound().finish();
    }

    let dataset = match datasets::update_dataset(
        &postgres_pool,
        dataset_id,
        &update_dataset.title,
        update_dataset.details.as_deref(),
        &username,
        &update_dataset.tags,
        update_dataset.is_public,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("error updating dataset due to: {e:?}"));
        }
    };

    HttpResponse::Ok().json(dataset)
}

#[utoipa::path(
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[delete("/api/datasets/{datasets_id}")]
#[tracing::instrument(name = "delete_dataset", skip(auth, postgres_pool), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn delete_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &username, dataset_id)
        .await
        .is_err()
    {
        return not_found("Dataset not found");
    };

    match datasets::delete_dataset(&postgres_pool, dataset_id, &username).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError()
            .body(format!("error deleting dataset due to: {e:?}")),
    }
}

#[utoipa::path(
    request_body(content = CreateDatasetItems),
    params(
        ("dataset_id", description = "Dataset ID"),
     ),
    responses(
        (status = 201, description = "Created", body = CreateDatasetItemsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Bad Request (dataset does not exist)"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[post("/api/datasets/{dataset_id}/items")]
#[tracing::instrument(name = "upload_to_dataset", skip(auth, postgres_pool, payload), fields(dataset_id = %dataset_id.as_ref(), item_count = payload.items.len()))]
pub(crate) async fn upload_to_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Json(payload): Json<CreateDatasetItems>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    let dataset = match datasets::get_dataset(&postgres_pool, &username, dataset_id).await {
        Ok(dataset) => dataset,
        Err(_) => {
            return bad_request(format!("dataset '{dataset_id}' does not exists"));
        }
    };

    // Prepare items for batch insert
    let batch_items: Vec<(String, Vec<_>, serde_json::Value)> = payload
        .items
        .into_iter()
        .map(|item| (item.title, item.chunks, item.metadata))
        .collect();

    let (completed, failed) =
        match datasets::create_dataset_items_batch(&postgres_pool, dataset.dataset_id, batch_items)
            .await
        {
            Ok((items, failed_titles)) => (
                items.into_iter().map(|item| item.title).collect(),
                failed_titles,
            ),
            Err(e) => {
                error!("error batch uploading items to dataset '{dataset_id}': {e:?}");
                return HttpResponse::InternalServerError()
                    .body(format!("failed to upload items: {e}"));
            }
        };

    HttpResponse::Ok().json(CreateDatasetItemsResponse { completed, failed })
}

#[utoipa::path(
    params(
        ("dataset_id", description = "Dataset ID"),
        ("page" = Option<i64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<i64>, Query, description = "Number of items per page"),
     ),
    responses(
        (status = 200, description = "OK", body = PaginatedDatasetItems),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Bad Request (dataset does not exist)"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets/{dataset_id}/items")]
#[tracing::instrument(name = "get_dataset_items", skip(auth, postgres_pool, params), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn get_dataset_items(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Query(params): Query<PaginationParams>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &username, dataset_id)
        .await
        .is_err()
    {
        return bad_request(format!(
            "dataset '{dataset_id}' does not exist or access denied"
        ));
    }

    let page = params.page.max(0);
    let page_size = params.page_size.clamp(1, 100);

    let items = match datasets::get_dataset_items(&postgres_pool, dataset_id, page, page_size).await
    {
        Ok(items) => items,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("error fetching dataset items: {e:?}"));
        }
    };

    let total_count = match datasets::count_dataset_items(&postgres_pool, dataset_id).await {
        Ok(count) => count,
        Err(e) => {
            error!("error counting dataset items: {e:?}");
            return HttpResponse::InternalServerError()
                .body(format!("error counting dataset items: {e:?}"));
        }
    };

    let has_more = (page + 1) * page_size < total_count;

    HttpResponse::Ok().json(PaginatedDatasetItems {
        items,
        page,
        page_size,
        total_count,
        has_more,
    })
}

#[utoipa::path(
    params(
        ("dataset_id" = i32, Path, description = "The dataset ID"),
        ("page" = i64, Query, description = "Page number (0-indexed)"),
        ("page_size" = i64, Query, description = "Number of items per page"),
    ),
    responses(
        (status = 200, description = "OK", body = PaginatedDatasetItemSummaries),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Bad Request (dataset does not exist)"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets/{dataset_id}/items-summary")]
#[tracing::instrument(name = "get_dataset_items_summary", skip(auth, postgres_pool, params), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn get_dataset_items_summary(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Query(params): Query<PaginationParams>,
) -> impl Responder {
    use crate::datasets::models::PaginatedDatasetItemSummaries;

    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &username, dataset_id)
        .await
        .is_err()
    {
        return bad_request(format!(
            "dataset '{dataset_id}' does not exist or access denied"
        ));
    }

    let page = params.page.max(0);
    let page_size = params.page_size.clamp(1, 100);

    let items = match datasets::get_dataset_items_summary(
        &postgres_pool,
        dataset_id,
        page,
        page_size,
    )
    .await
    {
        Ok(items) => items,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("error fetching dataset items: {e:?}"));
        }
    };

    let total_count = match datasets::count_dataset_items(&postgres_pool, dataset_id).await {
        Ok(count) => count,
        Err(e) => {
            error!("error counting dataset items: {e:?}");
            return HttpResponse::InternalServerError()
                .body(format!("error counting dataset items: {e:?}"));
        }
    };

    let has_more = (page + 1) * page_size < total_count;

    HttpResponse::Ok().json(PaginatedDatasetItemSummaries {
        items,
        page,
        page_size,
        total_count,
        has_more,
    })
}

#[utoipa::path(
    params(
        ("dataset_id" = i32, Path, description = "The dataset ID"),
        ("item_id" = i32, Path, description = "The item ID"),
    ),
    responses(
        (status = 200, description = "OK", body = DatasetItemChunks),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets/{dataset_id}/items/{item_id}/chunks")]
#[tracing::instrument(name = "get_dataset_item_chunks", skip(auth, postgres_pool, path))]
pub(crate) async fn get_dataset_item_chunks(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let (dataset_id, item_id) = path.into_inner();

    if datasets::get_dataset(&postgres_pool, &username, dataset_id)
        .await
        .is_err()
    {
        return not_found("Dataset not found or access denied");
    }

    match datasets::get_dataset_item_chunks(&postgres_pool, dataset_id, item_id).await {
        Ok(Some(chunks)) => HttpResponse::Ok().json(chunks),
        Ok(None) => not_found("Item not found"),
        Err(e) => {
            error!("error fetching dataset item chunks: {e:?}");
            HttpResponse::InternalServerError()
                .body(format!("error fetching dataset item chunks: {e:?}"))
        }
    }
}

#[utoipa::path(
    params(
        ("dataset_id" = i32, Path, description = "The dataset ID"),
    ),
    responses(
        (status = 200, description = "OK"),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Bad Request (dataset does not exist)"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[delete("/api/datasets/{dataset_id}/items/{item_id}")]
#[tracing::instrument(name = "delete_dataset_item", skip(auth, postgres_pool, qdrant_client))]
pub(crate) async fn delete_dataset_item(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let (dataset_id, item_id) = path.into_inner();

    if datasets::get_dataset(&postgres_pool, &username, dataset_id)
        .await
        .is_err()
    {
        return not_found("Dataset not found or access denied");
    }

    let deleted_item =
        match datasets::delete_dataset_item(&postgres_pool, item_id, dataset_id).await {
            Ok(item) => item,
            Err(e) => {
                error!("error deleting dataset item from database: {e:?}");
                return HttpResponse::InternalServerError()
                    .body(format!("error deleting dataset item: {e:?}"));
            }
        };

    // Get embedded datasets for this dataset to find collections to clean up
    let embedded_datasets_list = match embedded_datasets::get_embedded_datasets_for_dataset(
        &postgres_pool,
        &username,
        dataset_id,
    )
    .await
    {
        Ok(eds) => eds,
        Err(e) => {
            error!("error fetching embedded datasets: {e:?}");
            // Continue anyway - we deleted from DB
            return HttpResponse::Ok().finish();
        }
    };

    let collection_names: Vec<String> = embedded_datasets_list
        .into_iter()
        .map(|ed| ed.collection_name)
        .collect();

    if !collection_names.is_empty() {
        for collection_name in collection_names {
            let filter = Filter {
                must: vec![Condition {
                    condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                        key: "metadata.item_id".to_string(),
                        r#match: Some(QdrantMatch {
                            match_value: Some(MatchValue::Integer(item_id as i64)),
                        }),
                        ..Default::default()
                    })),
                }],
                ..Default::default()
            };

            let selector = PointsSelectorOneOf::Filter(filter);

            let delete_request = DeletePointsBuilder::new(&collection_name).points(selector);

            match qdrant_client.delete_points(delete_request).await {
                Ok(_result) => {
                    tracing::info!(
                        "Deleted chunks from collection '{}' for item_id {}",
                        collection_name,
                        item_id
                    );
                }
                Err(e) => {
                    error!(
                        "error deleting chunks from Qdrant collection '{}': {e:?}",
                        collection_name
                    );
                }
            }
        }
    }

    HttpResponse::Ok().json(deleted_item)
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(crate) struct DatasetEmbedders {
    pub(crate) dataset_id: i32,
    pub(crate) embedders: Vec<Embedder>,
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<DatasetEmbedders>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets/embedders")]
#[tracing::instrument(name = "get_datasets_embedders", skip(auth, postgres_pool))]
pub(crate) async fn get_datasets_embedders(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let pool = postgres_pool.into_inner();

    // Get all datasets for the user
    let all_datasets = match datasets::get_datasets(&pool, &username).await {
        Ok(datasets) => datasets,
        Err(e) => {
            error!("error fetching datasets: {e:?}");
            return HttpResponse::InternalServerError()
                .body(format!("error fetching datasets: {e:?}"));
        }
    };

    // Get all embedders to look up names
    let all_embedders = match embedders::get_embedders(&pool, &username).await {
        Ok(embedders) => embedders,
        Err(e) => {
            error!("error fetching embedders: {e:?}");
            return HttpResponse::InternalServerError()
                .body(format!("error fetching embedders: {e:?}"));
        }
    };

    let embedders_map: HashMap<i32, Embedder> = all_embedders
        .into_iter()
        .map(|e| (e.embedder_id, e))
        .collect();

    let mut dataset_embedders_map: HashMap<i32, Vec<Embedder>> = HashMap::new();

    // For each dataset, get embedded datasets and extract unique embedders
    for dataset in all_datasets {
        let embedded_datasets_list = match embedded_datasets::get_embedded_datasets_for_dataset(
            &pool,
            &username,
            dataset.dataset_id,
        )
        .await
        {
            Ok(eds) => eds,
            Err(e) => {
                error!("error fetching embedded datasets: {e:?}");
                continue;
            }
        };

        for ed in embedded_datasets_list {
            if let Some(embedder) = embedders_map.get(&ed.embedder_id) {
                let embedders_list = dataset_embedders_map.entry(dataset.dataset_id).or_default();
                // Only add if not already present
                if !embedders_list
                    .iter()
                    .any(|e| e.embedder_id == embedder.embedder_id)
                {
                    embedders_list.push(embedder.clone());
                }
            }
        }
    }

    let result: Vec<DatasetEmbedders> = dataset_embedders_map
        .into_iter()
        .map(|(dataset_id, embedders)| DatasetEmbedders {
            dataset_id,
            embedders,
        })
        .collect();

    HttpResponse::Ok().json(result)
}
