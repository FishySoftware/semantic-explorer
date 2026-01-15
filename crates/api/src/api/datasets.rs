use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{self, Data, Json, Path, Query},
};

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
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    datasets::models::{
        CreateDataset, CreateDatasetItems, CreateDatasetItemsResponse, Dataset, DatasetItemChunks,
        DatasetWithStats, PaginatedDatasetItemSummaries, PaginatedDatasetItems, PaginationParams,
    },
    embedders::models::Embedder,
    errors::ApiError,
    storage::postgres::{datasets, embedded_datasets, embedders},
};
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::validation;

#[utoipa::path(
    params(
        ("search" = Option<String>, Query, description = "Optional search term to filter datasets by title using ILIKE"),
    ),
    responses(
        (status = 200, description = "OK", body = Vec<DatasetWithStats>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets")]
#[tracing::instrument(name = "get_datasets", skip(user, postgres_pool, query))]
pub(crate) async fn get_datasets(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    let pool = postgres_pool.into_inner();
    let search_query = query.get("search").and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    // Get datasets
    let all_datasets = match search_query {
        Some(q) => match datasets::get_datasets_with_search(&pool, &user.as_owner(), q).await {
            Ok(datasets) => datasets,
            Err(e) => {
                return ApiError::Internal(format!("error fetching datasets: {:?}", e))
                    .error_response();
            }
        },
        None => match datasets::get_datasets(&pool, &user.as_owner()).await {
            Ok(datasets) => datasets,
            Err(e) => {
                return ApiError::Internal(format!("error fetching datasets: {:?}", e))
                    .error_response();
            }
        },
    };

    // Get stats
    let stats = match datasets::get_dataset_stats(&pool, &user.as_owner()).await {
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
#[tracing::instrument(name = "get_dataset", skip(user, postgres_pool))]
pub(crate) async fn get_dataset(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let dataset_id = path.into_inner();
    let pool = postgres_pool.into_inner();

    match datasets::get_dataset(&pool, &user.as_owner(), dataset_id).await {
        Ok(dataset) => {
            events::resource_read(&user, ResourceType::Dataset, &dataset_id.to_string());
            HttpResponse::Ok().json(dataset)
        }
        Err(e) => {
            error!("Failed to fetch dataset {}: {}", dataset_id, e);
            ApiError::NotFound(format!("Dataset not found: {}", e)).error_response()
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
#[tracing::instrument(name = "create_dataset", skip(user, postgres_pool, create_dataset, req), fields(dataset_title = %create_dataset.title))]
pub(crate) async fn create_dataset(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    Json(create_dataset): Json<CreateDataset>,
) -> impl Responder {
    // Input validation
    if let Err(e) = validation::validate_title(&create_dataset.title) {
        return ApiError::Validation(e).error_response();
    }
    if let Some(ref details) = create_dataset.details
        && let Err(e) = validation::validate_description(details)
    {
        return ApiError::Validation(e).error_response();
    }
    if let Err(e) = validation::validate_tags(&create_dataset.tags) {
        return ApiError::Validation(e).error_response();
    }

    let dataset = match datasets::create_dataset(
        &postgres_pool.into_inner(),
        &create_dataset.title,
        create_dataset.details.as_deref(),
        &user.as_owner(),
        &create_dataset.tags,
        create_dataset.is_public,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            return ApiError::Internal(format!("error creating dataset due to: {:?}", e))
                .error_response();
        }
    };

    events::resource_created_with_request(
        &req,
        &user,
        ResourceType::Dataset,
        &dataset.dataset_id.to_string(),
    );
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
#[tracing::instrument(name = "update_dataset", skip(user, postgres_pool, update_dataset), fields(dataset_id = %dataset_id.as_ref(), dataset_title = %update_dataset.title))]
pub(crate) async fn update_dataset(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Json(update_dataset): Json<CreateDataset>,
) -> impl Responder {
    // Input validation
    if let Err(e) = validation::validate_title(&update_dataset.title) {
        return ApiError::Validation(e).error_response();
    }
    if let Some(ref details) = update_dataset.details
        && let Err(e) = validation::validate_description(details)
    {
        return ApiError::Validation(e).error_response();
    }
    if let Err(e) = validation::validate_tags(&update_dataset.tags) {
        return ApiError::Validation(e).error_response();
    }

    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id)
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
        &user.as_owner(),
        &update_dataset.tags,
        update_dataset.is_public,
    )
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            return ApiError::Internal(format!("error updating dataset due to: {:?}", e))
                .error_response();
        }
    };

    events::resource_updated(&user, ResourceType::Dataset, &dataset_id.to_string());
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
#[tracing::instrument(name = "delete_dataset", skip(user, postgres_pool, qdrant_client, req), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn delete_dataset(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    dataset_id: Path<i32>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::NotFound("Dataset not found".to_string()).error_response();
    };

    // Get all embedded datasets for this dataset so we can delete their Qdrant collections
    let embedded_datasets = match embedded_datasets::get_embedded_datasets_for_dataset(
        &postgres_pool,
        &user.as_owner(),
        dataset_id,
    )
    .await
    {
        Ok(datasets) => datasets,
        Err(e) => {
            error!("Failed to fetch embedded datasets for deletion: {}", e);
            return ApiError::Internal(format!("error fetching embedded datasets due to: {:?}", e))
                .error_response();
        }
    };

    // Delete Qdrant collections for all embedded datasets
    for embedded_dataset in embedded_datasets {
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

    match datasets::delete_dataset(&postgres_pool, dataset_id, &user.as_owner()).await {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
                &user,
                ResourceType::Dataset,
                &dataset_id.to_string(),
            );
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            ApiError::Internal(format!("error deleting dataset due to: {:?}", e)).error_response()
        }
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
#[tracing::instrument(name = "upload_to_dataset", skip(user, postgres_pool, payload), fields(dataset_id = %dataset_id.as_ref(), item_count = payload.items.len()))]
pub(crate) async fn upload_to_dataset(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Json(payload): Json<CreateDatasetItems>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    let dataset = match datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id).await {
        Ok(dataset) => dataset,
        Err(_) => {
            return ApiError::BadRequest(format!("dataset '{}' does not exist", dataset_id))
                .error_response();
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
                return ApiError::Internal(format!("failed to upload items: {}", e))
                    .error_response();
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
#[tracing::instrument(name = "get_dataset_items", skip(user, postgres_pool, params), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn get_dataset_items(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Query(params): Query<PaginationParams>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::BadRequest(format!(
            "dataset '{}' does not exist or access denied",
            dataset_id
        ))
        .error_response();
    }

    let page = params.page.max(0);
    let page_size = params.page_size.clamp(1, 100);

    let items = match datasets::get_dataset_items(&postgres_pool, dataset_id, page, page_size).await
    {
        Ok(items) => items,
        Err(e) => {
            return ApiError::Internal(format!("error fetching dataset items: {:?}", e))
                .error_response();
        }
    };

    let total_count = match datasets::count_dataset_items(&postgres_pool, dataset_id).await {
        Ok(count) => count,
        Err(e) => {
            error!("error counting dataset items: {e:?}");
            return ApiError::Internal(format!("error counting dataset items: {:?}", e))
                .error_response();
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
        ("search" = Option<String>, Query, description = "Optional search term to filter items by title"),
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
#[tracing::instrument(name = "get_dataset_items_summary", skip(user, postgres_pool, params), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn get_dataset_items_summary(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Query(params): Query<PaginationParams>,
) -> impl Responder {
    use crate::datasets::models::PaginatedDatasetItemSummaries;

    let postgres_pool = postgres_pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::BadRequest(format!(
            "dataset '{}' does not exist or access denied",
            dataset_id
        ))
        .error_response();
    }

    let page = params.page.max(0);
    let page_size = params.page_size.clamp(1, 100);
    let search_query = params
        .search
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let items = match search_query {
        Some(query) => {
            match datasets::get_dataset_items_summary_with_search(
                &postgres_pool,
                dataset_id,
                page,
                page_size,
                query,
            )
            .await
            {
                Ok(items) => items,
                Err(e) => {
                    return ApiError::Internal(format!("error fetching dataset items: {:?}", e))
                        .error_response();
                }
            }
        }
        None => {
            match datasets::get_dataset_items_summary(&postgres_pool, dataset_id, page, page_size)
                .await
            {
                Ok(items) => items,
                Err(e) => {
                    return ApiError::Internal(format!("error fetching dataset items: {:?}", e))
                        .error_response();
                }
            }
        }
    };

    let total_count = match search_query {
        Some(query) => {
            match datasets::count_dataset_items_with_search(&postgres_pool, dataset_id, query).await
            {
                Ok(count) => count,
                Err(e) => {
                    error!("error counting dataset items: {e:?}");
                    return ApiError::Internal(format!("error counting dataset items: {:?}", e))
                        .error_response();
                }
            }
        }
        None => match datasets::count_dataset_items(&postgres_pool, dataset_id).await {
            Ok(count) => count,
            Err(e) => {
                error!("error counting dataset items: {e:?}");
                return ApiError::Internal(format!("error counting dataset items: {:?}", e))
                    .error_response();
            }
        },
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
#[tracing::instrument(name = "get_dataset_item_chunks", skip(user, postgres_pool, path))]
pub(crate) async fn get_dataset_item_chunks(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();
    let (dataset_id, item_id) = path.into_inner();

    if datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::NotFound("Dataset not found or access denied".to_string())
            .error_response();
    }

    match datasets::get_dataset_item_chunks(&postgres_pool, dataset_id, item_id).await {
        Ok(Some(chunks)) => HttpResponse::Ok().json(chunks),
        Ok(None) => ApiError::NotFound("Item not found".to_string()).error_response(),
        Err(e) => {
            error!("error fetching dataset item chunks: {e:?}");
            ApiError::Internal(format!("error fetching dataset item chunks: {:?}", e))
                .error_response()
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
#[tracing::instrument(name = "delete_dataset_item", skip(user, postgres_pool, qdrant_client))]
pub(crate) async fn delete_dataset_item(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();
    let (dataset_id, item_id) = path.into_inner();

    if datasets::get_dataset(&postgres_pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::NotFound("Dataset not found or access denied".to_string())
            .error_response();
    }

    let deleted_item =
        match datasets::delete_dataset_item(&postgres_pool, item_id, dataset_id).await {
            Ok(item) => item,
            Err(e) => {
                error!("error deleting dataset item from database: {e:?}");
                return ApiError::Internal(format!("error deleting dataset item: {:?}", e))
                    .error_response();
            }
        };

    // Get embedded datasets for this dataset to find collections to clean up
    let embedded_datasets_list = match embedded_datasets::get_embedded_datasets_for_dataset(
        &postgres_pool,
        &user.as_owner(),
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
#[tracing::instrument(name = "get_datasets_embedders", skip(user, postgres_pool, encryption))]
pub(crate) async fn get_datasets_embedders(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
) -> impl Responder {
    let pool = postgres_pool.into_inner();

    // Get all datasets for the user
    let all_datasets = match datasets::get_datasets(&pool, &user.as_owner()).await {
        Ok(datasets) => datasets,
        Err(e) => {
            error!("error fetching datasets: {e:?}");
            return ApiError::Internal(format!("error fetching datasets: {:?}", e))
                .error_response();
        }
    };

    // Get all embedders to look up names
    let all_embedders = match embedders::get_embedders(&pool, &user, &encryption).await {
        Ok(embedders) => embedders,
        Err(e) => {
            error!("error fetching embedders: {e:?}");
            return ApiError::Internal(format!("error fetching embedders: {:?}", e))
                .error_response();
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
            &user.as_owner(),
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
