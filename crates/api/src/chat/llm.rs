use futures_util::Stream;
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::http_client::HTTP_CLIENT;
use sqlx::{Pool, Postgres};
use std::pin::Pin;

use crate::{chat::models::ChatMessage, storage::postgres::chat as chat_storage};

/// Type alias for streaming LLM response results
type LLMStreamResult = Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String>;

/// Default system prompt for RAG chat
/// Uses {{chunks}} placeholder which gets replaced with retrieved document chunks
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful assistant that answers questions based on the provided context.

Context:
{{chunks}}

When answering, always cite the specific chunk number (e.g., 'According to Chunk 1' or 'As mentioned in Chunk 2 and Chunk 3') to reference where your information comes from. If the context doesn't contain relevant information to answer the question, say so explicitly.";

/// Configuration for LLM API requests
#[derive(Debug, Clone)]
struct LLMRequestConfig {
    api_base: String,
    model: String,
    api_key: Option<String>,
    temperature: f32,
    max_tokens: i32,
}

/// Build the effective system prompt from custom prompt or default
/// Replaces {{chunks}} placeholder with the actual context
fn build_system_prompt(custom_prompt: Option<&str>, context: &str) -> String {
    let prompt = match custom_prompt {
        Some(p) if !p.trim().is_empty() => p,
        _ => DEFAULT_SYSTEM_PROMPT,
    };
    // Replace {{chunks}} placeholder with actual context
    prompt.replace("{{chunks}}", context)
}

/// Generate an LLM response with RAG context
#[tracing::instrument(
    name = "generate_llm_response",
    skip(pool, query, context, encryption, system_prompt)
)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn generate_response(
    pool: &Pool<Postgres>,
    encryption: &EncryptionService,
    llm_inference_url: &str,
    llm_id: i32,
    session_id: &str,
    query: &str,
    context: &str,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    system_prompt: Option<&str>,
) -> Result<String, String> {
    let llm_start = std::time::Instant::now();

    // Check for injection attempts in user query
    if let Some(reason) = crate::chat::prompt_injection::detect_injection_attempt(query) {
        tracing::warn!(
            llm_id = llm_id,
            reason = %reason,
            "rejecting request due to injection attempt detection"
        );
        let llm_duration = llm_start.elapsed().as_secs_f64();
        semantic_explorer_core::observability::record_llm_response("unknown", llm_duration, false);
        return Err(format!("query rejected: {}", reason));
    }

    // Fetch LLM details from database
    let (name, provider, base_url, model, api_key) =
        chat_storage::get_llm_details(pool, encryption, llm_id)
            .await
            .map_err(|e| {
                let llm_duration = llm_start.elapsed().as_secs_f64();
                semantic_explorer_core::observability::record_llm_response(
                    "unknown",
                    llm_duration,
                    false,
                );
                format!("database error: {e}")
            })?;

    // Sanitize user input to prevent injection
    let sanitized_query = crate::chat::prompt_injection::sanitize_user_input(query);

    // Build the effective system prompt (with {{chunks}} replaced)
    let effective_system_prompt = build_system_prompt(system_prompt, context);

    // Build the user prompt - chunks are in the system prompt via {{chunks}} placeholder
    let user_prompt = format!(
        "Question: {}\n\nPlease provide a helpful answer based on the context provided.",
        sanitized_query
    );

    let temperature = temperature.unwrap_or(0.7).clamp(0.0, 2.0);
    let max_tokens = max_tokens.unwrap_or(2000).max(1);

    // Call the appropriate LLM API
    let response_text = match provider.to_lowercase().as_str() {
        "internal" => {
            // Get conversation history for internal LLM
            let history = chat_storage::get_chat_messages(pool, session_id)
                .await
                .unwrap_or_default();
            call_internal_llm_api(
                llm_inference_url,
                &model,
                &effective_system_prompt,
                &user_prompt,
                &history,
                temperature,
                max_tokens,
            )
            .await?
        }
        "openai" => {
            call_openai_api(
                &base_url,
                api_key.as_deref(),
                &model,
                &effective_system_prompt,
                &user_prompt,
                temperature,
                max_tokens,
            )
            .await?
        }
        "cohere" => {
            call_cohere_api(
                &base_url,
                api_key.as_deref(),
                &model,
                &user_prompt,
                temperature,
                max_tokens,
            )
            .await?
        }
        _ => {
            let llm_duration = llm_start.elapsed().as_secs_f64();
            semantic_explorer_core::observability::record_llm_response(&model, llm_duration, false);
            return Err(format!("unsupported LLM provider: {}", provider));
        }
    };

    // Validate response for injection indicators
    if crate::chat::prompt_injection::validate_response(&response_text) {
        tracing::warn!(
            llm_id = llm_id,
            response_len = response_text.len(),
            "LLM response contains suspicious patterns that may indicate successful injection"
        );
    }

    let llm_duration = llm_start.elapsed().as_secs_f64();
    semantic_explorer_core::observability::record_llm_response(&model, llm_duration, true);

    tracing::debug!(llm_name = %name, "generated LLM response");
    Ok(response_text)
}

