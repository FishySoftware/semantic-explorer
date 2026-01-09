use actix_web::{
    HttpResponse, Responder, delete, get, patch, post,
    web::{Data, Json, Path},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use sqlx::{Pool, Postgres};

use crate::{
    auth::extract_username,
    errors::not_found,
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
#[tracing::instrument(name = "get_llms", skip(auth, postgres_pool))]
pub(crate) async fn get_llms(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match llms::get_llms(&postgres_pool.into_inner(), &username).await {
        Ok(llms_list) => HttpResponse::Ok().json(llms_list),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch LLMs");
            HttpResponse::InternalServerError().body(format!("error fetching LLMs: {e:?}"))
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
#[tracing::instrument(name = "get_llm", skip(auth, postgres_pool))]
pub(crate) async fn get_llm(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match llms::get_llm(&postgres_pool.into_inner(), &username, *llm_id).await {
        Ok(llm) => HttpResponse::Ok().json(llm),
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to fetch LLM");
            not_found(format!("LLM not found: {e:?}"))
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
#[tracing::instrument(name = "create_llm", skip(auth, postgres_pool, create_llm))]
pub(crate) async fn create_llm(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    create_llm: Json<CreateLLM>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let payload = create_llm.into_inner();

    match llms::create_llm(&postgres_pool.into_inner(), &username, &payload).await {
        Ok(llm) => HttpResponse::Created().json(llm),
        Err(e) => {
            tracing::error!(error = %e, "failed to create LLM");
            HttpResponse::InternalServerError().body(format!("error creating LLM: {e:?}"))
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
#[tracing::instrument(name = "update_llm", skip(auth, postgres_pool, update_llm))]
pub(crate) async fn update_llm(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
    update_llm: Json<UpdateLargeLanguageModel>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match llms::update_llm(&postgres_pool.into_inner(), &username, *llm_id, &update_llm).await {
        Ok(llm) => HttpResponse::Ok().json(llm),
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to update LLM");
            HttpResponse::InternalServerError().body(format!("error updating LLM: {e:?}"))
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
#[tracing::instrument(name = "delete_llm", skip(auth, postgres_pool))]
pub(crate) async fn delete_llm(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match llms::delete_llm(&postgres_pool.into_inner(), &username, *llm_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to delete LLM");
            HttpResponse::InternalServerError().body(format!("error deleting LLM: {e:?}"))
        }
    }
}
