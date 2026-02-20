use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use std::sync::OnceLock;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::http_client::HTTP_CLIENT;
use crate::models::EmbedderConfig;

const DEFAULT_OPENAI_BATCH_SIZE: usize = 2048;
const DEFAULT_COHERE_BATCH_SIZE: usize = 96;
const DEFAULT_LOCAL_BATCH_SIZE: usize = 128;

// Global semaphore to limit concurrent embedding API requests
static EMBEDDING_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Circuit breaker for the embedding inference API.
/// Prevents cascading failures when the inference service is down or
/// persistently returning errors.
static INFERENCE_CIRCUIT_BREAKER: OnceLock<Arc<CircuitBreaker>> = OnceLock::new();

/// Get the inference circuit breaker (creating a default if not yet initialized).
fn inference_circuit_breaker() -> &'static Arc<CircuitBreaker> {
    INFERENCE_CIRCUIT_BREAKER.get_or_init(|| {
        CircuitBreaker::new(CircuitBreakerConfig::from_env_with_prefix(
            "inference",
            "INFERENCE_CB",
        ))
    })
}

/// Cached embedding inference API URL (set once at startup)
static EMBEDDING_INFERENCE_API_URL: OnceLock<String> = OnceLock::new();

/// Server-reported estimated wait time in milliseconds (from X-Estimated-Wait-Ms header).
/// Workers can read this to proactively pace requests instead of waiting for 503s.
static SERVER_ESTIMATED_WAIT_MS: AtomicU64 = AtomicU64::new(0);

/// Server-reported queue depth (from X-Queue-Depth header).
static SERVER_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);

/// Server-reported queue capacity (from X-Queue-Capacity header).
static SERVER_QUEUE_CAPACITY: AtomicU64 = AtomicU64::new(0);

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

/// Timestamp (epoch secs) when DOWNSTREAM_PRESSURE was last set to true.
/// Used for auto-clearing after a timeout so the system can recover when
/// the embedding service has shed load but no successful response has
/// flowed through yet to clear the flag.
static DOWNSTREAM_PRESSURE_SET_AT: AtomicU64 = AtomicU64::new(0);

/// Auto-clear timeout for DOWNSTREAM_PRESSURE (seconds).
/// If no successful response clears the flag within this window, we
/// optimistically reset it so adaptive concurrency can ramp back up.
/// Seconds before we auto-clear downstream-pressure even without a success.
fn downstream_pressure_timeout_secs() -> u64 {
    static VALUE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("EMBEDDING_DOWNSTREAM_PRESSURE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60)
    })
}

/// Check if the downstream embedding service is signalling overload.
/// Auto-clears after the configured timeout if no success has
/// reset the flag, preventing permanent pressure lock when all requests fail.
pub fn is_downstream_under_pressure() -> bool {
    if !DOWNSTREAM_PRESSURE.load(Ordering::Relaxed) {
        return false;
    }

    let set_at = DOWNSTREAM_PRESSURE_SET_AT.load(Ordering::Relaxed);
    if set_at > 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now.saturating_sub(set_at) >= downstream_pressure_timeout_secs() {
            // Auto-clear: pressure has been set for too long without a success
            tracing::info!(
                timeout_secs = downstream_pressure_timeout_secs(),
                "Auto-clearing downstream pressure flag after timeout"
            );
            DOWNSTREAM_PRESSURE.store(false, Ordering::Relaxed);
            DOWNSTREAM_PRESSURE_SET_AT.store(0, Ordering::Relaxed);
            return false;
        }
    }

    true
}

/// Get the server-reported estimated wait time in milliseconds.
/// Returns 0 when no data is available yet.
pub fn server_estimated_wait_ms() -> u64 {
    SERVER_ESTIMATED_WAIT_MS.load(Ordering::Relaxed)
}

/// Check if the embedding server queue is becoming congested.
/// Returns true when the queue is more than 50% full.
pub fn is_server_queue_congested() -> bool {
    let depth = SERVER_QUEUE_DEPTH.load(Ordering::Relaxed);
    let capacity = SERVER_QUEUE_CAPACITY.load(Ordering::Relaxed);
    capacity > 0 && depth * 2 > capacity
}

/// Adaptive inter-request pacing delay based on server-side queue state.
/// Returns a Duration that callers should wait between batch submissions
/// to maintain smooth throughput without overwhelming the embedder.
///
/// When the server queue is empty, returns Duration::ZERO (no pacing needed).
/// When the queue is filling up, returns a delay proportional to the
/// server-reported EMA latency × queue depth to prevent further buildup.
pub fn adaptive_pacing_delay() -> Duration {
    let wait_ms = SERVER_ESTIMATED_WAIT_MS.load(Ordering::Relaxed);
    let depth = SERVER_QUEUE_DEPTH.load(Ordering::Relaxed);
    let capacity = SERVER_QUEUE_CAPACITY.load(Ordering::Relaxed);

    if capacity == 0 || depth == 0 {
        return Duration::ZERO;
    }

    // Scale delay: when queue is nearly empty, no delay.
    // When >50% full, add proportional delay.
    let fill_ratio = depth as f64 / capacity as f64;
    if fill_ratio < 0.25 {
        Duration::ZERO
    } else {
        // Add a fraction of the estimated wait as pacing delay
        let delay_ms = (wait_ms as f64 * fill_ratio).min(5000.0) as u64;
        Duration::from_millis(delay_ms)
    }
}

