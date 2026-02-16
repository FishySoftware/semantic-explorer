use candle_core::{DType, Device};
use fastembed::{EmbeddingModel, Qwen3TextEmbedding, TextEmbedding, TextInitOptions};
use futures::stream::StreamExt;
use once_cell::sync::OnceCell;
use ort::ep::ArenaExtendStrategy;
use ort::ep::CUDA;
use ort::ep::cuda::AttentionBackend;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread::available_parallelism;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, oneshot};
use tracing::{debug, error, info, warn};

use std::sync::OnceLock;

use crate::config::ModelConfig;
use crate::errors::InferenceError;

use semantic_explorer_core::observability::gpu_monitor;

/// Known Qwen3 embedding model definitions.
/// These use the candle backend (not ONNX) and are loaded via `Qwen3TextEmbedding`.
struct Qwen3ModelDef {
    model_code: &'static str,
    description: &'static str,
    dim: usize,
    /// Weight loading precision — F32 is required because the embed() attention
    /// mask is built as F32 and candle does not auto-broadcast dtypes in matmul.
    dtype: DType,
    /// Maximum token length supported by the model.
    max_length: usize,
}

const QWEN3_MODELS: &[Qwen3ModelDef] = &[
    Qwen3ModelDef {
        model_code: "Qwen/Qwen3-Embedding-0.6B",
        description: "Qwen3 0.6B parameter embedding model (candle backend)",
        dim: 1024,
        dtype: DType::F32,
        max_length: 32768,
    },
    Qwen3ModelDef {
        model_code: "Qwen/Qwen3-Embedding-4B",
        description: "Qwen3 4B parameter embedding model (candle backend)",
        dim: 3584,
        dtype: DType::F32,
        max_length: 32768,
    },
    Qwen3ModelDef {
        model_code: "Qwen/Qwen3-Embedding-8B",
        description: "Qwen3 8B parameter embedding model (candle backend)",
        dim: 4096,
        dtype: DType::F32,
        max_length: 32768,
    },
];

/// Check whether a model code refers to a Qwen3 embedding model.
pub(crate) fn is_qwen3_model(model_code: &str) -> bool {
    QWEN3_MODELS.iter().any(|d| d.model_code == model_code)
}

// ---------------------------------------------------------------------------
// Unified model info type
// ---------------------------------------------------------------------------

/// Backend-agnostic model information exposed to callers.
#[derive(Debug, Clone)]
pub(crate) struct AvailableModel {
    pub model_code: String,
    pub description: String,
    pub dim: usize,
}

// ---------------------------------------------------------------------------
// Channel-based model queue
// ---------------------------------------------------------------------------
//
// Each loaded model gets a dedicated worker thread and a bounded async channel.
// Requests are submitted to the channel and the worker processes them one at a
// time — no Mutex contention, no latency snowball, completely deterministic
// queue depth.  The bounded channel provides natural backpressure: when full,
// senders get a clear signal to back off.

/// A request sent through the model channel.
struct EmbedRequest {
    texts: Vec<String>,
    batch_size: Option<usize>,
    reply: oneshot::Sender<Result<Vec<Vec<f32>>, InferenceError>>,
    enqueued_at: Instant,
}

/// Handle to a model's dedicated processing queue.
struct ModelHandle {
    sender: tokio::sync::mpsc::Sender<EmbedRequest>,
    /// Current queue depth (updated atomically by sender/worker).
    queue_depth: Arc<AtomicUsize>,
}

type ModelRegistry = Arc<RwLock<HashMap<String, Arc<ModelHandle>>>>;

/// Global model registry — maps model_code → ModelHandle
static MODEL_REGISTRY: OnceCell<ModelRegistry> = OnceCell::new();

/// Cached GPU VRAM pressure state (updated by background monitor)
static GPU_PRESSURE_HIGH: AtomicBool = AtomicBool::new(false);

/// GPU pressure threshold — configured via GPU_PRESSURE_THRESHOLD env var (default 95.0)
static GPU_PRESSURE_THRESHOLD: OnceLock<f64> = OnceLock::new();

