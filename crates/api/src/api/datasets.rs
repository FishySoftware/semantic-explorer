use actix_web::{
    HttpResponse, Responder, delete, get, patch, post,
    web::{Data, Json, Path, Query},
};

use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::{
    Qdrant,
    qdrant::{Condition, DeletePointsBuilder, FieldCondition, Filter, Match as QdrantMatch},
};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tracing::error;

use crate::{
    auth::extract_username,
    datasets::models::{
        CreateDataset, CreateDatasetItems, CreateDatasetItemsResponse, Dataset,
        PaginatedDatasetItems, PaginationParams,
    },
    embedders::models::Embedder,
    storage::postgres::{datasets, embedders, transforms},
};

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Dataset>),
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
    match datasets::get_datasets(&postgres_pool.into_inner(), &username).await {
        Ok(datasets) => HttpResponse::Ok().json(datasets),
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("error fetching collections: {e:?}"))
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
        return HttpResponse::NotFound().finish();
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
            return HttpResponse::BadRequest()
                .body(format!("dataset '{dataset_id}' does not exists"));
        }
    };

    let mut completed = Vec::with_capacity(payload.items.len());
    let mut failed = Vec::new();

    for item in payload.items {
        let title = item.title;
        match datasets::create_dataset_item(
            &postgres_pool,
            dataset.dataset_id,
            &title,
            &item.chunks,
            item.metadata,
        )
        .await
        {
            Ok(_) => completed.push(title),
            Err(e) => {
                failed.push(title.clone());
                error!("error uploading item '{title}' to dataset '{dataset_id}': {e:?}");
                continue;
            }
        }
    }

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
        return HttpResponse::BadRequest().body(format!(
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
        ("dataset_id", description = "Dataset ID"),
        ("item_id", description = "Item ID"),
     ),
    responses(
        (status = 200, description = "OK"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not Found"),
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
        return HttpResponse::NotFound().body("Dataset not found or access denied");
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

    let user_transforms = match transforms::get_transforms(&postgres_pool, &username).await {
        Ok(transforms) => transforms,
        Err(e) => {
            error!("error fetching transforms: {e:?}");
            // Continue anyway - we deleted from DB
            return HttpResponse::Ok().finish();
        }
    };

    let mut collection_mappings: std::collections::HashMap<i32, String> =
        std::collections::HashMap::new();

    for transform in user_transforms {
        if (transform.dataset_id == dataset_id || transform.source_dataset_id == Some(dataset_id))
            && let Some(embedder_ids) = &transform.embedder_ids
        {
            for embedder_id in embedder_ids {
                if let Some(collection_name) = transform.get_collection_name(*embedder_id) {
                    collection_mappings.insert(*embedder_id, collection_name);
                }
            }
        }
    }

    if !collection_mappings.is_empty() {
        for (embedder_id, collection_name) in collection_mappings {
            let filter = Filter {
                must: vec![Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: "metadata.item_id".to_string(),
                            r#match: Some(QdrantMatch {
                                match_value: Some(
                                    qdrant_client::qdrant::r#match::MatchValue::Integer(
                                        item_id as i64,
                                    ),
                                ),
                            }),
                            ..Default::default()
                        }),
                    ),
                }],
                ..Default::default()
            };

            let selector =
                qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(filter);

            let delete_request = DeletePointsBuilder::new(&collection_name).points(selector);

            match qdrant_client.delete_points(delete_request).await {
                Ok(_result) => {
                    tracing::info!(
                        "Deleted chunks from collection '{}' for embedder {} item_id {}",
                        collection_name,
                        embedder_id,
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
pub(crate) struct EmbeddedDataset {
    pub(crate) embedder_id: i32,
    pub(crate) embedder_name: String,
    pub(crate) collection_name: String,
    pub(crate) transform_id: i32,
    pub(crate) transform_title: String,
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<EmbeddedDataset>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Dataset not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Datasets",
)]
#[get("/api/datasets/{dataset_id}/embedded-datasets")]
#[tracing::instrument(name = "get_dataset_embedded_datasets", skip(auth, postgres_pool))]
pub(crate) async fn get_dataset_embedded_datasets(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let dataset_id = dataset_id.into_inner();
    let pool = postgres_pool.into_inner();

    // Verify dataset exists and user has access
    if datasets::get_dataset(&pool, &username, dataset_id)
        .await
        .is_err()
    {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": "dataset not found or access denied"
        }));
    }

    // Get all dataset_to_vector_storage transforms for this dataset
    let vector_transforms =
        match transforms::get_embedded_datasets_for_dataset(&pool, &username, dataset_id).await {
            Ok(transforms) => transforms,
            Err(e) => {
                error!("error fetching transforms: {e:?}");
                return HttpResponse::InternalServerError()
                    .body(format!("error fetching transforms: {e:?}"));
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

    let mut embedded_datasets = Vec::new();

    for transform in vector_transforms {
        if let Some(embedder_ids) = transform.embedder_ids {
            for embedder_id in embedder_ids {
                if let Some(embedder) = embedders_map.get(&embedder_id) {
                    // Get collection name from collection_mappings
                    if let Some(collection_name) = transform
                        .collection_mappings
                        .get(embedder_id.to_string())
                        .and_then(|v| v.as_str())
                    {
                        embedded_datasets.push(EmbeddedDataset {
                            embedder_id,
                            embedder_name: embedder.name.clone(),
                            collection_name: collection_name.to_string(),
                            transform_id: transform.transform_id,
                            transform_title: transform.title.clone(),
                        });
                    }
                }
            }
        }
    }

    HttpResponse::Ok().json(embedded_datasets)
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

    let transforms_list = match transforms::get_transforms(&pool, &username).await {
        Ok(transforms) => transforms,
        Err(e) => {
            error!("error fetching transforms: {e:?}");
            return HttpResponse::InternalServerError()
                .body(format!("error fetching transforms: {e:?}"));
        }
    };

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

    for transform in transforms_list {
        if let Some(embedder_ids) = transform.embedder_ids {
            for embedder_id in embedder_ids {
                if let Some(embedder) = embedders_map.get(&embedder_id) {
                    dataset_embedders_map
                        .entry(transform.dataset_id)
                        .or_default()
                        .push(embedder.clone());
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
