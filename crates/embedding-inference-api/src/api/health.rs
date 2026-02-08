//! Health check endpoints for the inference API.

use crate::config::ModelConfig;
use crate::embedding;
use crate::reranker;
use actix_web::{HttpResponse, Responder, get, web};

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
pub async fn health_status(_config: web::Data<ModelConfig>) -> impl Responder {
    let embedding_ready = embedding::is_ready();
    let reranker_ready = reranker::is_ready();
    let available_permits = embedding::available_permits();

    HttpResponse::Ok().json(serde_json::json!({
        "status": if embedding_ready { "ok" } else { "not_ready" },
        "embedding_ready": embedding_ready,
        "reranker_ready": reranker_ready,
        "available_permits": available_permits
    }))
}
