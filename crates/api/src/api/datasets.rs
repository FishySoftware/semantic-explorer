use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{self, Data, Json, Path, Query},
};

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    datasets::models::{
        CreateDataset, CreateDatasetItems, CreateDatasetItemsResponse, Dataset, DatasetItemChunks,
        DatasetListParams, DatasetWithStats, PaginatedDatasetItemSummaries, PaginatedDatasetItems,
        PaginatedDatasetList, PaginationParams,
    },
    errors::ApiError,
    storage::postgres::{INTERNAL_BATCH_SIZE, datasets, embedded_datasets, fetch_all_batched},
};
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, DeletePointsBuilder, FieldCondition, Filter, Match as QdrantMatch,
        condition::ConditionOneOf, r#match::MatchValue, points_selector::PointsSelectorOneOf,
    },
};
use semantic_explorer_core::config::S3Config;
use semantic_explorer_core::validation;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

#[utoipa::path(
    params(
        ("search" = Option<String>, Query, description = "Optional search term to filter datasets by title using ILIKE"),
        ("limit" = Option<i64>, Query, description = "Number of items to return (default 20)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "OK", body = PaginatedDatasetList),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets")]
#[tracing::instrument(name = "get_datasets", skip(user, pool, query))]
pub(crate) async fn get_datasets(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    query: web::Query<DatasetListParams>,
) -> impl Responder {
    let pool = pool.into_inner();

    // Parse pagination parameters
    let limit: i64 = query.limit.unwrap_or(20).clamp(1, 1000);
    let offset: i64 = query.offset.unwrap_or(0).max(0);

    let search_query = query.search.as_deref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    // Get paginated datasets with stats
    let paginated_result = match search_query {
        Some(query) => {
            match datasets::get_datasets_paginated_search(
                &pool,
                &user.as_owner(),
                query,
                limit,
                offset,
            )
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    return ApiError::Internal(format!("error fetching datasets: {:?}", e))
                        .error_response();
                }
            }
        }
        None => {
            match datasets::get_datasets_paginated(&pool, &user.as_owner(), limit, offset).await {
                Ok(result) => result,
                Err(e) => {
                    return ApiError::Internal(format!("error fetching datasets: {:?}", e))
                        .error_response();
                }
            }
        }
    };

    // Convert to response model
    let datasets_with_stats: Vec<DatasetWithStats> = paginated_result
        .items
        .into_iter()
        .map(|d| DatasetWithStats {
            dataset_id: d.dataset_id,
            title: d.title,
            details: d.details,
            owner_id: d.owner_id,
            owner_display_name: d.owner_display_name,
            tags: d.tags,
            is_public: d.is_public,
            created_at: d.created_at,
            updated_at: d.updated_at,
            item_count: d.item_count as i64,
            total_chunks: d.total_chunks,
            transform_count: d.transform_count,
        })
        .collect();

    let response = PaginatedDatasetList {
        items: datasets_with_stats,
        total_count: paginated_result.total_count,
        limit: paginated_result.limit,
        offset: paginated_result.offset,
    };

    HttpResponse::Ok().json(response)
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
#[tracing::instrument(name = "get_dataset", skip(user, pool))]
pub(crate) async fn get_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let dataset_id = path.into_inner();
    let pool = pool.into_inner();

    match datasets::get_dataset(&pool, &user.as_owner(), dataset_id).await {
        Ok(dataset) => {
            events::resource_read(
                &user.as_owner(),
                &user,
                ResourceType::Dataset,
                &dataset_id.to_string(),
            );
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
#[tracing::instrument(name = "create_dataset", skip(user, pool, create_dataset, req), fields(dataset_title = %create_dataset.title))]
pub(crate) async fn create_dataset(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    Json(create_dataset): Json<CreateDataset>,
) -> impl Responder {
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
        &pool.into_inner(),
        &create_dataset.title,
        create_dataset.details.as_deref(),
        &user.as_owner(),
        &user,
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
        &user.as_owner(),
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
#[tracing::instrument(name = "update_dataset", skip(user, pool, update_dataset), fields(dataset_id = %dataset_id.as_ref(), dataset_title = %update_dataset.title))]
pub(crate) async fn update_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Json(update_dataset): Json<CreateDataset>,
) -> impl Responder {
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

    let pool = pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return HttpResponse::NotFound().finish();
    }

    let dataset = match datasets::update_dataset(
        &pool,
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

    events::resource_updated(
        &user.as_owner(),
        &user,
        ResourceType::Dataset,
        &dataset_id.to_string(),
    );
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
#[tracing::instrument(name = "delete_dataset", skip(user, pool, qdrant_client, s3_client, s3_config, req), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn delete_dataset(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    s3_client: Data<aws_sdk_s3::Client>,
    s3_config: Data<S3Config>,
    dataset_id: Path<i32>,
) -> impl Responder {
    let pool = pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::NotFound("Dataset not found".to_string()).error_response();
    };

    // Get all embedded datasets for this dataset so we can delete their Qdrant collections
    let embedded_datasets = match fetch_all_batched(INTERNAL_BATCH_SIZE, |limit, offset| {
        let pool = pool.clone();
        let owner = user.as_owner();
        async move {
            embedded_datasets::get_embedded_datasets_for_dataset(
                &pool, &owner, dataset_id, limit, offset,
            )
            .await
        }
    })
    .await
    {
        Ok(datasets) => datasets,
        Err(e) => {
            error!("Failed to fetch embedded datasets for deletion: {}", e);
            return ApiError::Internal(format!("error fetching embedded datasets due to: {:?}", e))
                .error_response();
        }
    };

    // Delete Qdrant collections and clean up S3 batch files for all embedded datasets
    for embedded_dataset in &embedded_datasets {
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

        // Clean up S3 batch files so in-flight workers fail fast on download
        // instead of wasting embedding tokens on orphaned jobs
        let prefix = format!(
            "embedded-datasets/embedded-dataset-{}/",
            embedded_dataset.embedded_dataset_id
        );
        match semantic_explorer_core::storage::delete_files_by_prefix(
            &s3_client,
            &s3_config.bucket_name,
            &prefix,
        )
        .await
        {
            Ok(count) => {
                if count > 0 {
                    info!(
                        embedded_dataset_id = embedded_dataset.embedded_dataset_id,
                        deleted_files = count,
                        "Cleaned up S3 batch files for deleted embedded dataset"
                    );
                }
            }
            Err(e) => {
                error!(
                    "Failed to cleanup S3 batch files for embedded dataset {}: {}",
                    embedded_dataset.embedded_dataset_id, e
                );
                // Continue with deletion even if S3 cleanup fails
            }
        }
    }

    match datasets::delete_dataset(&pool, dataset_id, &user.as_owner()).await {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
                &user.as_owner(),
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
#[tracing::instrument(name = "upload_to_dataset", skip(user, pool, payload), fields(dataset_id = %dataset_id.as_ref(), item_count = payload.items.len()))]
pub(crate) async fn upload_to_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Json(payload): Json<CreateDatasetItems>,
) -> impl Responder {
    let pool = pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    let dataset = match datasets::get_dataset(&pool, &user.as_owner(), dataset_id).await {
        Ok(dataset) => dataset,
        Err(_) => {
            return ApiError::BadRequest(format!("dataset '{}' does not exist", dataset_id))
                .error_response();
        }
    };

    // Prepare items for batch insert
    let item_start = std::time::Instant::now();
    let batch_items: Vec<(String, Vec<_>, serde_json::Value)> = payload
        .items
        .into_iter()
        .map(|item| (item.title, item.chunks, item.metadata))
        .collect();

    let (completed, failed) =
        match datasets::create_dataset_items_batch(&pool, dataset.dataset_id, batch_items).await {
            Ok((items, failed_titles)) => {
                let item_duration = item_start.elapsed().as_secs_f64();
                semantic_explorer_core::observability::record_document_upload(
                    "dataset",
                    item_duration,
                    true,
                );
                (
                    items.into_iter().map(|item| item.title).collect(),
                    failed_titles,
                )
            }
            Err(e) => {
                let item_duration = item_start.elapsed().as_secs_f64();
                semantic_explorer_core::observability::record_document_upload(
                    "dataset",
                    item_duration,
                    false,
                );
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
#[tracing::instrument(name = "get_dataset_items", skip(user, pool, params), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn get_dataset_items(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Query(params): Query<PaginationParams>,
) -> impl Responder {
    let pool = pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&pool, &user.as_owner(), dataset_id)
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

    let items = match datasets::get_dataset_items(&pool, dataset_id, page, page_size).await {
        Ok(items) => items,
        Err(e) => {
            return ApiError::Internal(format!("error fetching dataset items: {:?}", e))
                .error_response();
        }
    };

    let total_count = match datasets::count_dataset_items(&pool, dataset_id).await {
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
#[tracing::instrument(name = "get_dataset_items_summary", skip(user, pool, params), fields(dataset_id = %dataset_id.as_ref()))]
pub(crate) async fn get_dataset_items_summary(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
    Query(params): Query<PaginationParams>,
) -> impl Responder {
    let pool = pool.into_inner();
    let dataset_id = dataset_id.into_inner();

    if datasets::get_dataset(&pool, &user.as_owner(), dataset_id)
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
                &pool, dataset_id, page, page_size, query,
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
            match datasets::get_dataset_items_summary(&pool, dataset_id, page, page_size).await {
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
            match datasets::count_dataset_items_with_search(&pool, dataset_id, query).await {
                Ok(count) => count,
                Err(e) => {
                    error!("error counting dataset items: {e:?}");
                    return ApiError::Internal(format!("error counting dataset items: {:?}", e))
                        .error_response();
                }
            }
        }
        None => match datasets::count_dataset_items(&pool, dataset_id).await {
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
#[tracing::instrument(name = "get_dataset_item_chunks", skip(user, pool, path))]
pub(crate) async fn get_dataset_item_chunks(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let pool = pool.into_inner();
    let (dataset_id, item_id) = path.into_inner();

    if datasets::get_dataset(&pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::NotFound("Dataset not found or access denied".to_string())
            .error_response();
    }

    match datasets::get_dataset_item_chunks(&pool, dataset_id, item_id).await {
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
#[tracing::instrument(name = "delete_dataset_item", skip(user, pool, qdrant_client))]
pub(crate) async fn delete_dataset_item(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let pool = pool.into_inner();
    let (dataset_id, item_id) = path.into_inner();

    if datasets::get_dataset(&pool, &user.as_owner(), dataset_id)
        .await
        .is_err()
    {
        return ApiError::NotFound("Dataset not found or access denied".to_string())
            .error_response();
    }

    let deleted_item = match datasets::delete_dataset_item(&pool, item_id, dataset_id).await {
        Ok(item) => item,
        Err(e) => {
            error!("error deleting dataset item from database: {e:?}");
            return ApiError::Internal(format!("error deleting dataset item: {:?}", e))
                .error_response();
        }
    };

    // Get embedded datasets for this dataset to find collections to clean up
    let embedded_datasets_list = match fetch_all_batched(INTERNAL_BATCH_SIZE, |limit, offset| {
        let pool = pool.clone();
        let owner = user.as_owner();
        async move {
            embedded_datasets::get_embedded_datasets_for_dataset(
                &pool, &owner, dataset_id, limit, offset,
            )
            .await
        }
    })
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
