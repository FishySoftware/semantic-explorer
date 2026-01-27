//! Text and code completion API endpoints.
//!
//! Provides completion-style endpoints for code and text completion use-cases,
//! with support for optional suffix (fill-in-middle).

use actix_web::{HttpResponse, Responder, ResponseError, post, web};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};
use utoipa::ToSchema;

use crate::config::{GenerationConfig, ModelConfig};
use crate::llm;

/// Request body for text/code completion
#[derive(Debug, Deserialize, ToSchema)]
pub struct CompletionRequest {
    /// Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
    pub model: String,
    /// Input prompt (text before the completion point)
    pub prompt: String,
    /// Optional suffix (text after the completion point, for fill-in-middle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
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

/// Response for text/code completion
#[derive(Debug, Serialize, ToSchema)]
pub struct CompletionResponse {
    /// Generated completion text
    pub text: String,
    /// Model used
    pub model: String,
    /// Number of tokens generated
    pub tokens_generated: usize,
    /// Reason generation stopped (length, stop, eos, error)
    pub finish_reason: String,
}

/// Text/code completion endpoint (blocking)
///
/// Generates a completion based on prompt and optional suffix (fill-in-middle).
/// Use this endpoint for code completion, text continuation, or fill-in-middle tasks.
#[utoipa::path(
    post,
    path = "/api/completions",
    request_body = CompletionRequest,
    responses(
        (status = 200, description = "Completion generated successfully", body = CompletionResponse),
        (status = 400, description = "Invalid request or unsupported model"),
        (status = 503, description = "Service at capacity"),
        (status = 500, description = "Internal server error")
    ),
    tag = "completions"
)]
#[post("/api/completions")]
#[instrument(skip(model_config, gen_config), fields(model))]
pub async fn completion(
    model_config: web::Data<ModelConfig>,
    gen_config: web::Data<GenerationConfig>,
    body: web::Json<CompletionRequest>,
) -> impl Responder {
    // Backpressure: wait for permit with timeout, return 503 if queue congested
    let _permit = match llm::acquire_permit_with_timeout().await {
        Ok(permit) => permit,
        Err(e) => {
            return e.error_response();
        }
    };

    let model_id = body.model.clone();
    let prompt = body.prompt.clone();
    let suffix = body.suffix.clone();

    tracing::Span::current().record("model", &model_id);

    // Build generation parameters
    let params = llm::GenerationParams {
        temperature: body.temperature.unwrap_or(gen_config.default_temperature),
        top_p: body.top_p.unwrap_or(gen_config.default_top_p),
        max_tokens: body.max_tokens.unwrap_or(gen_config.default_max_tokens),
        stop_sequences: body.stop.clone().unwrap_or_default(),
    };

    let start = std::time::Instant::now();

    // Generate completion
    let result = match llm::text_completion(
        &model_id,
        prompt,
        suffix,
        params,
        &model_config,
        &gen_config,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            tracing::error!(
                model = %model_id,
                error = %e,
                duration_secs = duration,
                "Text completion failed"
            );

            semantic_explorer_core::observability::record_llm_request(
                &model_id, 0, duration, false,
            );

            return e.error_response();
        }
    };

    let duration = start.elapsed().as_secs_f64();

    info!(
        model = %model_id,
        tokens = result.tokens_generated,
        duration_secs = duration,
        "Text completion generated successfully"
    );

    semantic_explorer_core::observability::record_llm_request(
        &model_id,
        result.tokens_generated as u64,
        duration,
        true,
    );

    HttpResponse::Ok().json(CompletionResponse {
        text: result.text,
        model: result.model,
        tokens_generated: result.tokens_generated,
        finish_reason: result.finish_reason.to_string(),
    })
}

/// Text/code completion with streaming response
///
/// Generates a completion and streams it as Server-Sent Events.
/// Use this for real-time code completion or text continuation with streaming output.
#[utoipa::path(
    post,
    path = "/api/completions/stream",
    request_body = CompletionRequest,
    responses(
        (status = 200, description = "Streaming completion", content_type = "text/event-stream"),
        (status = 400, description = "Invalid request or unsupported model"),
        (status = 503, description = "Service at capacity"),
        (status = 500, description = "Internal server error")
    ),
    tag = "completions"
)]
#[post("/api/completions/stream")]
#[instrument(skip(model_config, gen_config), fields(model))]
pub async fn completion_stream(
    model_config: web::Data<ModelConfig>,
    gen_config: web::Data<GenerationConfig>,
    body: web::Json<CompletionRequest>,
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
    let suffix = body.suffix.clone();

    tracing::Span::current().record("model", &model_id);

    // Build generation parameters
    let params = llm::GenerationParams {
        temperature: body.temperature.unwrap_or(gen_config.default_temperature),
        top_p: body.top_p.unwrap_or(gen_config.default_top_p),
        max_tokens: body.max_tokens.unwrap_or(gen_config.default_max_tokens),
        stop_sequences: body.stop.clone().unwrap_or_default(),
    };

    info!(
        model = %model_id,
        has_suffix = suffix.is_some(),
        "Starting streaming text completion"
    );

    // Generate completion stream
    let stream = match llm::text_completion_stream(
        &model_id,
        prompt,
        suffix,
        params,
        &model_config,
        &gen_config,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => return e.error_response(),
    };

    // Convert to SSE format
    let sse_stream = stream.map(|result| match result {
        Ok(chunk) => {
            // Format as Server-Sent Events with JSON data
            Ok::<_, actix_web::Error>(web::Bytes::from(format!(
                "data: {}\n\n",
                serde_json::json!({"text": chunk})
            )))
        }
        Err(e) => {
            // Send error as SSE event
            Ok(web::Bytes::from(format!(
                "data: {{\"error\": \"{}\"}}\n\n",
                e
            )))
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no")) // Disable nginx buffering
        .streaming(sse_stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request_deserialization() {
        let json = r#"{"model": "test-model", "prompt": "def hello():", "temperature": 0.2}"#;
        let req: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "def hello():");
        assert_eq!(req.temperature, Some(0.2));
        assert_eq!(req.suffix, None);
    }

    #[test]
    fn test_completion_request_with_suffix() {
        let json = r#"{
            "model": "test-model",
            "prompt": "def hello():\n    ",
            "suffix": "\n    return result",
            "max_tokens": 50
        }"#;
        let req: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "def hello():\n    ");
        assert_eq!(req.suffix, Some("\n    return result".to_string()));
        assert_eq!(req.max_tokens, Some(50));
    }

    #[test]
    fn test_completion_request_minimal() {
        let json = r#"{"model": "test-model", "prompt": "Hello"}"#;
        let req: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.temperature, None);
        assert_eq!(req.suffix, None);
    }
}
