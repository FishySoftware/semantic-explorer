//! Health check endpoints for the inference API.

use actix_web::{HttpResponse, Responder, get};
use serde::Serialize;
use utoipa::ToSchema;

use crate::embedding;
use crate::reranker;

/// Health status response
#[derive(Serialize, ToSchema)]
pub struct HealthStatus {
    status: String,
    service: String,
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

/// Health check
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service health status", body = HealthStatus)
    ),
    tag = "health"
)]
#[get("/health")]
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(HealthStatus {
        status: "ok".to_string(),
        service: "inference-api".to_string(),
    })
}
