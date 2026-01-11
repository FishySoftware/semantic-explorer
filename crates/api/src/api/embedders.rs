use actix_web::{
    HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{Data, Json, Path},
};
use semantic_explorer_core::validation;
use sqlx::{Pool, Postgres};

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    embedders::models::{CreateEmbedder, Embedder, UpdateEmbedder},
    errors::ApiError,
    storage::postgres::embedders,
};

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(crate) struct TestEmbedderResponse {
    success: bool,
    message: String,
    dimensions: Option<usize>,
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Embedder>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Embedders",
)]
#[get("/api/embedders")]
#[tracing::instrument(name = "get_embedders", skip(user, postgres_pool))]
pub(crate) async fn get_embedders(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match embedders::get_embedders(&postgres_pool.into_inner(), &user).await {
        Ok(embedders_list) => HttpResponse::Ok().json(embedders_list),
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
#[tracing::instrument(name = "get_embedder", skip(user, postgres_pool))]
pub(crate) async fn get_embedder(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    embedder_id: Path<i32>,
) -> impl Responder {
    match embedders::get_embedder(&postgres_pool.into_inner(), &user, *embedder_id).await {
        Ok(embedder) => {
            events::resource_read(&user, ResourceType::Embedder, &embedder_id.to_string());
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
#[tracing::instrument(name = "create_embedder", skip(user, postgres_pool, create_embedder))]
pub(crate) async fn create_embedder(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    create_embedder: Json<CreateEmbedder>,
) -> impl Responder {
    let payload = create_embedder.into_inner();

    // Validate input
    if let Err(e) = validation::validate_title(&payload.name) {
        return ApiError::Validation(e).error_response();
    }

    match embedders::create_embedder(&postgres_pool.into_inner(), &user, &payload).await {
        Ok(embedder) => {
            events::resource_created(
                &user,
                ResourceType::Embedder,
                &embedder.embedder_id.to_string(),
            );
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
#[tracing::instrument(name = "update_embedder", skip(user, postgres_pool, update_embedder))]
pub(crate) async fn update_embedder(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    embedder_id: Path<i32>,
    update_embedder: Json<UpdateEmbedder>,
) -> impl Responder {
    // Validate input if name is provided
    if let Some(ref name) = update_embedder.name
        && let Err(e) = validation::validate_title(name)
    {
        return ApiError::Validation(e).error_response();
    }

    match embedders::update_embedder(
        &postgres_pool.into_inner(),
        &user,
        *embedder_id,
        &update_embedder,
    )
    .await
    {
        Ok(embedder) => {
            events::resource_updated(&user, ResourceType::Embedder, &embedder_id.to_string());
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
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("embedder_id" = i32, Path, description = "Embedder ID")
    ),
    tag = "Embedders",
)]
#[delete("/api/embedders/{embedder_id}")]
#[tracing::instrument(name = "delete_embedder", skip(user, postgres_pool))]
pub(crate) async fn delete_embedder(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    embedder_id: Path<i32>,
) -> impl Responder {
    match embedders::delete_embedder(&postgres_pool.into_inner(), &user, *embedder_id).await {
        Ok(()) => {
            events::resource_deleted(&user, ResourceType::Embedder, &embedder_id.to_string());
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
#[tracing::instrument(name = "test_embedder", skip(user, postgres_pool), fields(embedder_id = %path.as_ref()))]
pub(crate) async fn test_embedder(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let embedder_id = path.into_inner();

    // Fetch the embedder
    let embedder =
        match embedders::get_embedder(&postgres_pool.into_inner(), &user, embedder_id).await {
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

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build();

            match client {
                Ok(c) => {
                    let mut req = c
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
                                        if let Some(dims) = json["data"][0]["embedding"].as_array()
                                        {
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
                Err(e) => Err(format!("failed to create HTTP client: {}", e)),
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

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build();

            match client {
                Ok(c) => {
                    let base = embedder.base_url.trim_end_matches('/');
                    let url = if base.ends_with("/embed") {
                        base.to_string()
                    } else {
                        format!("{}/embed", base)
                    };

                    let mut req = c.post(&url).json(&body);

                    if let Some(api_key) = &embedder.api_key {
                        req = req.bearer_auth(api_key);
                    }

                    match req.send().await {
                        Ok(response) => {
                            if !response.status().is_success() {
                                let status = response.status();
                                match response.text().await {
                                    Ok(text) => Err(format!("HTTP {}: {}", status, text)),
                                    Err(_) => {
                                        Err(format!("HTTP {}: check API key and base URL", status))
                                    }
                                }
                            } else {
                                match response.json::<serde_json::Value>().await {
                                    Ok(json) => {
                                        if let Some(dims) = json["embeddings"][0].as_array() {
                                            Ok(dims.len())
                                        } else {
                                            Err("unexpected response format (check model)"
                                                .to_string())
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
                Err(e) => Err(format!("failed to create HTTP client: {}", e)),
            }
        }
        _ => Err(format!("unsupported provider: {}", embedder.provider)),
    };

    match result {
        Ok(dimensions) => {
            tracing::info!(
                embedder_id = embedder_id,
                dimensions = dimensions,
                "embedder test successful"
            );
            HttpResponse::Ok().json(TestEmbedderResponse {
                success: true,
                message: format!(
                    "embedder test successful - received {} dimensional embeddings",
                    dimensions
                ),
                dimensions: Some(dimensions),
            })
        }
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