/// Update cached server-side backpressure metrics from response headers.
fn update_server_backpressure(resp: &reqwest::Response) {
    if let Some(val) = resp.headers().get("X-Estimated-Wait-Ms")
        && let Ok(ms) = val.to_str().unwrap_or("0").parse::<u64>()
    {
        SERVER_ESTIMATED_WAIT_MS.store(ms, Ordering::Relaxed);
    }
    if let Some(val) = resp.headers().get("X-Queue-Depth")
        && let Ok(d) = val.to_str().unwrap_or("0").parse::<u64>()
    {
        SERVER_QUEUE_DEPTH.store(d, Ordering::Relaxed);
    }
    if let Some(val) = resp.headers().get("X-Queue-Capacity")
        && let Ok(c) = val.to_str().unwrap_or("0").parse::<u64>()
    {
        SERVER_QUEUE_CAPACITY.store(c, Ordering::Relaxed);
    }
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

    // Acquire a job-level semaphore permit BEFORE sending any batches.
    // This ensures one embedding job completes all its batches before another
    // job can start, preventing multiple concurrent jobs from flooding the
    // embedding service queue and causing VRAM exhaustion (ORT BFCArena
    // expansion starving Qwen3/candle of GPU memory).
    let _job_permit = EMBEDDING_SEMAPHORE
        .get()
        .expect("Embedding semaphore not initialized — call init_embedder() from main")
        .acquire()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to acquire embedding semaphore permit: {}", e))?;

    tracing::debug!(
        texts = texts.len(),
        batch_size = effective_batch_size,
        "Acquired embedding job permit, starting batch processing"
    );

    if texts.len() <= effective_batch_size {
        return process_single_batch(config, texts).await;
    }

    let mut all_embeddings = Vec::new();
    for chunk in texts.chunks(effective_batch_size) {
        // Apply adaptive pacing between batches to avoid overwhelming the embedder.
        // The delay is derived from server-reported queue state (zero when idle).
        let pacing = adaptive_pacing_delay();
        if !pacing.is_zero() {
            tracing::debug!(
                pacing_ms = pacing.as_millis(),
                "Pacing between embedding batches"
            );
            sleep(pacing).await;
        }

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

    // Max retries for embedding operations.
    // For 503 (VRAM pressure) we fail fast after 2 attempts — the VRAM
    // situation won't resolve in seconds, and NATS redelivery (with NAK
    // delay) is a better retry granularity for resource exhaustion.
    let max_retries: u32 = 5;
    let max_503_retries: u32 = 2;
    let mut consecutive_503s: u32 = 0;
    let mut last_error = None;
    let mut used_server_retry_delay = false; // Track if we already waited per server's Retry-After

    let circuit = inference_circuit_breaker();

    for attempt in 0..=max_retries {
        // Check circuit breaker before each attempt
        if !circuit.should_allow().await {
            let batch_duration = batch_start.elapsed().as_secs_f64();
            crate::observability::record_embedding_batch(
                model_name,
                batch_duration,
                chunk_count,
                false,
            );
            return Err(anyhow::anyhow!(
                "Inference circuit breaker is open — embedding service unavailable"
            ));
        }
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

        let request = req
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to clone request for retry"))?;

        match request.send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    // Read server-side backpressure headers before consuming body
                    update_server_backpressure(&resp);

                    let response_body: serde_json::Value = resp.json().await?;
                    let result = parse_embeddings_response(config, response_body);

                    // Clear downstream pressure on success
                    DOWNSTREAM_PRESSURE.store(false, Ordering::Relaxed);
                    DOWNSTREAM_PRESSURE_SET_AT.store(0, Ordering::Relaxed);

                    circuit.record_success().await;

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
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    DOWNSTREAM_PRESSURE_SET_AT.store(now, Ordering::Relaxed);

                    let retry_after = resp
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(5);

                    let text = resp.text().await.unwrap_or_default();

                    consecutive_503s += 1;
                    circuit.record_failure().await;
                    tracing::warn!(
                        attempt = attempt,
                        retry_after_secs = retry_after,
                        consecutive_503s = consecutive_503s,
                        max_503_retries = max_503_retries,
                        "Embedding service at capacity (503), backing off"
                    );

                    // Fail fast on persistent VRAM pressure — retrying won't
                    // help, let the job NAK so NATS can redeliver later.
                    if consecutive_503s >= max_503_retries {
                        tracing::warn!(
                            consecutive_503s = consecutive_503s,
                            "Aborting batch: persistent 503 from embedding service"
                        );
                        // Record failure metrics
                        let batch_duration = batch_start.elapsed().as_secs_f64();
                        crate::observability::record_embedding_batch(
                            model_name,
                            batch_duration,
                            chunk_count,
                            false,
                        );
                        return Err(anyhow::anyhow!(
                            "Embedding service persistently at capacity after {} consecutive 503 responses: {}",
                            consecutive_503s,
                            text
                        ));
                    }

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

                circuit.record_failure().await;
                last_error = Some(anyhow::anyhow!("Embedder API error {}: {}", status, text));
            }
            Err(e) => {
                circuit.record_failure().await;
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
