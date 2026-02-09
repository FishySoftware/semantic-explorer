use fastembed::{RerankInitOptions, RerankerModel, TextRerank};
use futures::stream::StreamExt;
use once_cell::sync::OnceCell;
use ort::ep::ArenaExtendStrategy;
use ort::ep::CUDA;
use ort::ep::cuda::AttentionBackend;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info};

use crate::config::ModelConfig;
use crate::errors::InferenceError;

/// Type alias for the reranker model cache to reduce complexity
type RerankerCache = Arc<Mutex<HashMap<String, Arc<TokioMutex<TextRerank>>>>>;

/// Global reranker model cache - using per-model mutexes for concurrent access
/// The outer Mutex protects the HashMap structure, while each model has its own Tokio Mutex
static RERANKER_MODELS: OnceCell<RerankerCache> = OnceCell::new();

/// Initialize the reranker model cache and pre-load allowed models
///
/// This function:
/// 1. Takes the ModelConfig to determine which models to load
/// 2. Gets a list of models to load (all supported or filtered by allowed_rerank_models)
/// 3. Loads each model from the filesystem cache, fetching if needed
/// 4. Pre-populates the cache at startup to validate model availability
pub async fn init_cache(config: &ModelConfig) {
    let cache = RERANKER_MODELS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Get list of models to load
    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        tracing::info!("No reranker models to pre-load");
        return;
    }

    tracing::info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        "Pre-loading reranker models at startup with concurrency limit"
    );

    let concurrency_limit = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    tracing::info!("Using concurrency limit: {}", concurrency_limit);

    // Parallel loading
    let results = futures::stream::iter(models_to_load)
        .map(|model_id| {
            let config = config.clone();
            async move {
                let model_id_clone = model_id.clone();
                let res = tokio::task::spawn_blocking(move || {
                    match resolve_reranker_model(&model_id_clone) {
                        Ok(reranker_model) => match create_text_rerank(reranker_model, &config) {
                            Ok(text_rerank) => Ok((model_id_clone, text_rerank)),
                            Err(e) => Err((model_id_clone, e)),
                        },
                        Err(e) => Err((model_id_clone, e)),
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

    let mut cache_guard = match cache.lock() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::error!(error = %e, "Failed to acquire reranker cache lock during initialization");
            return;
        }
    };

    for result in results {
        match result {
            Ok((model_id, text_rerank)) => {
                cache_guard.insert(model_id.clone(), Arc::new(TokioMutex::new(text_rerank)));
                tracing::info!(model_id = %model_id, "Pre-loaded reranker model");
            }
            Err((model_id, e)) => {
                tracing::error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to load reranker model during initialization"
                );
            }
        }
    }
}

/// Get the list of reranker models to load based on configuration
fn get_models_to_load(config: &ModelConfig) -> Vec<String> {
    if config.all_rerank_models {
        // Load all supported reranker models
        get_all_supported_reranker_models()
    } else {
        // Use specific allowed models list (may be empty = no rerankers)
        config.allowed_rerank_models.clone()
    }
}

/// Get all supported reranker model IDs
fn get_all_supported_reranker_models() -> Vec<String> {
    vec![
        "BAAI/bge-reranker-base".to_string(),
        "BAAI/bge-reranker-v2-m3".to_string(),
        "jinaai/jina-reranker-v1-turbo-en".to_string(),
        "jinaai/jina-reranker-v2-base-multilingual".to_string(),
    ]
}

/// Resolve a model ID string to a fastembed RerankerModel enum
fn resolve_reranker_model(model_id: &str) -> Result<RerankerModel, InferenceError> {
    let model = match model_id {
        "BAAI/bge-reranker-base" => RerankerModel::BGERerankerBase,
        "BAAI/bge-reranker-v2-m3" => RerankerModel::BGERerankerV2M3,
        "jinaai/jina-reranker-v1-turbo-en" => RerankerModel::JINARerankerV1TurboEn,
        "jinaai/jina-reranker-v2-base-multilingual" => RerankerModel::JINARerankerV2BaseMultiligual,
        _ => {
            return Err(InferenceError::UnsupportedModel(format!(
                "Unsupported reranker model: {}",
                model_id
            )));
        }
    };

    Ok(model)
}

/// Create a TextRerank instance with proper configuration
///
/// IMPORTANT: ONNX Runtime's CUDA execution provider can silently fall back to CPU
/// if CUDA initialization fails. This function now logs detailed information.
fn create_text_rerank(
    model: RerankerModel,
    config: &ModelConfig,
) -> Result<TextRerank, InferenceError> {
    let mut cuda = CUDA::default()
        .with_prefer_nhwc(true)
        .with_attention_backend(AttentionBackend::CUDNN_FLASH_ATTENTION);

    // Apply CUDA arena size limit if configured, otherwise uses all available GPU memory
    if let Some(arena_size) = config.cuda_arena_size {
        info!(
            cuda_arena_size_bytes = arena_size,
            cuda_arena_size_mb = arena_size / (1024 * 1024),
            "Setting CUDA memory arena limit for reranker"
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
    let mut options = RerankInitOptions::new(model)
        .with_execution_providers(vec![cuda_provider])
        .with_show_download_progress(true);

    // Set cache directory if HF_HOME is configured
    if let Some(ref hf_home) = config.hf_home {
        options = options.with_cache_dir(hf_home.clone());
    }

    let text_rerank = TextRerank::try_new(options).map_err(|e| {
        error!(
            error = %e,
            "Failed to initialize reranker model with CUDA. \
                This may indicate a CUDA/cuDNN version mismatch."
        );
        InferenceError::ModelLoad(e.to_string())
    })?;

    Ok(text_rerank)
}

/// Rerank documents using the model cache
pub async fn rerank_documents(
    model_id: &str,
    config: &ModelConfig,
    query: &str,
    texts: &[&str],
    top_k: Option<usize>,
) -> Result<Vec<fastembed::RerankResult>, InferenceError> {
    // Check if model is allowed
    if !config.is_rerank_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    let models = RERANKER_MODELS
        .get()
        .ok_or_else(|| InferenceError::Internal("Reranker cache not initialized".to_string()))?;

    // Get or create the model with minimal lock time on the HashMap
    let model_mutex = {
        let mut cache = models
            .lock()
            .map_err(|e| InferenceError::Internal(format!("Failed to acquire lock: {}", e)))?;

        // Load model if not in cache
        if !cache.contains_key(model_id) {
            info!(model_id = %model_id, "Loading reranker model on demand");
            let reranker_model = resolve_reranker_model(model_id)?;
            let text_rerank = create_text_rerank(reranker_model, config)?;
            cache.insert(model_id.to_string(), Arc::new(TokioMutex::new(text_rerank)));
        }

        Arc::clone(
            cache
                .get(model_id)
                .ok_or_else(|| InferenceError::Internal(format!("Model {} not found", model_id)))?,
        )
    }; // HashMap lock released here

    // Lock only this specific model for reranking - allows concurrent requests to different models
    let mut text_rerank = model_mutex.lock().await;

    // Perform reranking
    text_rerank.rerank(query, texts, true, top_k).map_err(|e| {
        error!(error = %e, "Reranking failed");
        InferenceError::Rerank(e.to_string())
    })
}

/// Check if reranker models are loaded and ready
pub fn is_ready() -> bool {
    RERANKER_MODELS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|cache| !cache.is_empty())
        .unwrap_or(false)
}
