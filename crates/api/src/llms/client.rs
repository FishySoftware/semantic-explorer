//! LLM Inference API client.
//!
//! Provides functions to interact with the local llm-inference-api service.

use anyhow::{Context, Result};
use semantic_explorer_core::http_client::HTTP_CLIENT;
use serde::{Deserialize, Serialize};

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

/// Get the LLM inference API URL from config
fn get_llm_inference_api_url() -> String {
    std::env::var("LLM_INFERENCE_API_URL").unwrap_or_else(|_| "http://localhost:8091".to_string())
}

/// Generate a chat completion
///
/// # Arguments
/// * `model` - Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
/// * `messages` - Conversation history
/// * `temperature` - Optional temperature for sampling (0.0-2.0)
/// * `top_p` - Optional top-p for nucleus sampling (0.0-1.0)
/// * `max_tokens` - Optional maximum number of tokens to generate
/// * `stop` - Optional stop sequences
#[tracing::instrument(name = "llm_client_chat", skip(messages))]
pub async fn chat_completion(
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<usize>,
    stop: Option<Vec<String>>,
) -> Result<ChatResponse> {
    let url = format!("{}/api/chat", get_llm_inference_api_url());

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

/// Generate a simple chat-style response from a system prompt and user message
///
/// This is a convenience wrapper around chat_completion for simple use cases.
///
/// # Arguments
/// * `model` - Model to use (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
/// * `system_prompt` - System prompt that sets the assistant's behavior
/// * `user_message` - User's message
/// * `temperature` - Optional temperature for sampling (0.0-2.0)
/// * `max_tokens` - Optional maximum number of tokens to generate
#[tracing::instrument(name = "llm_client_simple_chat", skip(system_prompt, user_message))]
pub async fn simple_chat(
    model: &str,
    system_prompt: &str,
    user_message: &str,
    temperature: Option<f32>,
    max_tokens: Option<usize>,
) -> Result<String> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
        },
    ];

    let response = chat_completion(
        model,
        messages,
        temperature,
        None, // top_p
        max_tokens,
        None, // stop
    )
    .await?;

    Ok(response.message.content)
}