/// Exponential moving average of per-request latency in microseconds.
/// Used to populate the `X-Estimated-Wait-Ms` backpressure header so callers
/// can pace themselves proactively instead of waiting for 503s.
static EMA_LATENCY_US: AtomicU64 = AtomicU64::new(0);

/// Queue capacity (max pending requests per model)
static QUEUE_CAPACITY: OnceLock<usize> = OnceLock::new();

/// Queue timeout for waiting to enqueue
static QUEUE_TIMEOUT: OnceLock<Duration> = OnceLock::new();

/// Initialize queue configuration.
pub fn init_queue_config(max_queue_depth: usize, queue_timeout_ms: u64) {
    QUEUE_CAPACITY.get_or_init(|| {
        let cap = max_queue_depth.max(1);
        info!(
            max_queue_depth = cap,
            queue_timeout_ms = queue_timeout_ms,
            "Initialized embedding model queue configuration"
        );
        cap
    });
    QUEUE_TIMEOUT.get_or_init(|| Duration::from_millis(queue_timeout_ms));
}

// ---------------------------------------------------------------------------
// Backpressure metrics exposed to API handlers
// ---------------------------------------------------------------------------

/// Information about the current queue state for backpressure headers.
pub struct QueueStatus {
    /// Number of requests waiting in the queue for this model.
    pub queue_depth: usize,
    /// Maximum queue capacity.
    pub queue_capacity: usize,
    /// Estimated wait time in milliseconds based on EMA latency × queue depth.
    pub estimated_wait_ms: u64,
}

/// Get the current queue status for a model (for response headers).
pub fn get_queue_status(model_id: &str) -> Option<QueueStatus> {
    let registry = MODEL_REGISTRY.get()?;
    // Try fast sync read; if contended, return None (non-critical)
    let guard = registry.try_read().ok()?;
    let handle = guard.get(model_id)?;

    let depth = handle.queue_depth.load(Ordering::Relaxed);
    let capacity = QUEUE_CAPACITY.get().copied().unwrap_or(8);
    let ema_us = EMA_LATENCY_US.load(Ordering::Relaxed);
    let estimated_wait_ms = (depth as u64).saturating_mul(ema_us) / 1000;

    Some(QueueStatus {
        queue_depth: depth,
        queue_capacity: capacity,
        estimated_wait_ms,
    })
}

/// Get total queue depth across all models (for health endpoint).
pub fn total_queue_depth() -> usize {
    MODEL_REGISTRY
        .get()
        .and_then(|r| r.try_read().ok())
        .map(|guard| {
            guard
                .values()
                .map(|h| h.queue_depth.load(Ordering::Relaxed))
                .sum()
        })
        .unwrap_or(0)
}

/// Spawn background task that monitors GPU VRAM pressure and updates cached state.
///
/// Only VRAM utilization is checked.
pub fn spawn_gpu_pressure_monitor(threshold: f64) {
    GPU_PRESSURE_THRESHOLD.get_or_init(|| threshold);
    tokio::spawn(async move {
        if !gpu_monitor::init() {
            warn!("GPU monitoring disabled - NVML not available");
            return;
        }

        info!(
            device_count = gpu_monitor::device_count(),
            threshold = threshold,
            "Starting GPU VRAM pressure monitor"
        );

        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        // Track consecutive high-pressure ticks with no in-flight work so we
        // can distinguish "arena holds idle memory" from "genuinely overloaded".
        let mut consecutive_high_idle_ticks: u32 = 0;

        loop {
            ticker.tick().await;

            // Collect metrics (updates Prometheus gauges)
            gpu_monitor::collect_metrics();

            // Only check VRAM pressure — compute utilization being high is expected
            let is_high = gpu_monitor::is_memory_pressure_high(threshold);
            GPU_PRESSURE_HIGH.store(is_high, Ordering::Relaxed);

            if is_high {
                let total_queued = total_queue_depth();
                if total_queued == 0 {
                    consecutive_high_idle_ticks += 1;
                } else {
                    consecutive_high_idle_ticks = 0;
                }

                // After 30 seconds of high VRAM with no work, it's almost
                // certainly static arena allocation — log at reduced severity
                // to avoid alarm fatigue.
                if consecutive_high_idle_ticks >= 6 {
                    debug!(
                        consecutive_idle_secs = consecutive_high_idle_ticks * 5,
                        "GPU VRAM high but model idle — likely static CUDA arena allocation"
                    );
                } else {
                    warn!("GPU VRAM pressure HIGH (>{}% used)", threshold);
                }
            } else {
                consecutive_high_idle_ticks = 0;
            }
        }
    });
}

