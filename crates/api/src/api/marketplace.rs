use actix_web::{
    HttpResponse, Responder, ResponseError, get, post,
    web::{Data, Path, Query},
};
use aws_sdk_s3::Client;
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

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Collection>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/collections")]
#[tracing::instrument(name = "get_public_collections", skip(_user, postgres_pool))]
pub(crate) async fn get_public_collections(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match collections::get_public_collections(&postgres_pool.into_inner()).await {
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
#[tracing::instrument(name = "get_recent_public_collections", skip(_user, postgres_pool))]
pub(crate) async fn get_recent_public_collections(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5);
    match collections::get_recent_public_collections(&postgres_pool.into_inner(), limit).await {
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
        ("limit" = i32, Query, description = "Number of recent datasets to fetch"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/datasets/recent")]
#[tracing::instrument(name = "get_recent_public_datasets", skip(_user, postgres_pool))]
pub(crate) async fn get_recent_public_datasets(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5);
    match datasets::get_recent_public_datasets(&postgres_pool.into_inner(), limit).await {
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
#[tracing::instrument(name = "get_recent_public_embedders", skip(_user, postgres_pool))]
pub(crate) async fn get_recent_public_embedders(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5);
    match embedders::get_recent_public_embedders(&postgres_pool.into_inner(), limit).await {
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
#[tracing::instrument(name = "get_recent_public_llms", skip(_user, postgres_pool))]
pub(crate) async fn get_recent_public_llms(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    Query(query): Query<RecentCollectionsQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(5);
    match llms::get_recent_public_llms(&postgres_pool.into_inner(), limit).await {
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
        (status = 200, description = "OK", body = Vec<Dataset>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/datasets")]
#[tracing::instrument(name = "get_public_datasets", skip(_user, postgres_pool))]
pub(crate) async fn get_public_datasets(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match datasets::get_public_datasets(&postgres_pool.into_inner()).await {
        Ok(datasets_list) => HttpResponse::Ok().json(datasets_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch public datasets");
            ApiError::Internal(format!("error fetching public datasets: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Embedder>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Marketplace",
)]
#[get("/api/marketplace/embedders")]
#[tracing::instrument(name = "get_public_embedders", skip(_user, postgres_pool))]
pub(crate) async fn get_public_embedders(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match embedders::get_public_embedders(&postgres_pool.into_inner()).await {
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
    tag = "Marketplace",
)]
#[get("/api/marketplace/llms")]
#[tracing::instrument(name = "get_public_llms", skip(_user, postgres_pool))]
pub(crate) async fn get_public_llms(
    _user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match llms::get_public_llms(&postgres_pool.into_inner()).await {
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
#[tracing::instrument(name = "grab_collection", skip(user, s3_client, postgres_pool))]
pub(crate) async fn grab_collection(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
) -> impl Responder {
    match collections::grab_public_collection(
        &postgres_pool.into_inner(),
        &s3_client.into_inner(),
        &user,
        *collection_id,
    )
    .await
    {
        Ok(collection) => HttpResponse::Created().json(collection),
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
#[tracing::instrument(name = "grab_dataset", skip(user, postgres_pool))]
pub(crate) async fn grab_dataset(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    dataset_id: Path<i32>,
) -> impl Responder {
    match datasets::grab_public_dataset(&postgres_pool.into_inner(), &user, *dataset_id).await {
        Ok(dataset) => HttpResponse::Created().json(dataset),
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
#[tracing::instrument(name = "grab_embedder", skip(user, postgres_pool))]
pub(crate) async fn grab_embedder(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    embedder_id: Path<i32>,
) -> impl Responder {
    match embedders::grab_public_embedder(&postgres_pool.into_inner(), &user, *embedder_id).await {
        Ok(embedder) => HttpResponse::Created().json(embedder),
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
#[tracing::instrument(name = "grab_llm", skip(user, postgres_pool))]
pub(crate) async fn grab_llm(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
) -> impl Responder {
    match llms::grab_public_llm(&postgres_pool.into_inner(), &user, *llm_id).await {
        Ok(llm) => HttpResponse::Created().json(llm),
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to grab LLM");
            ApiError::Internal(format!("error grabbing LLM: {:?}", e)).error_response()
        }
    }
}