/// Call OpenAI API for chat completion
async fn call_openai_api(
    api_base: &str,
    api_key: Option<&str>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    max_tokens: i32,
) -> Result<String, String> {
    let api_key = api_key.ok_or_else(|| "API key not configured for OpenAI".to_string())?;

    let client = &*HTTP_CLIENT;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
    });

    let response = client
        .post(format!("{}/v1/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {e}"))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error: {}", error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse OpenAI response: {e}"))?;

    let response_text = response_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "unexpected OpenAI response format".to_string())?
        .to_string();

    Ok(response_text)
}

/// Call internal LLM inference API for chat completion
async fn call_internal_llm_api(
    llm_inference_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    history: &[ChatMessage],
    temperature: f32,
    max_tokens: i32,
) -> Result<String, String> {
    // Build messages with conversation history
    let mut messages = vec![crate::llms::client::ChatMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    }];

    // Add conversation history (excluding the current incomplete assistant message)
    for msg in history {
        // Skip incomplete/error messages
        if msg.status != "complete" {
            continue;
        }
        messages.push(crate::llms::client::ChatMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    // Add current user prompt
    messages.push(crate::llms::client::ChatMessage {
        role: "user".to_string(),
        content: user_prompt.to_string(),
    });

    // Use the llm_client to call the internal inference API
    let response = crate::llms::client::chat_completion(
        llm_inference_url,
        model,
        messages,
        Some(temperature),
        None,
        Some(max_tokens as usize),
        None,
    )
    .await
    .map_err(|e| format!("Internal LLM API error: {}", e))?;

    Ok(response.message.content)
}

/// Call internal LLM inference API for streaming chat completion
async fn call_internal_llm_api_stream(
    llm_inference_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    history: &[ChatMessage],
    temperature: f32,
    max_tokens: i32,
) -> LLMStreamResult {
    // Build messages with conversation history
    let mut messages = vec![crate::llms::client::ChatMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    }];

    // Add conversation history (excluding the current incomplete assistant message)
    for msg in history {
        // Skip incomplete/error messages
        if msg.status != "complete" {
            continue;
        }
        messages.push(crate::llms::client::ChatMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    // Add current user prompt
    messages.push(crate::llms::client::ChatMessage {
        role: "user".to_string(),
        content: user_prompt.to_string(),
    });

    // Use the llm_client to call the local inference API streaming endpoint
    let stream = crate::llms::client::chat_completion_stream(
        llm_inference_url,
        model,
        messages,
        Some(temperature),
        Some(max_tokens as usize),
    )
    .await
    .map_err(|e| format!("Local LLM API streaming error: {}", e))?;

    // Convert anyhow::Result to Result<String, String>
    let converted_stream = async_stream::stream! {
        use futures_util::StreamExt;
        let mut stream = stream;
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => yield Ok(chunk),
                Err(e) => yield Err(e.to_string()),
            }
        }
    };

    Ok(Box::pin(converted_stream))
}

/// Call Cohere API for text generation
async fn call_cohere_api(
    api_base: &str,
    api_key: Option<&str>,
    model: &str,
    prompt: &str,
    temperature: f32,
    max_tokens: i32,
) -> Result<String, String> {
    let api_key = api_key.ok_or_else(|| "API key not configured for Cohere".to_string())?;

    let client = &*HTTP_CLIENT;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let response = client
        .post(format!("{}/chat", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Cohere API request failed: {e}"))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Cohere API error: {}", error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse Cohere response: {e}"))?;

    // Extract text from Cohere response
    // Cohere format: message.content[0].text
    let response_text = response_json
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.get(0))
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            format!(
                "unexpected Cohere response format. Expected message.content[0].text. Response: {}",
                serde_json::to_string_pretty(&response_json)
                    .unwrap_or_else(|_| "unknown".to_string())
            )
        })?
        .to_string();

    Ok(response_text)
}

