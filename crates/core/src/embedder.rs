use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;

use crate::http_client::HTTP_CLIENT;
use crate::models::EmbedderConfig;

const DEFAULT_OPENAI_BATCH_SIZE: usize = 2048;
const DEFAULT_COHERE_BATCH_SIZE: usize = 96;
const DEFAULT_LOCAL_BATCH_SIZE: usize = 256;

fn get_embedding_inference_api_url() -> String {
    std::env::var("EMBEDDING_INFERENCE_API_URL").unwrap_or_else(|_| "http://localhost:8090".to_string())
}

pub async fn generate_batch_embeddings(
    config: &EmbedderConfig,
    texts: Vec<&str>,
    batch_size: Option<usize>,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let effective_batch_size = if let Some(size) = batch_size {
        size
    } else {
        match config.provider.as_str() {
            "openai" => DEFAULT_OPENAI_BATCH_SIZE,
            "cohere" => DEFAULT_COHERE_BATCH_SIZE,
            "local" => DEFAULT_LOCAL_BATCH_SIZE,
            _ => return Err(anyhow::anyhow!("Unsupported provider: {}", config.provider)),
        }
    };

    if texts.len() <= effective_batch_size {
        return process_single_batch(config, texts).await;
    }

    let mut all_embeddings = Vec::new();
    for chunk in texts.chunks(effective_batch_size) {
        let embeddings = process_single_batch(config, chunk.to_vec()).await?;
        all_embeddings.extend(embeddings);
    }

    Ok(all_embeddings)
}

async fn process_single_batch(config: &EmbedderConfig, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::with_capacity(0));
    }

    let client = &*HTTP_CLIENT;

    let (url, body, needs_bearer_auth) = match config.provider.as_str() {
        "openai" => {
            let model = &config.model;
            let body = serde_json::json!({
                "input": texts,
                "model": model,
            });
            let url = format!("{}/embeddings", config.base_url.trim_end_matches('/'));
            (url, body, true)
        }
        "cohere" => {
            let model = &config.model;
            let input_type = config
                .config
                .get("input_type")
                .and_then(|v| v.as_str())
                .unwrap_or("clustering");

            let embedding_types = config
                .config
                .get("embedding_types")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_else(|| vec!["float"]);

            let truncate = config
                .config
                .get("truncate")
                .and_then(|v| v.as_str())
                .unwrap_or("NONE");

            let body = serde_json::json!({
                "texts": texts,
                "model": model,
                "input_type": input_type,
                "embedding_types": embedding_types,
                "truncate": truncate
            });
            let base = config.base_url.trim_end_matches('/');
            let url = if base.ends_with("/embed") {
                base.to_string()
            } else {
                format!("{}/embed", base)
            };
            (url, body, true)
        }
        "local" => {
            let model = &config.model;
            let body = serde_json::json!({
                "texts": texts,
                "model": model,
            });
            let inference_url = get_embedding_inference_api_url();
            let url = format!("{}/api/embed/batch", inference_url.trim_end_matches('/'));
            (url, body, false)
        }
        _ => return Err(anyhow::anyhow!("Unsupported provider: {}", config.provider)),
    };

    let mut req = client.post(&url).json(&body);

    if needs_bearer_auth {
        if let Some(key) = &config.api_key {
            req = req.bearer_auth(key);
        } else {
            return Err(anyhow::anyhow!(
                "API key required for {} provider",
                config.provider
            ));
        }
    }

    //TODO: make configurable with environment variable
    let max_retries = 3;
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_secs(1 << (attempt - 1));
            tracing::warn!(
                attempt = attempt,
                delay_secs = delay.as_secs(),
                "Retrying embedder request after transient error"
            );
            sleep(delay).await;
        }

        let request = req
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to clone request for retry"))?;

        match request.send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let response_body: serde_json::Value = resp.json().await?;
                    return parse_embeddings_response(config, response_body);
                }

                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();

                if status.is_client_error() {
                    return Err(anyhow::anyhow!("Embedder API error {}: {}", status, text));
                }

                last_error = Some(anyhow::anyhow!("Embedder API error {}: {}", status, text));
            }
            Err(e) => {
                last_error = Some(anyhow::anyhow!("Failed to send request to {}: {}", url, e));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown embedder error")))
}

fn parse_embeddings_response(
    config: &EmbedderConfig,
    response_body: serde_json::Value,
) -> Result<Vec<Vec<f32>>> {
    match config.provider.as_str() {
        "openai" => {
            let data = response_body
                .get("data")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("Invalid OpenAI response"))?;
            let mut embeddings = Vec::new();
            for item in data {
                let embedding = item
                    .get("embedding")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .ok_or_else(|| anyhow::anyhow!("Missing embedding in OpenAI response"))?;
                embeddings.push(embedding);
            }
            Ok(embeddings)
        }
        "cohere" => {
            if let Some(embeddings) = response_body.get("embeddings") {
                if let Some(float_embeddings) = embeddings.get("float") {
                    serde_json::from_value(float_embeddings.clone()).map_err(|e| e.into())
                } else if embeddings.is_array() {
                    serde_json::from_value(embeddings.clone()).map_err(|e| e.into())
                } else {
                    Err(anyhow::anyhow!("Invalid Cohere response format"))
                }
            } else {
                Err(anyhow::anyhow!("Missing embeddings in Cohere response"))
            }
        }
        "local" => response_body
            .get("embeddings")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid local inference response")),
        _ => Err(anyhow::anyhow!("Unsupported provider parsing")),
    }
}
