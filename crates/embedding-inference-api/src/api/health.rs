//! Health check endpoints for the inference API.

use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;

use crate::config::ModelConfig;
use crate::embedding;
use crate::reranker;

/// Model capacity status
#[derive(Serialize)]
struct ModelCapacity {
    model_id: String,
    available_permits: usize,
    max_concurrent: usize,
}

/// Liveness probe - always returns OK if the service is running
#[utoipa::path(
    get,
    path = "/health/live",
    responses(
        (status = 200, description = "Service is alive", body = serde_json::Value)
    ),
    tag = "health"
)]
#[get("/health/live")]
pub async fn health_live() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok"
    }))
}

/// Readiness probe - returns OK if models are loaded
#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "Service is ready", body = serde_json::Value),
        (status = 503, description = "Service is not ready", body = serde_json::Value)
    ),
    tag = "health"
)]
#[get("/health/ready")]
pub async fn health_ready() -> impl Responder {
    let embedding_ready = embedding::is_ready();
    let reranker_ready = reranker::is_ready();

    // Service is ready if at least embeddings are loaded
    // Reranker is optional
    if embedding_ready {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "embedding": embedding_ready,
            "reranker": reranker_ready
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "embedding": embedding_ready,
            "reranker": reranker_ready
        }))
    }
}

/// Status endpoint - returns detailed capacity information
#[utoipa::path(
    get,
    path = "/health/status",
    responses(
        (status = 200, description = "Service status with capacity info", body = serde_json::Value)
    ),
    tag = "health"
)]
#[get("/health/status")]
pub async fn health_status(config: web::Data<ModelConfig>) -> impl Responder {
    let embedding_ready = embedding::is_ready();
    let reranker_ready = reranker::is_ready();
    let total_permits = embedding::total_available_permits().await;

    // Get per-model capacity
    let mut model_capacities = Vec::new();
    for model_id in &config.allowed_embedding_models {
        let available = embedding::available_permits_for_model(model_id).await;
        let model_size = embedding::get_model_size(model_id);
        model_capacities.push(ModelCapacity {
            model_id: model_id.clone(),
            available_permits: available,
            max_concurrent: model_size.max_concurrent(),
        });
    }

    HttpResponse::Ok().json(serde_json::json!({
        "status": if embedding_ready { "ok" } else { "not_ready" },
        "embedding_ready": embedding_ready,
        "reranker_ready": reranker_ready,
        "total_available_permits": total_permits,
        "models": model_capacities
    }))
}
