use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use std::sync::OnceLock;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::http_client::HTTP_CLIENT;
use crate::models::EmbedderConfig;

const DEFAULT_OPENAI_BATCH_SIZE: usize = 2048;
const DEFAULT_COHERE_BATCH_SIZE: usize = 96;
const DEFAULT_LOCAL_BATCH_SIZE: usize = 256;

// Global semaphore to limit concurrent embedding API requests
static EMBEDDING_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Cached embedding inference API URL (set once at startup)
static EMBEDDING_INFERENCE_API_URL: OnceLock<String> = OnceLock::new();

/// Initialize the embedding client configuration.
///
/// Must be called from main before any embedding requests.
/// - `api_url`: URL of the local embedding inference API
/// - `max_concurrent_requests`: maximum concurrent embedding API requests
pub fn init_embedder(api_url: &str, max_concurrent_requests: usize) {
    EMBEDDING_INFERENCE_API_URL.get_or_init(|| api_url.to_string());
    EMBEDDING_SEMAPHORE.get_or_init(|| {
        tracing::info!(
            max_concurrent = max_concurrent_requests,
            "Initialized embedding request semaphore for rate limiting"
        );
        Arc::new(Semaphore::new(max_concurrent_requests))
    });
}

/// Global flag indicating the embedding/inference downstream is under pressure.
/// Set to `true` on 503 responses, cleared on successful requests.
/// Workers can poll this to throttle NATS consumption.
static DOWNSTREAM_PRESSURE: AtomicBool = AtomicBool::new(false);

/// Check if the downstream embedding service is signalling overload.
pub fn is_downstream_under_pressure() -> bool {
    DOWNSTREAM_PRESSURE.load(Ordering::Relaxed)
}

fn get_embedding_inference_api_url() -> &'static str {
    EMBEDDING_INFERENCE_API_URL
        .get()
        .map(|s| s.as_str())
        .unwrap_or("http://localhost:8090")
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
            "internal" => DEFAULT_LOCAL_BATCH_SIZE,
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

    let batch_start = Instant::now();
    let chunk_count = texts.len();
    let model_name = &config.model;

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
        "internal" => {
            let model = &config.model;
            let body = serde_json::json!({
                "texts": texts,
                "model": model,
            });
            let inference_url = get_embedding_inference_api_url();
            let url = format!("{}/api/embed/batch", inference_url.trim_end_matches('/'));
            tracing::debug!(
                provider = "internal",
                model = %model,
                endpoint = %url,
                text_count = texts.len(),
                "Generating batch embeddings via internal inference API"
            );
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

    // Max retries for embedding operations (hardcoded — no env var needed)
    let max_retries: u32 = 5;
    let mut last_error = None;
    let mut used_server_retry_delay = false; // Track if we already waited per server's Retry-After

    for attempt in 0..=max_retries {
        // Apply exponential backoff only if we didn't already use server's Retry-After delay
        if attempt > 0 && !used_server_retry_delay {
            let delay = Duration::from_secs(1 << (attempt - 1).min(4)); // Cap at 16 seconds
            tracing::warn!(
                attempt = attempt,
                delay_secs = delay.as_secs(),
                "Retrying embedder request after transient error"
            );
            sleep(delay).await;
        }
        used_server_retry_delay = false; // Reset for this attempt

        // Acquire permit from global semaphore to limit concurrent embedding requests
        let _permit = EMBEDDING_SEMAPHORE
            .get()
            .expect("Embedding semaphore not initialized — call init_embedder() from main")
            .acquire()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to acquire embedding semaphore permit: {}", e))?;

        tracing::debug!(texts = texts.len(), "Acquired embedding request permit");

        let request = req
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to clone request for retry"))?;

        match request.send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let response_body: serde_json::Value = resp.json().await?;
                    let result = parse_embeddings_response(config, response_body);

                    // Clear downstream pressure on success
                    DOWNSTREAM_PRESSURE.store(false, Ordering::Relaxed);

                    // Aggregate metrics: record once per batch with total duration
                    let batch_duration = batch_start.elapsed().as_secs_f64();
                    crate::observability::record_embedding_batch(
                        model_name,
                        batch_duration,
                        chunk_count,
                        result.is_ok(),
                    );

                    return result;
                }

                let status = resp.status();

                // Don't retry 4xx client errors - these are non-transient failures
                if status.is_client_error() {
                    let text = resp.text().await.unwrap_or_default();
                    tracing::error!(
                        status = %status,
                        error = %text,
                        "Embedder API client error (non-retriable)"
                    );
                    return Err(anyhow::anyhow!("Embedder API error {}: {}", status, text));
                }

                // Handle 503 Service Unavailable with Retry-After header
                if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
                    // Signal downstream pressure to upstream workers
                    DOWNSTREAM_PRESSURE.store(true, Ordering::Relaxed);

                    let retry_after = resp
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(5);

                    let text = resp.text().await.unwrap_or_default();
                    tracing::warn!(
                        attempt = attempt,
                        retry_after_secs = retry_after,
                        "Embedding service at capacity (503), backing off"
                    );

                    // Use the server-suggested retry delay (skip exponential backoff on next iteration)
                    if attempt < max_retries {
                        sleep(Duration::from_secs(retry_after)).await;
                        used_server_retry_delay = true; // Skip exponential backoff on next attempt
                    }

                    last_error = Some(anyhow::anyhow!(
                        "Embedding service at capacity (503): {}",
                        text
                    ));
                    continue;
                }

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

    // Record failure metrics for this batch (aggregated)
    let batch_duration = batch_start.elapsed().as_secs_f64();
    crate::observability::record_embedding_batch(model_name, batch_duration, chunk_count, false);

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
        "internal" => response_body
            .get("embeddings")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid internal inference response")),
        _ => Err(anyhow::anyhow!("Unsupported provider parsing")),
    }
}
