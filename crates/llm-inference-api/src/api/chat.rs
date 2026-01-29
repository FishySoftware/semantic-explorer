//! Chat completion API endpoints.

use actix_web::{HttpResponse, Responder, ResponseError, post, web};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};
use utoipa::ToSchema;

use crate::config::{GenerationConfig, ModelConfig};
use crate::errors::InferenceError;
use crate::llm;

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatMessage {
    /// Role of the message sender (system, user, assistant)
    pub role: String,
    /// Content of the message
    pub content: String,
}

impl From<ChatMessage> for llm::ChatMessage {
    fn from(msg: ChatMessage) -> Self {
        llm::ChatMessage {
            role: msg.role,
            content: msg.content,
        }
    }
}

impl From<llm::ChatMessage> for ChatMessage {
    fn from(msg: llm::ChatMessage) -> Self {
        ChatMessage {
            role: msg.role,
            content: msg.content,
        }
    }
}

/// Request body for chat completion
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatRequest {
    /// Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
    pub model: String,
    /// Conversation history
    pub messages: Vec<ChatMessage>,
    /// Temperature for sampling (0.0-2.0, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p for nucleus sampling (0.0-1.0, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Maximum number of tokens to generate (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    /// Stop sequences (optional, kept for OpenAI API compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    pub stop: Option<Vec<String>>,
}

/// Response for chat completion
#[derive(Debug, Serialize, ToSchema)]
pub struct ChatResponse {
    /// Generated message
    pub message: ChatMessage,
    /// Model used
    pub model: String,
    /// Number of tokens generated
    pub tokens_generated: usize,
    /// Reason generation stopped (length, stop, eos, error)
    pub finish_reason: String,
}

/// Chat completion endpoint
///
/// Generates a response based on conversation history
#[utoipa::path(
    post,
    path = "/api/chat",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Chat completion generated successfully", body = ChatResponse),
        (status = 400, description = "Invalid request or unsupported model"),
        (status = 500, description = "Internal server error")
    ),
    tag = "chat"
)]
#[post("/api/chat")]
#[instrument(skip(model_config, gen_config), fields(model, messages))]
pub async fn chat_completion(
    model_config: web::Data<ModelConfig>,
    gen_config: web::Data<GenerationConfig>,
    body: web::Json<ChatRequest>,
) -> impl Responder {
    // Backpressure: wait for permit with timeout, return 503 if queue congested
    let _permit = match llm::acquire_permit_with_timeout().await {
        Ok(permit) => permit,
        Err(e) => {
            return e.error_response();
        }
    };

    let model_id = body.model.clone();
    let messages = body.messages.clone();

    tracing::Span::current().record("model", &model_id);
    tracing::Span::current().record("messages", messages.len());

    // Validate messages
    if messages.is_empty() {
        return InferenceError::BadRequest("Messages array cannot be empty".to_string())
            .error_response();
    }

    // Build generation parameters
    let params = llm::GenerationParams {
        temperature: body.temperature.unwrap_or(gen_config.default_temperature),
        top_p: body.top_p.unwrap_or(gen_config.default_top_p),
        max_tokens: body.max_tokens.unwrap_or(gen_config.default_max_tokens),
    };

    let start = std::time::Instant::now();

    // Convert messages
    let llm_messages: Vec<llm::ChatMessage> = messages.into_iter().map(Into::into).collect();

    // Generate chat completion
    let result =
        match llm::chat_completion(&model_id, llm_messages, params, &model_config, &gen_config)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                tracing::error!(
                    model = %model_id,
                    error = %e,
                    duration_secs = duration,
                    "Chat completion failed"
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
        "Chat completion generated successfully"
    );

    semantic_explorer_core::observability::record_llm_request(
        &model_id,
        result.tokens_generated as u64,
        duration,
        true,
    );

    HttpResponse::Ok().json(ChatResponse {
        message: result.message.into(),
        model: result.model,
        tokens_generated: result.tokens_generated,
        finish_reason: result.finish_reason.to_string(),
    })
}

/// Chat completion with streaming response
///
/// Generates a response based on conversation history and streams it as Server-Sent Events
#[utoipa::path(
    post,
    path = "/api/chat/stream",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Streaming chat completion", content_type = "text/event-stream"),
        (status = 400, description = "Invalid request or unsupported model"),
        (status = 500, description = "Internal server error")
    ),
    tag = "chat"
)]
#[post("/api/chat/stream")]
#[instrument(skip(model_config, gen_config), fields(model, messages))]
pub async fn chat_completion_stream(
    model_config: web::Data<ModelConfig>,
    gen_config: web::Data<GenerationConfig>,
    body: web::Json<ChatRequest>,
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
    let messages = body.messages.clone();

    tracing::Span::current().record("model", &model_id);
    tracing::Span::current().record("messages", messages.len());

    // Validate messages
    if messages.is_empty() {
        return InferenceError::BadRequest("Messages array cannot be empty".to_string())
            .error_response();
    }

    // Build generation parameters
    let params = llm::GenerationParams {
        temperature: body.temperature.unwrap_or(gen_config.default_temperature),
        top_p: body.top_p.unwrap_or(gen_config.default_top_p),
        max_tokens: body.max_tokens.unwrap_or(gen_config.default_max_tokens),
    };

    info!(
        model = %model_id,
        "Starting streaming chat completion"
    );

    // Convert messages
    let llm_messages: Vec<llm::ChatMessage> = messages.into_iter().map(Into::into).collect();

    // Generate chat completion stream
    let stream = match llm::chat_completion_stream(
        &model_id,
        llm_messages,
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
                serde_json::json!({"content": chunk})
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
    fn test_chat_message_conversion() {
        let api_msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };

        let llm_msg: llm::ChatMessage = api_msg.clone().into();
        assert_eq!(llm_msg.role, "user");
        assert_eq!(llm_msg.content, "Hello");

        let back_to_api: ChatMessage = llm_msg.into();
        assert_eq!(back_to_api.role, api_msg.role);
        assert_eq!(back_to_api.content, api_msg.content);
    }

    #[test]
    fn test_chat_request_deserialization() {
        let json = r#"{
            "model": "test-model",
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "temperature": 0.7
        }"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.temperature, Some(0.7));
    }

    #[test]
    fn test_chat_request_minimal() {
        let json = r#"{
            "model": "test-model",
            "messages": [{"role": "user", "content": "Hi"}]
        }"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test-model");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.temperature, None);
    }
}
