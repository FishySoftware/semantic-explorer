//! Embedding API endpoints.

use actix_web::{HttpResponse, Responder, ResponseError, get, post, web};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{info, instrument};
use utoipa::ToSchema;

use crate::config::ModelConfig;
use crate::embedding;
use crate::errors::InferenceError;

/// Add backpressure headers to successful responses so callers can
/// adaptively throttle without waiting for 503 failures.
///
/// Headers:
/// - `X-Queue-Depth`: current number of pending requests for this model
/// - `X-Queue-Capacity`: maximum queue capacity
/// - `X-Estimated-Wait-Ms`: estimated wait time for a new request based on EMA latency
fn add_backpressure_headers(response: &mut actix_web::HttpResponseBuilder, model_id: &str) {
    if let Some(status) = embedding::get_queue_status(model_id) {
        response.insert_header(("X-Queue-Depth", status.queue_depth.to_string()));
        response.insert_header(("X-Queue-Capacity", status.queue_capacity.to_string()));
        response.insert_header(("X-Estimated-Wait-Ms", status.estimated_wait_ms.to_string()));
    }
}

/// Request body for single text embedding
#[derive(Debug, Deserialize, ToSchema)]
pub struct EmbedRequest {
    /// Text to embed
    pub text: String,
    /// Model to use (required)
    pub model: String,
}

/// Request body for batch text embedding
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchEmbedRequest {
    /// Texts to embed
    pub texts: Vec<String>,
    /// Model to use (required)
    pub model: String,
}

/// Response for embedding requests
#[derive(Debug, Serialize, ToSchema)]
pub struct EmbedResponse {
    /// The generated embeddings
    pub embeddings: Vec<Vec<f32>>,
    /// Model used
    pub model: String,
    /// Number of embeddings generated
    pub count: usize,
    /// Embedding dimensions
    pub dimensions: usize,
}

use crate::models::get_embedding_models;

/// List available embedding models only
#[utoipa::path(
    get,
    path = "/api/embedders",
    responses(
        (status = 200, description = "List of available embedding models", body = Vec<crate::models::ModelInfo>),
        (status = 500, description = "Internal server error")
    ),
    tag = "models"
)]
#[get("/api/embedders")]
#[instrument(skip(config))]
pub async fn list_embedders(config: web::Data<ModelConfig>) -> impl Responder {
    let embedders = get_embedding_models(&config);
    HttpResponse::Ok().json(embedders)
}

/// Generate embedding for a single text
#[utoipa::path(
    post,
    path = "/api/embed",
    request_body = EmbedRequest,
    responses(
        (status = 200, description = "Embedding generated successfully", body = EmbedResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "embedding"
)]
#[post("/api/embed")]
#[instrument(skip(config), fields(model))]
pub async fn embed(
    config: web::Data<ModelConfig>,
    body: web::Json<EmbedRequest>,
) -> impl Responder {
    let model_id = body.model.clone();
    let text = body.text.clone();

    tracing::Span::current().record("model", &model_id);

    let start = Instant::now();

    // Generate embeddings asynchronously
    let result = embedding::generate_embeddings(&model_id, &config, vec![text]).await;

    let duration = start.elapsed().as_secs_f64();

    match result {
        Ok(embeddings) => {
            semantic_explorer_core::observability::record_embed_request(
                &model_id, 1, duration, true,
            );
            info!(
                "Generated embedding for model {} in {} seconds",
                model_id, duration
            );
            let dimensions = embeddings.first().map(|e| e.len()).unwrap_or(0);
            let mut response = HttpResponse::Ok();
            add_backpressure_headers(&mut response, &model_id);
            response.json(EmbedResponse {
                embeddings,
                model: model_id,
                count: 1,
                dimensions,
            })
        }
        Err(e) => {
            semantic_explorer_core::observability::record_embed_request(
                &model_id, 1, duration, false,
            );
            e.error_response()
        }
    }
}

/// Generate embeddings for multiple texts
#[utoipa::path(
    post,
    path = "/api/embed/batch",
    request_body = BatchEmbedRequest,
    responses(
        (status = 200, description = "Embeddings generated successfully", body = EmbedResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "embedding"
)]
#[post("/api/embed/batch")]
#[instrument(skip(config, body), fields(model, count = body.texts.len()))]
pub async fn embed_batch(
    config: web::Data<ModelConfig>,
    body: web::Json<BatchEmbedRequest>,
) -> impl Responder {
    if body.texts.is_empty() {
        return HttpResponse::Ok().json(EmbedResponse {
            embeddings: vec![],
            model: body.model.clone(),
            count: 0,
            dimensions: 0,
        });
    }

    if body.texts.len() > config.max_batch_size {
        return InferenceError::BadRequest(format!(
            "Batch size {} exceeds maximum {}",
            body.texts.len(),
            config.max_batch_size
        ))
        .error_response();
    }

    let model_id = body.model.clone();
    let texts = body.texts.clone();

    tracing::Span::current().record("model", &model_id);

    let item_count = texts.len() as u64;
    let start = Instant::now();

    // Generate embeddings asynchronously
    let result = embedding::generate_embeddings(&model_id, &config, texts).await;

    let duration = start.elapsed().as_secs_f64();

    match result {
        Ok(embeddings) => {
            semantic_explorer_core::observability::record_embed_request(
                &model_id, item_count, duration, true,
            );
            info!(
                "Generated {item_count} embeddings for model {} in {} seconds",
                model_id, duration
            );
            let count = embeddings.len();
            let dimensions = embeddings.first().map(|e| e.len()).unwrap_or(0);
            let mut response = HttpResponse::Ok();
            add_backpressure_headers(&mut response, &model_id);
            response.json(EmbedResponse {
                embeddings,
                model: model_id,
                count,
                dimensions,
            })
        }
        Err(e) => {
            semantic_explorer_core::observability::record_embed_request(
                &model_id, item_count, duration, false,
            );
            e.error_response()
        }
    }
}