/// Check cached GPU VRAM pressure (fast, no NVML call)
#[inline]
pub fn is_gpu_pressure_high() -> bool {
    GPU_PRESSURE_HIGH.load(Ordering::Relaxed)
}

pub(crate) fn get_all_available_embedding_models(config: &ModelConfig) -> Vec<AvailableModel> {
    // ONNX-based models from fastembed
    let onnx_models = TextEmbedding::list_supported_models()
        .into_iter()
        .filter(|m| {
            (m.model_file.eq("onnx/model.onnx") || m.model_file.eq("model.onnx")) && // Remove optimized and quantized variants (not GPU friendly)
            !m.model_code.contains("-zh-") && // Remove Chinese-specific models
            !m.model_code.eq("onnx-community/embeddinggemma-300m-ONNX") // Remove Gemma (not GPU friendly)
        })
        .filter(|m| config.is_embedding_model_allowed(&m.model_code))
        .map(|m| AvailableModel {
            model_code: m.model_code,
            description: m.description,
            dim: m.dim,
        });

    // Qwen3 candle-based models
    let qwen3_models = QWEN3_MODELS
        .iter()
        .filter(|d| config.is_embedding_model_allowed(d.model_code))
        .map(|d| AvailableModel {
            model_code: d.model_code.to_string(),
            description: d.description.to_string(),
            dim: d.dim,
        });

    onnx_models.chain(qwen3_models).collect()
}

/// Get the list of embedding models to load based on configuration
fn get_models_to_load(config: &ModelConfig) -> Vec<String> {
    if config.all_embedding_models {
        get_all_available_embedding_models(config)
            .into_iter()
            .map(|m| m.model_code)
            .collect()
    } else {
        config.allowed_embedding_models.clone()
    }
}

/// Resolve a model code string to a fastembed EmbeddingModel enum (ONNX models only)
fn resolve_onnx_embedding_model(model_code: &str) -> Result<EmbeddingModel, InferenceError> {
    TextEmbedding::list_supported_models()
        .into_iter()
        .find(|m| m.model_code == model_code)
        .map(|m| m.model)
        .ok_or_else(|| {
            InferenceError::UnsupportedModel(format!("Unknown ONNX model: {}", model_code))
        })
}

/// Embedder enum — dispatches to ONNX or Qwen3 at inference time.
/// Owned by the dedicated worker thread (no Mutex needed).
enum Embedder {
    Onnx(Box<TextEmbedding>),
    Qwen3(Box<Qwen3TextEmbedding>),
}

