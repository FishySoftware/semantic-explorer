//! LLM Inference API client.
//!
//! Provides functions to interact with the local llm-inference-api service.

use anyhow::{Context, Result};
use futures_util::Stream;
use semantic_explorer_core::http_client::HTTP_CLIENT;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender (system, user, assistant)
    pub role: String,
    /// Content of the message
    pub content: String,
}

/// Request body for chat completion
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    /// Model to use
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
    /// Stop sequences (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Response for chat completion
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    /// Generated message
    pub message: ChatMessage,
    /// Model used
    #[allow(dead_code)]
    pub model: String,
    /// Number of tokens generated
    #[allow(dead_code)]
    pub tokens_generated: usize,
    /// Reason generation stopped (length, stop, eos, error)
    #[allow(dead_code)]
    pub finish_reason: String,
}

/// Generate a chat completion
///
/// # Arguments
/// * `llm_inference_url` - Base URL for the LLM inference API
/// * `model` - Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
/// * `messages` - Conversation history
/// * `temperature` - Optional temperature for sampling (0.0-2.0)
/// * `top_p` - Optional top-p for nucleus sampling (0.0-1.0)
/// * `max_tokens` - Optional maximum number of tokens to generate
/// * `stop` - Optional stop sequences
#[tracing::instrument(name = "llm_client_chat", skip(messages))]
pub async fn chat_completion(
    llm_inference_url: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<usize>,
    stop: Option<Vec<String>>,
) -> Result<ChatResponse> {
    let url = format!("{}/api/chat", llm_inference_url.trim_end_matches('/'));

    let request = ChatRequest {
        model: model.to_string(),
        messages,
        temperature,
        top_p,
        max_tokens,
        stop,
    };

    let response = HTTP_CLIENT
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Failed to send request to LLM inference API")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("LLM inference API returned {}: {}", status, error_text);
    }

    let result = response
        .json::<ChatResponse>()
        .await
        .context("Failed to parse response from LLM inference API")?;

    Ok(result)
}

/// Generate a streaming chat response
///
/// Returns a stream of text chunks as they are generated.
///
/// # Arguments
/// * `llm_inference_url` - Base URL for the LLM inference API
/// * `model` - Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
/// * `messages` - Conversation history
/// * `temperature` - Optional temperature for sampling (0.0-2.0)
/// * `max_tokens` - Optional maximum number of tokens to generate
#[tracing::instrument(name = "llm_client_chat_stream", skip(messages))]
pub async fn chat_completion_stream(
    llm_inference_url: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<usize>,
) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
    let url = format!(
        "{}/api/chat/stream",
        llm_inference_url.trim_end_matches('/')
    );

    let request = ChatRequest {
        model: model.to_string(),
        messages,
        temperature,
        top_p: None,
        max_tokens,
        stop: None,
    };

    let response = HTTP_CLIENT
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Failed to send streaming request to LLM inference API")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("LLM inference API returned {}: {}", status, error_text);
    }

    // Create a stream from the response body using the same pattern as other streaming code
    let stream = async_stream::stream! {
        let mut response = response;
        let mut buffer = String::new();

        while let Some(chunk_result) = response.chunk().await.transpose() {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(anyhow::anyhow!("stream error: {}", e));
                    return;
                }
            };

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines (SSE format)
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer.drain(..=newline_pos);

                // Skip empty lines
                if line.is_empty() {
                    continue;
                }

                // Parse SSE data lines
                if let Some(data) = line.strip_prefix("data: ") {
                    // Parse JSON data
                    match serde_json::from_str::<serde_json::Value>(data) {
                        Ok(json) => {
                            // Extract content from {"content": "..."}
                            if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                                yield Ok(content.to_string());
                            } else if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                                yield Err(anyhow::anyhow!("Stream error: {}", error));
                                return;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse SSE JSON: {} - data: {}", e, data);
                        }
                    }
                }
            }
        }
    };

    Ok(Box::pin(stream))
}
