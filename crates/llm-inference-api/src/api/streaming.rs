//! Streaming text generation API endpoints.

use actix_web::{HttpResponse, Responder, ResponseError, post, web};
use futures::StreamExt;
use serde::Deserialize;
use tracing::{info, instrument, warn};
use utoipa::ToSchema;

use crate::config::{GenerationConfig, ModelConfig};
use crate::llm;

/// Request body for streaming text generation
/// Reuses the same structure as non-streaming generation
#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateStreamRequest {
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

/// Generate text with streaming response
///
/// Returns Server-Sent Events (SSE) stream of text chunks
#[utoipa::path(
    post,
    path = "/api/generate/stream",
    request_body = GenerateStreamRequest,
    responses(
        (status = 200, description = "Streaming text generation", content_type = "text/event-stream"),
        (status = 400, description = "Invalid request or unsupported model"),
        (status = 500, description = "Internal server error")
    ),
    tag = "generation"
)]
#[post("/api/generate/stream")]
#[instrument(skip(model_config, gen_config), fields(model))]
pub async fn generate_stream(
    model_config: web::Data<ModelConfig>,
    gen_config: web::Data<GenerationConfig>,
    body: web::Json<GenerateStreamRequest>,
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

    info!(
        model = %model_id,
        "Starting streaming text generation"
    );

    // Generate text stream
    let stream = match llm::generate_text_stream(
        &model_id,
        prompt,
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
            // Format as Server-Sent Events
            Ok::<_, actix_web::Error>(web::Bytes::from(format!("data: {}\n\n", chunk)))
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
    fn test_generate_stream_request_deserialization() {
        let json = r#"{"model": "test-model", "prompt": "Hello", "temperature": 0.7}"#;
        let req: GenerateStreamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.temperature, Some(0.7));
    }

    #[test]
    fn test_generate_stream_request_minimal() {
        let json = r#"{"model": "test-model", "prompt": "Hello"}"#;
        let req: GenerateStreamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.temperature, None);
    }
}
