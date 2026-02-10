use candle_core::{DType, Device};
use fastembed::{EmbeddingModel, Qwen3TextEmbedding, TextEmbedding, TextInitOptions};
use futures::stream::StreamExt;
use once_cell::sync::OnceCell;
use ort::ep::ArenaExtendStrategy;
use ort::ep::CUDA;
use ort::ep::cuda::AttentionBackend;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::available_parallelism;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
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
// Cached embedder enum — dispatches to ONNX or Qwen3 at inference time
// ---------------------------------------------------------------------------

enum CachedEmbedder {
    /// ONNX-based model via fastembed's `TextEmbedding`
    Onnx(Box<Mutex<TextEmbedding>>),
    /// Candle-based Qwen3 model via fastembed's `Qwen3TextEmbedding`
    Qwen3(Box<Mutex<Qwen3TextEmbedding>>),
}

struct CachedModel {
    embedder: CachedEmbedder,
}

type EmbeddingCache = Arc<RwLock<HashMap<String, Arc<CachedModel>>>>;

/// Global embedding model cache
static EMBEDDING_MODELS: OnceCell<EmbeddingCache> = OnceCell::new();

/// Global concurrency semaphore for backpressure
static EMBEDDING_SEMAPHORE: OnceCell<Arc<Semaphore>> = OnceCell::new();

/// Queue timeout for acquiring semaphore permits
static SEMAPHORE_QUEUE_TIMEOUT: OnceCell<Duration> = OnceCell::new();

/// Cached GPU pressure state (updated by background monitor)
static GPU_PRESSURE_HIGH: AtomicBool = AtomicBool::new(false);

/// GPU pressure threshold — configured via GPU_PRESSURE_THRESHOLD env var (default 95.0)
static GPU_PRESSURE_THRESHOLD: OnceLock<f64> = OnceLock::new();

/// Initialize the global concurrency semaphore for embedding requests
pub fn init_semaphore(max_concurrent: usize, queue_timeout_ms: u64) {
    EMBEDDING_SEMAPHORE.get_or_init(|| {
        info!(
            max_concurrent = max_concurrent,
            queue_timeout_ms = queue_timeout_ms,
            "Initialized global embedding semaphore"
        );
        Arc::new(Semaphore::new(max_concurrent))
    });
    SEMAPHORE_QUEUE_TIMEOUT.get_or_init(|| Duration::from_millis(queue_timeout_ms));
}

/// Get available permits (for health checks)
pub fn available_permits() -> usize {
    EMBEDDING_SEMAPHORE
        .get()
        .map(|s| s.available_permits())
        .unwrap_or(0)
}

/// Spawn background task that monitors GPU pressure and updates cached state
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
            "Starting embedding GPU pressure monitor"
        );

        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;

            // Collect metrics (updates Prometheus gauges)
            gpu_monitor::collect_metrics();

            // Update cached pressure state (VRAM OR compute)
            let is_high = gpu_monitor::is_gpu_under_pressure(threshold);
            GPU_PRESSURE_HIGH.store(is_high, Ordering::Relaxed);

            if is_high {
                warn!(
                    "Embedding: GPU pressure is HIGH (>{}% VRAM or compute)",
                    threshold
                );
            }
        }
    });
}

/// Check cached GPU pressure (fast, no NVML call)
#[inline]
pub fn is_gpu_pressure_high() -> bool {
    GPU_PRESSURE_HIGH.load(Ordering::Relaxed)
}

