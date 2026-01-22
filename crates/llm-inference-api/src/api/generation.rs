//! Text generation API endpoints.

use actix_web::{HttpResponse, Responder, ResponseError, get, post, web};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};
use utoipa::ToSchema;

use crate::config::{GenerationConfig, ModelConfig};
use crate::llm;
use crate::models::get_llm_models;

/// Request body for text generation
#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateRequest {
    /// Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
    pub model: String,
    /// Input prompt
    pub prompt: String,
    /// Temperature for sampling (0.0-2.0, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p for nucleus sampling (0.0-1.0, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Maximum number of tokens to generate (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    /// Stop sequences (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Response for text generation
#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateResponse {
    /// Generated text
    pub text: String,
    /// Model used
    pub model: String,
    /// Number of tokens generated
    pub tokens_generated: usize,
    /// Reason generation stopped (length, stop, eos, error)
    pub finish_reason: String,
}

/// List available LLM models
#[utoipa::path(
    get,
    path = "/api/llms",
    responses(
        (status = 200, description = "List of available LLM models", body = Vec<crate::models::ModelInfo>),
        (status = 500, description = "Internal server error")
    ),
    tag = "models"
)]
#[get("/api/llms")]
#[instrument(skip(model_config))]
pub async fn list_llms(model_config: web::Data<ModelConfig>) -> impl Responder {
    let llms = get_llm_models(&model_config);
    HttpResponse::Ok().json(llms)
}

/// Generate text from a prompt
#[utoipa::path(
    post,
    path = "/api/generate",
    request_body = GenerateRequest,
    responses(
        (status = 200, description = "Text generated successfully", body = GenerateResponse),
        (status = 400, description = "Invalid request or unsupported model"),
        (status = 500, description = "Internal server error")
    ),
    tag = "generation"
)]
#[post("/api/generate")]
#[instrument(skip(model_config, gen_config), fields(model))]
pub async fn generate(
    model_config: web::Data<ModelConfig>,
    gen_config: web::Data<GenerationConfig>,
    body: web::Json<GenerateRequest>,
) -> impl Responder {
    // Backpressure: try to acquire a permit, return 503 if at capacity
    let _permit = match llm::try_acquire_permit() {
        Some(permit) => permit,
        None => {
            warn!(
                available_permits = llm::available_permits(),
                "LLM service at capacity, returning 503"
            );
            return HttpResponse::ServiceUnavailable()
                .insert_header(("Retry-After", "10"))
                .json(serde_json::json!({
                    "error": "Service temporarily at capacity",
                    "message": "Too many concurrent LLM requests. Please retry after a short delay.",
                    "retry_after_seconds": 10
                }));
        }
    };

    let model_id = body.model.clone();
    let prompt = body.prompt.clone();

    tracing::Span::current().record("model", &model_id);

    // Build generation parameters
    let params = llm::GenerationParams {
        temperature: body.temperature.unwrap_or(gen_config.default_temperature),
        top_p: body.top_p.unwrap_or(gen_config.default_top_p),
        max_tokens: body.max_tokens.unwrap_or(gen_config.default_max_tokens),
        stop_sequences: body.stop.clone().unwrap_or_default(),
    };

    let start = std::time::Instant::now();

    // Generate text
    match llm::generate_text(&model_id, prompt, params, &model_config, &gen_config).await {
        Ok(result) => {
            let duration = start.elapsed().as_secs_f64();

            info!(
                model = %model_id,
                tokens = result.tokens_generated,
                duration_secs = duration,
                "Generated text successfully"
            );

            semantic_explorer_core::observability::record_llm_request(
                &model_id,
                result.tokens_generated as u64,
                duration,
                true,
            );

            HttpResponse::Ok().json(GenerateResponse {
                text: result.text,
                model: result.model,
                tokens_generated: result.tokens_generated,
                finish_reason: result.finish_reason.to_string(),
            })
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            tracing::error!(
                model = %model_id,
                error = %e,
                duration_secs = duration,
                "Text generation failed"
            );

            semantic_explorer_core::observability::record_llm_request(
                &model_id, 0, duration, false,
            );

            e.error_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_request_deserialization() {
        let json = r#"{"model": "test-model", "prompt": "Hello", "temperature": 0.7}"#;
        let req: GenerateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.temperature, Some(0.7));
    }

    #[test]
    fn test_generate_request_minimal() {
        let json = r#"{"model": "test-model", "prompt": "Hello"}"#;
        let req: GenerateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.temperature, None);
        assert_eq!(req.top_p, None);
        assert_eq!(req.max_tokens, None);
    }
}