/// Generate a streaming LLM response with RAG context
#[tracing::instrument(
    name = "generate_llm_response_stream",
    skip(pool, query, context, encryption, system_prompt)
)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn generate_response_stream(
    pool: &Pool<Postgres>,
    encryption: &EncryptionService,
    llm_inference_url: &str,
    llm_id: i32,
    session_id: &str,
    query: &str,
    context: &str,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    system_prompt: Option<&str>,
) -> LLMStreamResult {
    // Fetch LLM details from database
    let (name, provider, base_url, model, api_key) =
        chat_storage::get_llm_details(pool, encryption, llm_id)
            .await
            .map_err(|e| format!("database error: {e}"))?;

    // Build the effective system prompt (with {{chunks}} replaced)
    let effective_system_prompt = build_system_prompt(system_prompt, context);

    // Build the user prompt - chunks are in the system prompt via {{chunks}} placeholder
    let user_prompt = format!(
        "Question: {}\n\nPlease provide a helpful answer based on the context provided.",
        query
    );

    let temperature = temperature.unwrap_or(0.7).clamp(0.0, 2.0);
    let max_tokens = max_tokens.unwrap_or(2000).max(1);

    tracing::debug!(llm_name = %name, provider = %provider, "starting streaming LLM response");

    // Handle internal provider separately (before consuming model in config)
    if provider.to_lowercase() == "internal" {
        // Get conversation history for internal LLM
        let history = chat_storage::get_chat_messages(pool, session_id)
            .await
            .unwrap_or_default();
        // Use internal LLM inference API streaming
        let stream = call_internal_llm_api_stream(
            llm_inference_url,
            &model,
            &effective_system_prompt,
            &user_prompt,
            &history,
            temperature,
            max_tokens,
        )
        .await?;
        return Ok(stream);
    }

    // Build config for the LLM request (for other providers)
    let config = LLMRequestConfig {
        api_base: base_url,
        model,
        api_key,
        temperature,
        max_tokens,
    };

    let response = match provider.to_lowercase().as_str() {
        "openai" => {
            make_streaming_request(&config, &effective_system_prompt, &user_prompt, "openai")
                .await?
        }
        "cohere" => {
            make_streaming_request(&config, &effective_system_prompt, &user_prompt, "cohere")
                .await?
        }
        _ => return Err(format!("unsupported LLM provider: {}", provider)),
    };

    let provider_clone = provider.clone();
    let stream = async_stream::stream! {
        let mut response = response;
        let mut buffer = String::new();

        while let Some(chunk_result) = response.chunk().await.transpose() {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(format!("stream error: {}", e));
                    return;
                }
            };

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer.drain(..=newline_pos);
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        return;
                    }

                    match serde_json::from_str::<serde_json::Value>(data) {
                        Ok(json) => {
                            if let Some(content) = extract_content_from_provider(&json, &provider_clone) {
                                yield Ok(content);
                            } else {
                                tracing::debug!("No content extracted from LLM stream JSON");
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, data = %data, "Failed to parse LLM stream JSON");
                        }
                    }
                }
            }
        }
    };

    Ok(Box::pin(stream))
}

/// Make a streaming request to an LLM provider
async fn make_streaming_request(
    config: &LLMRequestConfig,
    system_prompt: &str,
    user_prompt: &str,
    provider: &str,
) -> Result<reqwest::Response, String> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| format!("API key not configured for {}", provider))?;
    let client = &*HTTP_CLIENT;

    let response = match provider {
        "openai" => {
            let request_body = serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": config.temperature,
                "max_tokens": config.max_tokens,
                "stream": true,
            });

            client
                .post(format!("{}/v1/chat/completions", config.api_base))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request_body)
                .send()
                .await
                .map_err(|e| format!("OpenAI API request failed: {e}"))?
        }
        "cohere" => {
            let request_body = serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "user", "content": user_prompt}
                ],
                "max_tokens": config.max_tokens,
                "temperature": config.temperature,
                "stream": true,
            });

            client
                .post(format!("{}/chat", config.api_base))
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Accept", "text/event-stream")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| format!("Cohere API request failed: {e}"))?
        }
        _ => return Err(format!("unsupported provider: {}", provider)),
    };

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("{} API error: {}", provider, error_text));
    }

    Ok(response)
}

/// Extract content from provider-specific JSON response
fn extract_content_from_provider(json: &serde_json::Value, provider: &str) -> Option<String> {
    match provider {
        "openai" => json
            .get("choices")?
            .get(0)?
            .get("delta")?
            .get("content")?
            .as_str()
            .map(String::from),
        "cohere" => {
            // Cohere V2 Chat API streaming format
            if json.get("type")?.as_str()? == "content-delta" {
                json.get("delta")?
                    .get("message")?
                    .get("content")?
                    .get("text")?
                    .as_str()
                    .map(String::from)
            } else {
                None
            }
        }
        _ => None,
    }
}