pub(crate) fn get_all_available_embedding_models(config: &ModelConfig) -> Vec<AvailableModel> {
    // ONNX-based models from fastembed
    let onnx_models = TextEmbedding::list_supported_models()
        .into_iter()
        .filter(|m| {
            m.model_file.eq("onnx/model.onnx") && // Remove optimized and quantized variants (not GPU friendly)
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

/// Initialize the embedding model cache and pre-load allowed models
pub async fn init_cache(config: &ModelConfig) {
    let cache = EMBEDDING_MODELS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())));

    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        info!("No embedding models to pre-load");
        return;
    }

    info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        "Pre-loading embedding models"
    );

    let concurrency_limit = available_parallelism().map(|n| n.get()).unwrap_or(1);

    info!("Using concurrency limit: {}", concurrency_limit);

    // Parallel loading — dispatch to ONNX or Qwen3 based on model_code
    let results = futures::stream::iter(models_to_load)
        .map(|model_id| {
            let config = config.clone();
            async move {
                let model_id_clone = model_id.clone();
                let res = tokio::task::spawn_blocking(move || {
                    if is_qwen3_model(&model_id_clone) {
                        let id_for_err = model_id_clone.clone();
                        create_qwen3_embedding(&model_id_clone, &config)
                            .map(|qwen| {
                                (
                                    model_id_clone,
                                    CachedEmbedder::Qwen3(Box::new(Mutex::new(qwen))),
                                )
                            })
                            .map_err(|e| (id_for_err, e))
                    } else {
                        let id_for_err = model_id_clone.clone();
                        resolve_onnx_embedding_model(&model_id_clone)
                            .and_then(|emb| create_text_embedding(emb, &config))
                            .map(|te| {
                                (
                                    model_id_clone,
                                    CachedEmbedder::Onnx(Box::new(Mutex::new(te))),
                                )
                            })
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

    let mut cache_guard = cache.write().await;

    for result in results {
        match result {
            Ok((model_id, embedder)) => {
                let backend = match &embedder {
                    CachedEmbedder::Onnx(_) => "onnx",
                    CachedEmbedder::Qwen3(_) => "qwen3/candle",
                };
                let entry = Arc::new(CachedModel { embedder });
                cache_guard.insert(model_id.clone(), entry);

                info!(model_id = %model_id, backend = backend, "Pre-loaded embedding model");
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
        loaded_models = cache_guard.len(),
        "Embedding model cache initialization complete"
    );
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

/// Generate embeddings using the model cache
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

    let models = EMBEDDING_MODELS
        .get()
        .ok_or_else(|| InferenceError::Internal("Embedding cache not initialized".to_string()))?;

    // Get the cached model
    let entry = {
        let cache = models.read().await;
        cache.get(model_id).cloned().ok_or_else(|| {
            warn!(model_id = %model_id, "Model not found in preloaded cache");
            InferenceError::UnsupportedModel(format!(
                "Model {} not preloaded. Please check configuration.",
                model_id
            ))
        })?
    };

    // Check GPU pressure before accepting work
    let threshold = GPU_PRESSURE_THRESHOLD.get().copied().unwrap_or(95.0);
    if is_gpu_pressure_high() {
        warn!(
            model_id = %model_id,
            "GPU pressure high (>{}% VRAM or compute), rejecting request",
            threshold
        );
        return Err(InferenceError::ServiceUnavailable(
            "GPU pressure high, try again later".to_string(),
        ));
    }

    // Acquire global semaphore permit with timeout for backpressure
    let _permit = if let Some(semaphore) = EMBEDDING_SEMAPHORE.get() {
        let timeout = SEMAPHORE_QUEUE_TIMEOUT
            .get()
            .copied()
            .unwrap_or(Duration::from_millis(5000));

        let permit = match tokio::time::timeout(timeout, semaphore.clone().acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(InferenceError::Internal("Semaphore closed".to_string()));
            }
            Err(_) => {
                warn!(
                    model_id = %model_id,
                    available_permits = semaphore.available_permits(),
                    "Embedding queue congested, returning 503"
                );
                return Err(InferenceError::ServiceUnavailable(format!(
                    "Model {} queue congested, try again later",
                    model_id
                )));
            }
        };
        Some(permit)
    } else {
        None
    };

    // Generate embeddings in a blocking task
    let texts_count = texts.len();
    let total_chars: usize = texts.iter().map(|t| t.len()).sum();
    let avg_chars = if texts_count > 0 {
        total_chars / texts_count
    } else {
        0
    };
    let batch_size = Some(config.max_batch_size);
    let model_id_owned = model_id.to_string();

    tokio::task::spawn_blocking(move || {
        let lock_start = Instant::now();

        match &entry.embedder {
            CachedEmbedder::Onnx(model_mutex) => {
                let mut text_embedding = model_mutex.lock().map_err(|e| {
                    InferenceError::Internal(format!("Failed to acquire ONNX model lock: {}", e))
                })?;
                let lock_time = lock_start.elapsed();
                let embed_start = Instant::now();
                let res = text_embedding.embed(texts, batch_size).map_err(|e| {
                    error!(error = %e, "ONNX embedding generation failed");
                    InferenceError::Embedding(e.to_string())
                });
                let embed_time = embed_start.elapsed();
                debug!(
                    model_id = %model_id_owned,
                    backend = "onnx",
                    texts_count = texts_count,
                    total_chars = total_chars,
                    avg_chars_per_text = avg_chars,
                    lock_time_ms = lock_time.as_millis(),
                    embed_time_ms = embed_time.as_millis(),
                    per_text_ms = embed_time.as_millis() as f64 / texts_count as f64,
                    chars_per_sec = total_chars as f64 / embed_time.as_secs_f64(),
                    "Embedding timing"
                );
                res
            }
            CachedEmbedder::Qwen3(model_mutex) => {
                let model = model_mutex.lock().map_err(|e| {
                    InferenceError::Internal(format!("Failed to acquire Qwen3 model lock: {}", e))
                })?;
                let lock_time = lock_start.elapsed();
                let embed_start = Instant::now();
                let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                let res = model.embed(&text_refs).map_err(|e| {
                    error!(error = %e, "Qwen3 embedding generation failed");
                    InferenceError::Embedding(e.to_string())
                });
                let embed_time = embed_start.elapsed();
                debug!(
                    model_id = %model_id_owned,
                    backend = "qwen3/candle",
                    texts_count = texts_count,
                    total_chars = total_chars,
                    avg_chars_per_text = avg_chars,
                    lock_time_ms = lock_time.as_millis(),
                    embed_time_ms = embed_time.as_millis(),
                    per_text_ms = embed_time.as_millis() as f64 / texts_count as f64,
                    chars_per_sec = total_chars as f64 / embed_time.as_secs_f64(),
                    "Embedding timing"
                );
                res
            }
        }
    })
    .await
    .map_err(|e| InferenceError::Internal(format!("Blocking task join error: {}", e)))?
}

/// Check if models are loaded and ready
pub fn is_ready() -> bool {
    EMBEDDING_MODELS.get().is_some()
}
