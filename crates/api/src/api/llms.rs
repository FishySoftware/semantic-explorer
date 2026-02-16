use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{self, Data, Json, Path},
};
use semantic_explorer_core::config::ValkeyConfig;
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::validation;
use sqlx::{Pool, Postgres};

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    errors::ApiError,
    llms::models::{
        CreateLLM, LargeLanguageModel, LlmListQuery, PaginatedLLMList, UpdateLargeLanguageModel,
    },
    storage::{
        postgres::llms,
        valkey::{self, ValkeyClients},
    },
};

#[utoipa::path(
    params(
        LlmListQuery
    ),
    responses(
        (status = 200, description = "OK", body = Vec<LargeLanguageModel>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "LLMs",
)]
#[get("/api/llms")]
#[tracing::instrument(
    name = "get_llms",
    skip(user, pool, query, encryption, valkey, valkey_config)
)]
pub(crate) async fn get_llms(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    query: web::Query<LlmListQuery>,
    valkey: Option<Data<ValkeyClients>>,
    valkey_config: Option<Data<ValkeyConfig>>,
) -> impl Responder {
    let search_query = query.search.as_ref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    // Only cache non-search listing queries
    let cache_key = if search_query.is_none() {
        Some(format!(
            "llms:{}:{}:{}",
            user.as_owner(),
            query.limit,
            query.offset
        ))
    } else {
        None
    };

    // Check Valkey cache first
    if let (Some(key), Some(v)) = (&cache_key, &valkey)
        && let Some(cached) = valkey::cache_get::<PaginatedLLMList>(&v.read, key).await
    {
        return HttpResponse::Ok().json(cached);
    }

    let result = match search_query {
        Some(q) => {
            llms::get_llms_with_search(
                &pool.into_inner(),
                &user,
                q,
                query.limit,
                query.offset,
                &encryption,
            )
            .await
        }
        None => {
            llms::get_llms(
                &pool.into_inner(),
                &user,
                query.limit,
                query.offset,
                &encryption,
            )
            .await
        }
    };

    match result {
        Ok(result) => {
            let response = PaginatedLLMList {
                items: result.items,
                total_count: result.total_count,
                limit: result.limit,
                offset: result.offset,
            };

            // Write to cache (fire-and-forget)
            if let (Some(key), Some(v)) = (cache_key, valkey) {
                let conn = v.write.clone();
                let ttl = valkey_config
                    .map(|c| c.resource_cache_ttl_secs)
                    .unwrap_or(300);
                let resp_clone = PaginatedLLMList {
                    items: response.items.clone(),
                    total_count: response.total_count,
                    limit: response.limit,
                    offset: response.offset,
                };
                actix_web::rt::spawn(async move {
                    valkey::cache_set(&conn, &key, &resp_clone, ttl).await;
                });
            }

            HttpResponse::Ok().json(response)
        }
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
#[tracing::instrument(name = "get_llm", skip(user, pool, encryption))]
pub(crate) async fn get_llm(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    llm_id: Path<i32>,
) -> impl Responder {
    match llms::get_llm(&pool.into_inner(), &user, *llm_id, &encryption).await {
        Ok(llm) => {
            events::resource_read(
                &user.as_owner(),
                &user,
                ResourceType::LlmProvider,
                &llm_id.to_string(),
            );
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
#[tracing::instrument(
    name = "create_llm",
    skip(user, pool, create_llm, req, encryption, valkey)
)]
pub(crate) async fn create_llm(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    create_llm: Json<CreateLLM>,
    valkey: Option<Data<ValkeyClients>>,
) -> impl Responder {
    let mut payload = create_llm.into_inner();

    // Default api_key to "dummy" if not provided or empty — clients expect a key even if not necessary
    if payload.api_key.as_ref().is_none_or(|k| k.is_empty()) {
        payload.api_key = Some("dummy".to_string());
    }

    if let Err(e) = validation::validate_title(&payload.name) {
        return ApiError::Validation(e).error_response();
    }

    match llms::create_llm(&pool.into_inner(), &user, &payload, &encryption).await {
        Ok(llm) => {
            events::resource_created_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::LlmProvider,
                &llm.llm_id.to_string(),
            );
            valkey::invalidate_resource_cache(valkey.as_ref(), "llms", &user.as_owner());
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
#[tracing::instrument(name = "update_llm", skip(user, pool, update_llm, encryption, valkey))]
pub(crate) async fn update_llm(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    llm_id: Path<i32>,
    update_llm: Json<UpdateLargeLanguageModel>,
    valkey: Option<Data<ValkeyClients>>,
) -> impl Responder {
    if let Some(ref name) = update_llm.name
        && let Err(e) = validation::validate_title(name)
    {
        return ApiError::Validation(e).error_response();
    }

    match llms::update_llm(&pool.into_inner(), &user, *llm_id, &update_llm, &encryption).await {
        Ok(llm) => {
            events::resource_updated(
                &user.as_owner(),
                &user,
                ResourceType::LlmProvider,
                &llm_id.to_string(),
            );

            // Audit log configuration changes if sensitive fields were updated
            if let Some(api_key) = &update_llm.api_key
                && !api_key.is_empty()
            {
                crate::audit::events::configuration_changed(
                    &user.as_owner(),
                    &user,
                    ResourceType::LlmProvider,
                    &llm_id.to_string(),
                    "api_key",
                );
            }
            if update_llm.name.is_some() {
                crate::audit::events::configuration_changed(
                    &user.as_owner(),
                    &user,
                    ResourceType::LlmProvider,
                    &llm_id.to_string(),
                    "name",
                );
            }

            valkey::invalidate_resource_cache(valkey.as_ref(), "llms", &user.as_owner());
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
#[tracing::instrument(name = "delete_llm", skip(user, pool, req, valkey))]
pub(crate) async fn delete_llm(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    llm_id: Path<i32>,
    valkey: Option<Data<ValkeyClients>>,
) -> impl Responder {
    match llms::delete_llm(&pool.into_inner(), &user, *llm_id).await {
        Ok(()) => {
            events::resource_deleted_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::LlmProvider,
                &llm_id.to_string(),
            );
            valkey::invalidate_resource_cache(valkey.as_ref(), "llms", &user.as_owner());
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            tracing::error!(error = %e, llm_id = %llm_id, "failed to delete LLM");
            ApiError::Internal(format!("error deleting LLM: {:?}", e)).error_response()
        }
    }
}
