use actix_web::{
    HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{Data, Json, Path},
};
use semantic_explorer_core::validation;
use sqlx::{Pool, Postgres};

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    errors::ApiError,
    llms::models::{CreateLLM, LargeLanguageModel, UpdateLargeLanguageModel},
    storage::postgres::llms,
};

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<LargeLanguageModel>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "LLMs",
)]
#[get("/api/llms")]
#[tracing::instrument(name = "get_llms", skip(user, postgres_pool))]
pub(crate) async fn get_llms(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match llms::get_llms(&postgres_pool.into_inner(), &user).await {
        Ok(llms_list) => HttpResponse::Ok().json(llms_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch LLMs");
            ApiError::Internal(format!("error fetching LLMs: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = LargeLanguageModel),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("llm_id" = i32, Path, description = "LLM ID")
    ),
    tag = "LLMs",
)]
#[get("/api/llms/{llm_id}")]
#[tracing::instrument(name = "get_llm", skip(user, postgres_pool))]
pub(crate) async fn get_llm(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
) -> impl Responder {
    match llms::get_llm(&postgres_pool.into_inner(), &user, *llm_id).await {
        Ok(llm) => {
            events::resource_read(&user, ResourceType::LlmProvider, &llm_id.to_string());
            HttpResponse::Ok().json(llm)
        }
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to fetch LLM");
            ApiError::NotFound(format!("LLM not found: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    request_body = CreateLLM,
    responses(
        (status = 201, description = "Created", body = LargeLanguageModel),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "LLMs",
)]
#[post("/api/llms")]
#[tracing::instrument(name = "create_llm", skip(user, postgres_pool, create_llm))]
pub(crate) async fn create_llm(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    create_llm: Json<CreateLLM>,
) -> impl Responder {
    let payload = create_llm.into_inner();

    // Validate input
    if let Err(e) = validation::validate_title(&payload.name) {
        return ApiError::Validation(e).error_response();
    }

    match llms::create_llm(&postgres_pool.into_inner(), &user, &payload).await {
        Ok(llm) => {
            events::resource_created(&user, ResourceType::LlmProvider, &llm.llm_id.to_string());
            HttpResponse::Created().json(llm)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create LLM");
            ApiError::Internal(format!("error creating LLM: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    request_body = UpdateLargeLanguageModel,
    responses(
        (status = 200, description = "OK", body = LargeLanguageModel),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("llm_id" = i32, Path, description = "LLM ID")
    ),
    tag = "LLMs",
)]
#[patch("/api/llms/{llm_id}")]
#[tracing::instrument(name = "update_llm", skip(user, postgres_pool, update_llm))]
pub(crate) async fn update_llm(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
    update_llm: Json<UpdateLargeLanguageModel>,
) -> impl Responder {
    // Validate input if name is provided
    if let Some(ref name) = update_llm.name
        && let Err(e) = validation::validate_title(name)
    {
        return ApiError::Validation(e).error_response();
    }

    match llms::update_llm(&postgres_pool.into_inner(), &user, *llm_id, &update_llm).await {
        Ok(llm) => {
            events::resource_updated(&user, ResourceType::LlmProvider, &llm_id.to_string());
            HttpResponse::Ok().json(llm)
        }
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to update LLM");
            ApiError::Internal(format!("error updating LLM: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 204, description = "No Content"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("llm_id" = i32, Path, description = "LLM ID")
    ),
    tag = "LLMs",
)]
#[delete("/api/llms/{llm_id}")]
#[tracing::instrument(name = "delete_llm", skip(user, postgres_pool))]
pub(crate) async fn delete_llm(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
) -> impl Responder {
    match llms::delete_llm(&postgres_pool.into_inner(), &user, *llm_id).await {
        Ok(()) => {
            events::resource_deleted(&user, ResourceType::LlmProvider, &llm_id.to_string());
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to delete LLM");
            ApiError::Internal(format!("error deleting LLM: {:?}", e)).error_response()
        }
    }
}
