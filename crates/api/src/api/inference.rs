//! Inference API endpoints for testing local inference service.

use actix_web::{HttpResponse, Responder, ResponseError, get, post, web::Data, web::Json};
use semantic_explorer_core::{config::InferenceConfig, http_client::HTTP_CLIENT};

use crate::{auth::AuthenticatedUser, errors::ApiError};

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct TestInferenceRequest {
    /// Model to use for embedding
    model: String,
    /// Texts to embed
    texts: Vec<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(crate) struct TestInferenceResponse {
    success: bool,
    message: String,
    dimensions: Option<usize>,
    embedding_count: Option<usize>,
}

#[utoipa::path(
    post,
    path = "/api/inference/test",
    tag = "Inference",
    request_body = TestInferenceRequest,
    responses(
        (status = 200, description = "Test successful", body = TestInferenceResponse),
        (status = 500, description = "Test failed"),
    ),
)]
#[post("/api/inference/test")]
#[tracing::instrument(name = "test_inference", skip(_user, inference_config, request))]
pub(crate) async fn test_inference_embedder(
    _user: AuthenticatedUser,
    inference_config: Data<InferenceConfig>,
    request: Json<TestInferenceRequest>,
) -> impl Responder {
    let payload = request.into_inner();

    let body = serde_json::json!({
        "texts": payload.texts,
        "model": payload.model,
    });

    let url = format!(
        "{}/api/embed/batch",
        inference_config.url.trim_end_matches('/')
    );

    match HTTP_CLIENT.post(&url).json(&body).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                match response.text().await {
                    Ok(text) => {
                        return ApiError::Internal(format!("HTTP {}: {}", status, text))
                            .error_response();
                    }
                    Err(_) => {
                        return ApiError::Internal(format!(
                            "HTTP {}: check inference-api is running",
                            status
                        ))
                        .error_response();
                    }
                }
            }

            match response.json::<serde_json::Value>().await {
                Ok(json) => {
                    let embedding_count = json["embeddings"]
                        .as_array()
                        .map(|arr| arr.len())
                        .unwrap_or(0);

                    let dimensions = json["embeddings"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|first| first.as_array())
                        .map(|arr| arr.len());

                    HttpResponse::Ok().json(TestInferenceResponse {
                        success: true,
                        message: format!(
                            "Inference test successful - generated {} embedding(s) with {} dimensions",
                            embedding_count,
                            dimensions.unwrap_or(0)
                        ),
                        dimensions,
                        embedding_count: Some(embedding_count),
                    })
                }
                Err(e) => {
                    ApiError::Internal(format!("failed to parse response: {}", e)).error_response()
                }
            }
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "request timeout (inference-api may be loading models)".to_string()
            } else if e.is_connect() {
                format!(
                    "failed to connect to inference-api at {} - is it running?",
                    inference_config.url
                )
            } else {
                format!("{}", e)
            };
            ApiError::Internal(format!("inference test failed: {}", error_msg)).error_response()
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

#[utoipa::path(
    get,
    path = "/api/inference/models/embedders",
    tag = "Inference",
    responses(
        (status = 200, description = "List of available embedding models", body = Vec<ModelInfo>),
        (status = 500, description = "Failed to fetch models"),
    ),
)]
#[get("/api/inference/models/embedders")]
#[tracing::instrument(name = "list_inference_embedders", skip(_user, inference_config))]
pub(crate) async fn list_inference_embedders(
    _user: AuthenticatedUser,
    inference_config: Data<InferenceConfig>,
) -> impl Responder {
    let url = format!(
        "{}/api/embedders",
        inference_config.url.trim_end_matches('/')
    );

    match HTTP_CLIENT.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                return ApiError::Internal(format!(
                    "HTTP {}: check inference-api is running",
                    status
                ))
                .error_response();
            }

            match response.json::<Vec<ModelInfo>>().await {
                Ok(mut models) => {
                    // Sort and deduplicate by ID
                    models.sort_by(|a, b| a.id.cmp(&b.id));
                    models.dedup_by(|a, b| a.id == b.id);
                    HttpResponse::Ok().json(models)
                }
                Err(e) => ApiError::Internal(format!("Failed to parse embedder models: {}", e))
                    .error_response(),
            }
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "Request timeout (inference-api may be loading models)".to_string()
            } else if e.is_connect() {
                format!(
                    "Failed to connect to inference-api at {} - is it running?",
                    inference_config.url
                )
            } else {
                format!("{}", e)
            };
            ApiError::Internal(format!("Failed to fetch embedder models: {}", error_msg))
                .error_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/inference/models/rerankers",
    tag = "Inference",
    responses(
        (status = 200, description = "List of available reranker models", body = Vec<ModelInfo>),
        (status = 500, description = "Failed to fetch models"),
    ),
)]
#[get("/api/inference/models/rerankers")]
#[tracing::instrument(name = "list_inference_rerankers", skip(_user, inference_config))]
pub(crate) async fn list_inference_rerankers(
    _user: AuthenticatedUser,
    inference_config: Data<InferenceConfig>,
) -> impl Responder {
    let url = format!(
        "{}/api/rerankers",
        inference_config.url.trim_end_matches('/')
    );

    match HTTP_CLIENT.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                return ApiError::Internal(format!(
                    "HTTP {}: check inference-api is running",
                    status
                ))
                .error_response();
            }

            match response.json::<Vec<ModelInfo>>().await {
                Ok(mut models) => {
                    // Sort and deduplicate by ID
                    models.sort_by(|a, b| a.id.cmp(&b.id));
                    models.dedup_by(|a, b| a.id == b.id);
                    HttpResponse::Ok().json(models)
                }
                Err(e) => ApiError::Internal(format!("Failed to parse reranker models: {}", e))
                    .error_response(),
            }
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "Request timeout (inference-api may be loading models)".to_string()
            } else if e.is_connect() {
                format!(
                    "Failed to connect to inference-api at {} - is it running?",
                    inference_config.url
                )
            } else {
                format!("{}", e)
            };
            ApiError::Internal(format!("Failed to fetch reranker models: {}", error_msg))
                .error_response()
        }
    }
}