/// Initialize the model registry and pre-load allowed models.
///
/// For each loaded model a bounded channel and dedicated worker thread are
/// spawned.  The worker drains the channel sequentially — one batch at a time —
/// which eliminates Mutex contention and provides predictable, spike-free
/// latency.
pub async fn init_cache(config: &ModelConfig) {
    let queue_cap = QUEUE_CAPACITY.get().copied().unwrap_or(8);
    let registry = MODEL_REGISTRY.get_or_init(|| Arc::new(RwLock::new(HashMap::new())));

    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        info!("No embedding models to pre-load");
        return;
    }

    info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        queue_capacity = queue_cap,
        "Pre-loading embedding models with dedicated worker threads"
    );

    let concurrency_limit = available_parallelism().map(|n| n.get()).unwrap_or(1);
    info!("Using concurrency limit: {}", concurrency_limit);

    // Load models in parallel (IO/download bound — safe to parallelise)
    let results = futures::stream::iter(models_to_load)
        .map(|model_id| {
            let config = config.clone();
            async move {
                let model_id_clone = model_id.clone();
                let res = tokio::task::spawn_blocking(move || {
                    if is_qwen3_model(&model_id_clone) {
                        let id_for_err = model_id_clone.clone();
                        create_qwen3_embedding(&model_id_clone, &config)
                            .map(|qwen| (model_id_clone, Embedder::Qwen3(Box::new(qwen))))
                            .map_err(|e| (id_for_err, e))
                    } else {
                        let id_for_err = model_id_clone.clone();
                        resolve_onnx_embedding_model(&model_id_clone)
                            .and_then(|emb| create_text_embedding(emb, &config))
                            .map(|te| (model_id_clone, Embedder::Onnx(Box::new(te))))
                            .map_err(|e| (id_for_err, e))
                    }
                })
                .await;

                match res {
                    Ok(inner_res) => inner_res,
                    Err(join_err) => {
                        Err((model_id, InferenceError::ModelLoad(join_err.to_string())))
                    }
                }
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

    let mut reg_guard = registry.write().await;

    for result in results {
        match result {
            Ok((model_id, embedder)) => {
                let backend = match &embedder {
                    Embedder::Onnx(_) => "onnx",
                    Embedder::Qwen3(_) => "qwen3/candle",
                };

                // Create bounded channel + spawn worker
                let (tx, rx) = tokio::sync::mpsc::channel::<EmbedRequest>(queue_cap);
                let queue_depth = Arc::new(AtomicUsize::new(0));
                let qd = Arc::clone(&queue_depth);
                let mid = model_id.clone();

                // The worker thread owns the embedder — no Mutex needed
                std::thread::Builder::new()
                    .name(format!("embed-{}", model_id))
                    .spawn(move || {
                        run_model_worker(mid, embedder, rx, qd);
                    })
                    .expect("Failed to spawn model worker thread");

                let handle = Arc::new(ModelHandle {
                    sender: tx,
                    queue_depth,
                });
                reg_guard.insert(model_id.clone(), handle);

                info!(model_id = %model_id, backend = backend, "Pre-loaded embedding model with dedicated worker");
            }
            Err((model_id, e)) => {
                error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to load embedding model during initialization"
                );
            }
        }
    }

    info!(
        loaded_models = reg_guard.len(),
        "Embedding model registry initialization complete"
    );
}

// ---------------------------------------------------------------------------
// Dedicated model worker (runs on its own OS thread)
// ---------------------------------------------------------------------------

/// Blocking worker loop that owns the embedder and processes requests
/// sequentially.  Runs on a dedicated OS thread (not in the Tokio pool) so
/// it cannot starve async tasks.
fn run_model_worker(
    model_id: String,
    mut embedder: Embedder,
    rx: tokio::sync::mpsc::Receiver<EmbedRequest>,
    queue_depth: Arc<AtomicUsize>,
) {
    // Build a single-threaded Tokio runtime just for draining the async channel
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build worker runtime");

    rt.block_on(async move {
        let mut rx = rx;

        while let Some(req) = rx.recv().await {
            queue_depth.fetch_sub(1, Ordering::Relaxed);

            let queue_wait = req.enqueued_at.elapsed();
            let texts_count = req.texts.len();
            let total_chars: usize = req.texts.iter().map(|t| t.len()).sum();
            let avg_chars = if texts_count > 0 { total_chars / texts_count } else { 0 };

            let embed_start = Instant::now();

            let result = match &mut embedder {
                Embedder::Onnx(te) => {
                    te.embed(req.texts, req.batch_size).map_err(|e| {
                        error!(error = %e, "ONNX embedding generation failed");
                        InferenceError::Embedding(e.to_string())
                    })
                }
                Embedder::Qwen3(qwen) => {
                    let text_refs: Vec<&str> = req.texts.iter().map(|s| s.as_str()).collect();
                    qwen.embed(&text_refs).map_err(|e| {
                        error!(error = %e, "Qwen3 embedding generation failed");
                        InferenceError::Embedding(e.to_string())
                    })
                }
            };

            let embed_time = embed_start.elapsed();

            // Update EMA latency
            let latency_us = embed_time.as_micros() as u64;
            let prev = EMA_LATENCY_US.load(Ordering::Relaxed);
            let new_ema = if prev == 0 {
                latency_us
            } else {
                // Integer EMA: new = old * 0.7 + sample * 0.3
                (prev * 7 + latency_us * 3) / 10
            };
            EMA_LATENCY_US.store(new_ema, Ordering::Relaxed);

            debug!(
                model_id = %model_id,
                backend = match &embedder { Embedder::Onnx(_) => "onnx", Embedder::Qwen3(_) => "qwen3/candle" },
                texts_count = texts_count,
                total_chars = total_chars,
                avg_chars_per_text = avg_chars,
                queue_wait_ms = queue_wait.as_millis(),
                embed_time_ms = embed_time.as_millis(),
                per_text_ms = embed_time.as_millis() as f64 / texts_count.max(1) as f64,
                chars_per_sec = total_chars as f64 / embed_time.as_secs_f64(),
                remaining_queue = queue_depth.load(Ordering::Relaxed),
                "Embedding timing"
            );

            // Send result back; ignore error if caller timed out
            let _ = req.reply.send(result);
        }

        info!(model_id = %model_id, "Model worker shutting down (channel closed)");
    });
}

/// Create a TextEmbedding instance with proper configuration
fn create_text_embedding(
    model: EmbeddingModel,
    config: &ModelConfig,
) -> Result<TextEmbedding, InferenceError> {
    let mut cuda = CUDA::default()
        .with_prefer_nhwc(true)
        .with_attention_backend(AttentionBackend::CUDNN_FLASH_ATTENTION);

    // Apply CUDA arena size limit if configured, otherwise uses all available GPU memory
    if let Some(arena_size) = config.cuda_arena_size {
        info!(
            cuda_arena_size_bytes = arena_size,
            cuda_arena_size_mb = arena_size / (1024 * 1024),
            "Setting CUDA memory arena limit"
        );
        cuda = cuda.with_memory_limit(arena_size);
    }

    // Apply arena extend strategy
    let strategy = match config.cuda_arena_extend_strategy {
        crate::config::CudaArenaExtendStrategy::SameAsRequested => {
            ArenaExtendStrategy::SameAsRequested
        }
        crate::config::CudaArenaExtendStrategy::NextPowerOfTwo => {
            ArenaExtendStrategy::NextPowerOfTwo
        }
    };
    cuda = cuda.with_arena_extend_strategy(strategy);

    let cuda_provider = cuda.build().error_on_failure();
    let mut options = TextInitOptions::new(model)
        .with_execution_providers(vec![cuda_provider])
        .with_show_download_progress(true);

    if let Some(ref hf_home) = config.hf_home {
        options = options.with_cache_dir(hf_home.clone());
    }

    TextEmbedding::try_new(options).map_err(|e| {
        error!(
            error = %e,
            "Failed to initialize embedding model with CUDA. \
            This may indicate a CUDA/cuDNN version mismatch or driver issue. \
            Check: nvidia-smi, nvcc --version, and cuDNN installation."
        );
        InferenceError::ModelLoad(e.to_string())
    })
}

/// Create a Qwen3TextEmbedding instance using the candle backend
fn create_qwen3_embedding(
    model_code: &str,
    _config: &ModelConfig,
) -> Result<Qwen3TextEmbedding, InferenceError> {
    let def = QWEN3_MODELS
        .iter()
        .find(|d| d.model_code == model_code)
        .ok_or_else(|| {
            InferenceError::UnsupportedModel(format!("Unknown Qwen3 model: {}", model_code))
        })?;

    let device = Device::new_cuda(0).unwrap_or_else(|e| {
        warn!(error = %e, "CUDA device unavailable for Qwen3, falling back to CPU");
        Device::Cpu
    });

    info!(
        model_code = %model_code,
        dtype = ?def.dtype,
        max_length = def.max_length,
        device = ?device,
        "Loading Qwen3 embedding model via candle backend"
    );

    Qwen3TextEmbedding::from_hf(model_code, &device, def.dtype, def.max_length).map_err(|e| {
        error!(
            model_code = %model_code,
            error = %e,
            "Failed to load Qwen3 embedding model"
        );
        InferenceError::ModelLoad(e.to_string())
    })
}

/// Generate embeddings by submitting to the model's dedicated worker queue.
///
/// Returns backpressure metadata alongside the result so API handlers can
/// populate response headers for caller-side adaptive throttling.
pub async fn generate_embeddings(
    model_id: &str,
    config: &ModelConfig,
    texts: Vec<String>,
) -> Result<Vec<Vec<f32>>, InferenceError> {
    // Check if model is allowed
    if !config.is_embedding_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    let registry = MODEL_REGISTRY
        .get()
        .ok_or_else(|| InferenceError::Internal("Model registry not initialized".to_string()))?;

    // Get the model handle
    let handle = {
        let reg = registry.read().await;
        reg.get(model_id).cloned().ok_or_else(|| {
            warn!(model_id = %model_id, "Model not found in registry");
            InferenceError::UnsupportedModel(format!(
                "Model {} not preloaded. Please check configuration.",
                model_id
            ))
        })?
    };

    // Check GPU VRAM pressure before accepting work.
    // Trickle-through: when the model's queue is empty we allow one request
    // through even under VRAM pressure.  The ONNX arena allocates GPU memory
    // and never releases it, so NVML reports high VRAM even when the model is
    // idle.  Blocking **all** requests creates a deadlock where no work can
    // complete and the arena never gets to reuse its already-allocated memory.
    let threshold = GPU_PRESSURE_THRESHOLD.get().copied().unwrap_or(98.0);
    if is_gpu_pressure_high() {
        let current_depth = handle.queue_depth.load(Ordering::Relaxed);
        if current_depth > 0 {
            // Queue already has work — reject to prevent piling on.
            warn!(
                model_id = %model_id,
                queue_depth = current_depth,
                "GPU VRAM pressure high (>{}%) and queue non-empty, rejecting request",
                threshold
            );
            return Err(InferenceError::ServiceUnavailable(
                "GPU VRAM pressure high, try again later".to_string(),
            ));
        }
        // Queue is empty — let this one request trickle through so progress
        // can be made (the worker processes sequentially, one at a time).
        info!(
            model_id = %model_id,
            "GPU VRAM pressure high but queue idle — allowing trickle-through request"
        );
    }

    // Build the request
    // Use gpu_batch_size (not max_batch_size) for the ONNX inference sub-batch.
    // max_batch_size controls API input validation (how many texts per request).
    // gpu_batch_size controls how many texts ORT processes per session.run() call,
    // limiting peak VRAM usage for intermediate tensors (MatMul, attention, etc.).
    let (tx, rx) = oneshot::channel();
    let req = EmbedRequest {
        texts,
        batch_size: Some(config.gpu_batch_size),
        reply: tx,
        enqueued_at: Instant::now(),
    };

    // Try to enqueue with timeout — bounded channel provides natural backpressure
    let timeout = QUEUE_TIMEOUT
        .get()
        .copied()
        .unwrap_or(Duration::from_millis(30000));

    // Increment queue depth before sending
    handle.queue_depth.fetch_add(1, Ordering::Relaxed);

    match tokio::time::timeout(timeout, handle.sender.send(req)).await {
        Ok(Ok(())) => {
            // Request enqueued, wait for result
        }
        Ok(Err(_)) => {
            handle.queue_depth.fetch_sub(1, Ordering::Relaxed);
            return Err(InferenceError::Internal(
                "Model worker channel closed unexpectedly".to_string(),
            ));
        }
        Err(_) => {
            handle.queue_depth.fetch_sub(1, Ordering::Relaxed);
            let depth = handle.queue_depth.load(Ordering::Relaxed);
            warn!(
                model_id = %model_id,
                queue_depth = depth,
                "Embedding queue full, returning 503"
            );
            return Err(InferenceError::ServiceUnavailable(format!(
                "Model {} queue full (depth {}), try again later",
                model_id, depth
            )));
        }
    }

    // Wait for the worker to process and return the result
    rx.await.map_err(|_| {
        InferenceError::Internal("Model worker dropped the response channel".to_string())
    })?
}

/// Check if models are loaded and ready
pub fn is_ready() -> bool {
    MODEL_REGISTRY.get().is_some()
}
