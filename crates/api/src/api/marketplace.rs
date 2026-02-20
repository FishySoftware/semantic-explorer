use actix_web::{
    HttpResponse, Responder, ResponseError, get, post,
    web::{Data, Path, Query},
};
use aws_sdk_s3::Client;
use semantic_explorer_core::{config::S3Config, encryption::EncryptionService};
use serde::Deserialize;
use sqlx::{Pool, Postgres};

use crate::{
    auth::AuthenticatedUser,
    collections::models::Collection,
    datasets::models::Dataset,
    embedders::models::Embedder,
    errors::ApiError,
    llms::models::LargeLanguageModel as LLMModel,
    storage::postgres::{collections, datasets, embedders, llms},
};

#[derive(Debug, Deserialize)]
pub(crate) struct RecentCollectionsQuery {
    pub(crate) limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MarketplacePaginationParams {
    #[serde(default = "default_marketplace_limit")]
    pub(crate) limit: i64,
    #[serde(default)]
    pub(crate) offset: i64,
}
fn default_marketplace_limit() -> i64 {
    50
}
const MAX_MARKETPLACE_LIMIT: i64 = 200;

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Collection>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i64, Query, description = "Maximum number of results"),
        ("offset" = i64, Query, description = "Offset for pagination"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/collections")]
#[tracing::instrument(name = "get_public_collections", skip(_user, pool))]
pub(crate) async fn get_public_collections(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    Query(params): Query<MarketplacePaginationParams>,
) -> impl Responder {
    let limit = params.limit.clamp(1, MAX_MARKETPLACE_LIMIT);
    let offset = params.offset.max(0);
    match collections::get_public_collections(&pool.into_inner(), limit, offset).await {
        Ok(collections_list) => HttpResponse::Ok().json(collections_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch public collections");
            ApiError::Internal(format!("error fetching public collections: {:?}", e))
                .error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Collection>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i32, Query, description = "Number of recent collections to fetch"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/collections/recent")]
#[tracing::instrument(name = "get_recent_public_collections", skip(_user, pool))]
pub(crate) async fn get_recent_public_collections(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5).clamp(0, 100);
    match collections::get_recent_public_collections(&pool.into_inner(), limit).await {
        Ok(collections_list) => HttpResponse::Ok().json(collections_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch recent public collections");
            ApiError::Internal(format!("error fetching recent public collections: {:?}", e))
                .error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Dataset>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i64, Query, description = "Maximum number of results"),
        ("offset" = i64, Query, description = "Offset for pagination"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/datasets")]
#[tracing::instrument(name = "get_public_datasets", skip(_user, pool))]
pub(crate) async fn get_public_datasets(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    Query(params): Query<MarketplacePaginationParams>,
) -> impl Responder {
    let limit = params.limit.clamp(1, MAX_MARKETPLACE_LIMIT);
    let offset = params.offset.max(0);
    match datasets::get_public_datasets(&pool.into_inner(), limit, offset).await {
        Ok(datasets_list) => HttpResponse::Ok().json(datasets_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch public datasets");
            ApiError::Internal(format!("error fetching public datasets: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Dataset>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i32, Query, description = "Number of recent datasets to fetch"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/datasets/recent")]
#[tracing::instrument(name = "get_recent_public_datasets", skip(_user, pool))]
pub(crate) async fn get_recent_public_datasets(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5).clamp(0, 100);
    match datasets::get_recent_public_datasets(&pool.into_inner(), limit).await {
        Ok(datasets_list) => HttpResponse::Ok().json(datasets_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch recent public datasets");
            ApiError::Internal(format!("error fetching recent public datasets: {:?}", e))
                .error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Embedder>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i32, Query, description = "Number of recent embedders to fetch"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/embedders/recent")]
#[tracing::instrument(name = "get_recent_public_embedders", skip(_user, pool, encryption))]
pub(crate) async fn get_recent_public_embedders(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5).clamp(0, 100);
    match embedders::get_recent_public_embedders(&pool.into_inner(), limit, &encryption).await {
        Ok(embedders_list) => HttpResponse::Ok().json(embedders_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch recent public embedders");
            ApiError::Internal(format!("error fetching recent public embedders: {:?}", e))
                .error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<LLMModel>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i32, Query, description = "Number of recent LLMs to fetch"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/llms/recent")]
#[tracing::instrument(name = "get_recent_public_llms", skip(_user, pool, encryption))]
pub(crate) async fn get_recent_public_llms(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5).clamp(0, 100);
    match llms::get_recent_public_llms(&pool.into_inner(), limit, &encryption).await {
        Ok(llms_list) => HttpResponse::Ok().json(llms_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch recent public LLMs");
            ApiError::Internal(format!("error fetching recent public LLMs: {:?}", e))
                .error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Embedder>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i64, Query, description = "Maximum number of results"),
        ("offset" = i64, Query, description = "Offset for pagination"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/embedders")]
#[tracing::instrument(name = "get_public_embedders", skip(_user, pool, encryption))]
pub(crate) async fn get_public_embedders(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    Query(params): Query<MarketplacePaginationParams>,
) -> impl Responder {
    let limit = params.limit.clamp(1, MAX_MARKETPLACE_LIMIT);
    let offset = params.offset.max(0);
    match embedders::get_public_embedders(&pool.into_inner(), limit, offset, &encryption).await {
        Ok(embedders_list) => HttpResponse::Ok().json(embedders_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch public embedders");
            ApiError::Internal(format!("error fetching public embedders: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<LLMModel>),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("limit" = i64, Query, description = "Maximum number of results"),
        ("offset" = i64, Query, description = "Offset for pagination"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/llms")]
#[tracing::instrument(name = "get_public_llms", skip(_user, pool, encryption))]
pub(crate) async fn get_public_llms(
    _user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    Query(params): Query<MarketplacePaginationParams>,
) -> impl Responder {
    let limit = params.limit.clamp(1, MAX_MARKETPLACE_LIMIT);
    let offset = params.offset.max(0);
    match llms::get_public_llms(&pool.into_inner(), limit, offset, &encryption).await {
        Ok(llms_list) => HttpResponse::Ok().json(llms_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch public LLMs");
            ApiError::Internal(format!("error fetching public LLMs: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 201, description = "Created", body = Collection),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("collection_id" = i32, Path, description = "Collection ID to grab")
    ),
    tag = "Marketplace",
)]
#[post("/api/marketplace/collections/{collection_id}/grab")]
#[tracing::instrument(name = "grab_collection", skip(user, s3_client, s3_config, pool))]
pub(crate) async fn grab_collection(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
) -> impl Responder {
    match collections::grab_public_collection(
        &pool.into_inner(),
        &s3_client.into_inner(),
        &s3_config.bucket_name,
        &user.as_owner(),
        &user,
        *collection_id,
    )
    .await
    {
        Ok(collection) => {
            // Audit log the marketplace grab
            crate::audit::events::marketplace_grab(
                &user.as_owner(),
                &user,
                crate::audit::ResourceType::Collection,
                &collection_id.to_string(),
            );
            HttpResponse::Created().json(collection)
        }
        Err(e) => {
            tracing::error!(error = %e, collection_id = %collection_id, "failed to grab collection");
            ApiError::Internal(format!("error grabbing collection: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 201, description = "Created", body = Dataset),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("dataset_id" = i32, Path, description = "Dataset ID to grab")
    ),
    tag = "Marketplace",
)]
#[post("/api/marketplace/datasets/{dataset_id}/grab")]
#[tracing::instrument(name = "grab_dataset", skip(user, pool))]
pub(crate) async fn grab_dataset(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
) -> impl Responder {
    match datasets::grab_public_dataset(&pool.into_inner(), &user.as_owner(), &user, *dataset_id)
        .await
    {
        Ok(dataset) => {
            // Audit log the marketplace grab
            crate::audit::events::marketplace_grab(
                &user.as_owner(),
                &user,
                crate::audit::ResourceType::Dataset,
                &dataset_id.to_string(),
            );
            HttpResponse::Created().json(dataset)
        }
        Err(e) => {
            tracing::error!(error = %e, dataset_id = %dataset_id, "failed to grab dataset");
            ApiError::Internal(format!("error grabbing dataset: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 201, description = "Created", body = Embedder),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("embedder_id" = i32, Path, description = "Embedder ID to grab")
    ),
    tag = "Marketplace",
)]
#[post("/api/marketplace/embedders/{embedder_id}/grab")]
#[tracing::instrument(name = "grab_embedder", skip(user, pool, encryption))]
pub(crate) async fn grab_embedder(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    embedder_id: Path<i32>,
) -> impl Responder {
    match embedders::grab_public_embedder(&pool.into_inner(), &user, *embedder_id, &encryption)
        .await
    {
        Ok(embedder) => {
            crate::audit::events::marketplace_grab(
                &user.as_owner(),
                &user,
                crate::audit::ResourceType::Embedder,
                &embedder_id.to_string(),
            );
            HttpResponse::Created().json(embedder)
        }
        Err(e) => {
            tracing::error!(error = %e, embedder_id = %embedder_id, "failed to grab embedder");
            ApiError::Internal(format!("error grabbing embedder: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 201, description = "Created", body = LLMModel),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("llm_id" = i32, Path, description = "LLM ID to grab")
    ),
    tag = "Marketplace",
)]
#[post("/api/marketplace/llms/{llm_id}/grab")]
#[tracing::instrument(name = "grab_llm", skip(user, pool, encryption))]
pub(crate) async fn grab_llm(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    llm_id: Path<i32>,
) -> impl Responder {
    match llms::grab_public_llm(&pool.into_inner(), &user, *llm_id, &encryption).await {
        Ok(llm) => {
            crate::audit::events::marketplace_grab(
                &user.as_owner(),
                &user,
                crate::audit::ResourceType::LlmProvider,
                &llm_id.to_string(),
            );
            HttpResponse::Created().json(llm)
        }
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to grab LLM");
            ApiError::Internal(format!("error grabbing LLM: {:?}", e)).error_response()
        }
    }
}
