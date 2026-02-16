use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    embedders::models::{
        CreateEmbedder, Embedder, EmbedderListQuery, PaginatedEmbedderList, UpdateEmbedder,
    },
    errors::ApiError,
    storage::{
        postgres::embedders,
        valkey::{self, ValkeyClients},
    },
};
use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{self, Data, Json, Path},
};
use semantic_explorer_core::validation;
use semantic_explorer_core::{
    config::{EmbeddingInferenceConfig, ValkeyConfig},
    encryption::EncryptionService,
    http_client::HTTP_CLIENT,
};
use sqlx::{Pool, Postgres};

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(crate) struct TestEmbedderResponse {
    success: bool,
    message: String,
    dimensions: Option<usize>,
}

#[utoipa::path(
    params(
        EmbedderListQuery
    ),
    responses(
        (status = 200, description = "OK", body = Vec<Embedder>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Embedders",
)]
#[get("/api/embedders")]
#[tracing::instrument(
    name = "get_embedders",
    skip(user, pool, query, encryption, valkey, valkey_config)
)]
pub(crate) async fn get_embedders(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    query: web::Query<EmbedderListQuery>,
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
            "embedders:{}:{}:{}",
            user.as_owner(),
            query.limit,
            query.offset
        ))
    } else {
        None
    };

    // Check Valkey cache first
    if let (Some(key), Some(v)) = (&cache_key, &valkey)
        && let Some(cached) = valkey::cache_get::<PaginatedEmbedderList>(&v.read, key).await
    {
        return HttpResponse::Ok().json(cached);
    }

    let result = match search_query {
        Some(q) => {
            embedders::get_embedders_with_search(
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
            embedders::get_embedders(
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
            let response = PaginatedEmbedderList {
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
                let resp_clone = PaginatedEmbedderList {
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
            tracing::error!(error = %e, "failed to fetch embedders");
            ApiError::Internal(format!("error fetching embedders: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Embedder),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("embedder_id" = i32, Path, description = "Embedder ID")
    ),
    tag = "Embedders",
)]
#[get("/api/embedders/{embedder_id}")]
#[tracing::instrument(name = "get_embedder", skip(user, pool, encryption))]
pub(crate) async fn get_embedder(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    embedder_id: Path<i32>,
) -> impl Responder {
    match embedders::get_embedder(&pool.into_inner(), &user, *embedder_id, &encryption).await {
        Ok(embedder) => {
            events::resource_read(
                &user.as_owner(),
                &user,
                ResourceType::Embedder,
                &embedder_id.to_string(),
            );
            HttpResponse::Ok().json(embedder)
        }
        Err(e) => {
            tracing::error!(error = %e, embedder_id = %embedder_id, "failed to fetch embedder");
            ApiError::NotFound(format!("embedder not found: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    request_body = CreateEmbedder,
    responses(
        (status = 201, description = "Created", body = Embedder),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Embedders",
)]
#[post("/api/embedders")]
#[tracing::instrument(
    name = "create_embedder",
    skip(user, pool, create_embedder, req, encryption, valkey)
)]
pub(crate) async fn create_embedder(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    create_embedder: Json<CreateEmbedder>,
    valkey: Option<Data<ValkeyClients>>,
) -> impl Responder {
    let mut payload = create_embedder.into_inner();

    // Default api_key to "dummy" if not provided or empty.
    if payload.api_key.as_ref().is_none_or(|k| k.is_empty()) {
        payload.api_key = Some("dummy".to_string());
    }

    if let Err(e) = validation::validate_title(&payload.name) {
        return ApiError::Validation(e).error_response();
    }

    // Validate base_url is not empty for non-internal providers
    if payload.provider != "internal" && payload.base_url.trim().is_empty() {
        return ApiError::BadRequest("base_url cannot be empty for this provider".to_string())
            .error_response();
    }

    match embedders::create_embedder(&pool.into_inner(), &user, &payload, &encryption).await {
        Ok(embedder) => {
            events::resource_created_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::Embedder,
                &embedder.embedder_id.to_string(),
            );
            valkey::invalidate_resource_cache(valkey.as_ref(), "embedders", &user.as_owner());
            HttpResponse::Created().json(embedder)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create embedder");
            ApiError::Internal(format!("error creating embedder: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    request_body = UpdateEmbedder,
    responses(
        (status = 200, description = "OK", body = Embedder),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("embedder_id" = i32, Path, description = "Embedder ID")
    ),
    tag = "Embedders",
)]
#[patch("/api/embedders/{embedder_id}")]
#[tracing::instrument(
    name = "update_embedder",
    skip(user, pool, update_embedder, encryption, valkey)
)]
pub(crate) async fn update_embedder(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    embedder_id: Path<i32>,
    update_embedder: Json<UpdateEmbedder>,
    valkey: Option<Data<ValkeyClients>>,
) -> impl Responder {
    if let Some(ref name) = update_embedder.name
        && let Err(e) = validation::validate_title(name)
    {
        return ApiError::Validation(e).error_response();
    }

    match embedders::update_embedder(
        &pool.into_inner(),
        &user,
        *embedder_id,
        &update_embedder,
        &encryption,
    )
    .await
    {
        Ok(embedder) => {
            events::resource_updated(
                &user.as_owner(),
                &user,
                ResourceType::Embedder,
                &embedder_id.to_string(),
            );

            // Audit log configuration changes if sensitive fields were updated
            if let Some(api_key) = &update_embedder.api_key
                && !api_key.is_empty()
            {
                crate::audit::events::configuration_changed(
                    &user.as_owner(),
                    &user,
                    ResourceType::Embedder,
                    &embedder_id.to_string(),
                    "api_key",
                );
            }
            if update_embedder.name.is_some() {
                crate::audit::events::configuration_changed(
                    &user.as_owner(),
                    &user,
                    ResourceType::Embedder,
                    &embedder_id.to_string(),
                    "name",
                );
            }

            valkey::invalidate_resource_cache(valkey.as_ref(), "embedders", &user.as_owner());
            HttpResponse::Ok().json(embedder)
        }
        Err(e) => {
            tracing::error!(error = %e, embedder_id = %embedder_id, "failed to update embedder");
            ApiError::Internal(format!("error updating embedder: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    responses(
        (status = 204, description = "No Content"),
        (status = 404, description = "Not Found"),
        (status = 409, description = "Conflict - embedder has associated embedded datasets"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("embedder_id" = i32, Path, description = "Embedder ID")
    ),
    tag = "Embedders",
)]
#[delete("/api/embedders/{embedder_id}")]
#[tracing::instrument(name = "delete_embedder", skip(user, pool, req, valkey))]
pub(crate) async fn delete_embedder(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    embedder_id: Path<i32>,
    valkey: Option<Data<ValkeyClients>>,
) -> impl Responder {
    let pool = pool.into_inner();

    // Check for associated embedded datasets before deleting
    match crate::storage::postgres::embedded_datasets::count_by_embedder(&pool, &user, *embedder_id)
        .await
    {
        Ok(count) if count > 0 => {
            return ApiError::Conflict(format!(
                "Cannot delete embedder: {} embedded dataset(s) still reference it. Delete them first.",
                count
            ))
            .error_response();
        }
        Err(e) => {
            tracing::error!(error = %e, embedder_id = %embedder_id, "failed to check embedded datasets for embedder");
            return ApiError::Internal(format!("error checking embedder dependencies: {:?}", e))
                .error_response();
        }
        _ => {}
    }

    match embedders::delete_embedder(&pool, &user, *embedder_id).await {
        Ok(()) => {
            events::resource_deleted_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::Embedder,
                &embedder_id.to_string(),
            );
            valkey::invalidate_resource_cache(valkey.as_ref(), "embedders", &user.as_owner());
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            tracing::error!(error = %e, embedder_id = %embedder_id, "failed to delete embedder");
            ApiError::Internal(format!("error deleting embedder: {:?}", e)).error_response()
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/embedders/{embedder_id}/test",
    tag = "Embedders",
    params(
        ("embedder_id" = i32, Path, description = "Embedder ID")
    ),
    responses(
        (status = 200, description = "Test successful", body = TestEmbedderResponse),
        (status = 404, description = "Embedder not found"),
        (status = 500, description = "Test failed"),
    ),
)]
#[post("/api/embedders/{embedder_id}/test")]
#[tracing::instrument(name = "test_embedder", skip(user, pool, encryption, inference_config), fields(embedder_id = %path.as_ref()))]
pub(crate) async fn test_embedder(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    encryption: Data<EncryptionService>,
    inference_config: Data<EmbeddingInferenceConfig>,
    path: Path<i32>,
) -> impl Responder {
    let embedder_id = path.into_inner();

    // Fetch the embedder (api_key is decrypted by storage layer)
    let embedder =
        match embedders::get_embedder(&pool.into_inner(), &user, embedder_id, &encryption).await {
            Ok(e) => e,
            Err(e) => {
                return ApiError::NotFound(format!("embedder not found: {}", e)).error_response();
            }
        };

    let test_text = "This is a test embedding request to verify the embedder is working correctly.";

    let result = match embedder.provider.as_str() {
        "openai" => {
            let model = embedder
                .config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("text-embedding-ada-002");

            let body = serde_json::json!({
                "input": test_text,
                "model": model,
            });

            let mut req = HTTP_CLIENT
                .post(format!(
                    "{}/embeddings",
                    embedder.base_url.trim_end_matches('/')
                ))
                .json(&body);

            if let Some(api_key) = &embedder.api_key {
                req = req.bearer_auth(api_key);
            }

            match req.send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        Err(format!(
                            "HTTP {}: check API key and base URL",
                            response.status()
                        ))
                    } else {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if let Some(dims) = json["data"][0]["embedding"].as_array() {
                                    Ok(dims.len())
                                } else {
                                    Err("unexpected response format".to_string())
                                }
                            }
                            Err(e) => Err(format!("failed to parse response: {}", e)),
                        }
                    }
                }
                Err(e) => {
                    let error_msg = if e.is_timeout() {
                        "request timeout (check network/firewall)".to_string()
                    } else if e.is_connect() {
                        "failed to connect (check base URL)".to_string()
                    } else {
                        format!("{}", e)
                    };
                    Err(error_msg)
                }
            }
        }
        "cohere" => {
            let model = embedder
                .config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("embed-english-v3.0");

            let body = serde_json::json!({
                "texts": [test_text],
                "model": model,
                "input_type": "search_document",
            });

            let base = embedder.base_url.trim_end_matches('/');
            let url = if base.ends_with("/embed") {
                base.to_string()
            } else {
                format!("{}/embed", base)
            };

            let mut req = HTTP_CLIENT.post(&url).json(&body);

            if let Some(api_key) = &embedder.api_key {
                req = req.bearer_auth(api_key);
            }

            match req.send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        match response.text().await {
                            Ok(text) => Err(format!("HTTP {}: {}", status, text)),
                            Err(_) => Err(format!("HTTP {}: check API key and base URL", status)),
                        }
                    } else {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if let Some(dims) = json["embeddings"][0].as_array() {
                                    Ok(dims.len())
                                } else {
                                    Err("unexpected response format (check model)".to_string())
                                }
                            }
                            Err(e) => Err(format!("failed to parse response: {}", e)),
                        }
                    }
                }
                Err(e) => {
                    let error_msg = if e.is_timeout() {
                        "request timeout (check network/firewall)".to_string()
                    } else if e.is_connect() {
                        "failed to connect (check base URL)".to_string()
                    } else {
                        format!("{}", e)
                    };
                    Err(error_msg)
                }
            }
        }
        "internal" => {
            // Test internal embedding-inference-api service using configured URL
            let model = embedder
                .config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("BAAI/bge-small-en-v1.5");

            let body = serde_json::json!({
                "text": test_text,
                "model": model,
            });

            let url = format!("{}/api/embed", inference_config.url.trim_end_matches('/'));
            let req = HTTP_CLIENT.post(&url).json(&body);

            match req.send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        match response.text().await {
                            Ok(text) => Err(format!("HTTP {}: {}", status, text)),
                            Err(_) => Err(format!(
                                "HTTP {}: check embedding-inference-api URL",
                                status
                            )),
                        }
                    } else {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if let Some(dims) = json["dimensions"].as_u64() {
                                    Ok(dims as usize)
                                } else if let Some(embeddings) = json["embeddings"].as_array() {
                                    if let Some(first) = embeddings.first() {
                                        if let Some(arr) = first.as_array() {
                                            Ok(arr.len())
                                        } else {
                                            Err("unexpected embedding format".to_string())
                                        }
                                    } else {
                                        Err("empty embeddings array".to_string())
                                    }
                                } else {
                                    Err("unexpected response format".to_string())
                                }
                            }
                            Err(e) => Err(format!("failed to parse response: {}", e)),
                        }
                    }
                }
                Err(e) => {
                    let error_msg = if e.is_timeout() {
                        "request timeout (embedding-inference-api may be loading models)"
                            .to_string()
                    } else if e.is_connect() {
                        "failed to connect (check embedding-inference-api URL)".to_string()
                    } else {
                        format!("{}", e)
                    };
                    Err(error_msg)
                }
            }
        }
        _ => Err(format!("unsupported provider: {}", embedder.provider)),
    };

    match result {
        Ok(dimensions) => HttpResponse::Ok().json(TestEmbedderResponse {
            success: true,
            message: format!(
                "embedder test successful - received {} dimensional embeddings",
                dimensions
            ),
            dimensions: Some(dimensions),
        }),
        Err(error) => {
            tracing::warn!(
                embedder_id = embedder_id,
                error = %error,
                "embedder test failed"
            );
            ApiError::Internal(format!("embedder test failed: {}", error)).error_response()
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ModelInfo {
    /// Model identifier (HuggingFace repo format)
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Model description
    pub description: String,
    /// Model type (embedding or reranker)
    pub model_type: String,
    /// Output dimensions (for embeddings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}
