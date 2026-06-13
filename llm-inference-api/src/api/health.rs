//! Health check endpoints for the LLM inference API.

use actix_web::{HttpResponse, Responder, get};

use crate::llm;

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
    let llm_ready = llm::is_ready();

    if llm_ready {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "llm_models_loaded": llm_ready
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "llm_models_loaded": llm_ready
        }))
    }
}
