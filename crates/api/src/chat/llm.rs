use sqlx::{Pool, Postgres};

use crate::storage::postgres::chat as chat_storage;

const SYSTEM_PROMPT: &str = "You are a helpful assistant that answers questions based on the provided context. Always cite the source of your information when possible.";

/// Generate an LLM response with RAG context
#[tracing::instrument(name = "generate_llm_response", skip(postgres_pool, query, context))]
pub(crate) async fn generate_response(
    postgres_pool: &Pool<Postgres>,
    llm_id: i32,
    query: &str,
    context: &str,
) -> Result<String, String> {
    // Fetch LLM details from database
    let (name, provider, base_url, model, api_key) =
        chat_storage::get_llm_details(postgres_pool, llm_id)
            .await
            .map_err(|e| format!("database error: {e}"))?;

    // Build the prompt with RAG context
    let user_prompt = format!(
        "{}\n\nQuestion: {}\n\nPlease provide a helpful answer based on the context above.",
        context, query
    );

    // Call the appropriate LLM API
    let response_text = match provider.to_lowercase().as_str() {
        "openai" => {
            call_openai_api(
                &base_url,
                api_key.as_deref(),
                &model,
                SYSTEM_PROMPT,
                &user_prompt,
            )
            .await?
        }
        "anthropic" => {
            call_anthropic_api(
                &base_url,
                api_key.as_deref(),
                &model,
                SYSTEM_PROMPT,
                &user_prompt,
            )
            .await?
        }
        "cohere" => call_cohere_api(&base_url, api_key.as_deref(), &model, &user_prompt).await?,
        _ => {
            return Err(format!("unsupported LLM provider: {}", provider));
        }
    };

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
) -> Result<String, String> {
    let api_key = api_key.ok_or_else(|| "API key not configured for OpenAI".to_string())?;

    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.7,
        "max_tokens": 2000,
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

/// Call Anthropic API for chat completion
async fn call_anthropic_api(
    api_base: &str,
    api_key: Option<&str>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let api_key = api_key.ok_or_else(|| "API key not configured for Anthropic".to_string())?;

    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": user_prompt}
        ],
        "system": system_prompt,
        "max_tokens": 2000,
    });

    let response = client
        .post(format!("{}/messages", api_base))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Anthropic API request failed: {e}"))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error: {}", error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse Anthropic response: {e}"))?;

    let response_text = response_json
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| "unexpected Anthropic response format".to_string())?
        .to_string();

    Ok(response_text)
}

/// Call Cohere API for text generation
async fn call_cohere_api(
    api_base: &str,
    api_key: Option<&str>,
    model: &str,
    prompt: &str,
) -> Result<String, String> {
    let api_key = api_key.ok_or_else(|| "API key not configured for Cohere".to_string())?;

    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 2000,
        "temperature": 0.7,
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
