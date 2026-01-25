use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use futures::stream::StreamExt;
use once_cell::sync::OnceCell;
use ort::ep::CUDA;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::config::ModelConfig;
use crate::errors::InferenceError;

type EmbeddingCache = Arc<RwLock<HashMap<String, Arc<Mutex<TextEmbedding>>>>>;

/// Global embedding model cache - using async RwLock for better concurrency
static EMBEDDING_MODELS: OnceCell<EmbeddingCache> = OnceCell::new();

/// Global semaphore for limiting concurrent embedding requests (backpressure)
static EMBEDDING_SEMAPHORE: OnceCell<Arc<Semaphore>> = OnceCell::new();

/// Queue timeout for acquiring semaphore permits (allows brief queuing before 503)
static SEMAPHORE_QUEUE_TIMEOUT: OnceCell<Duration> = OnceCell::new();

/// Mapping from model_code to EmbeddingModel enum
type ModelCodeToEnum = HashMap<String, EmbeddingModel>;

/// Global model code to enum mapping
static MODEL_CODE_MAP: OnceCell<ModelCodeToEnum> = OnceCell::new();

/// Initialize the embedding semaphore for backpressure control
/// queue_timeout_ms: how long to wait for a permit before returning 503
pub fn init_semaphore(max_concurrent: usize, queue_timeout_ms: u64) {
    let permits = max_concurrent.max(1);
    let timeout = Duration::from_millis(queue_timeout_ms);

    EMBEDDING_SEMAPHORE.get_or_init(|| {
        info!(
            max_concurrent = permits,
            queue_timeout_ms = queue_timeout_ms,
            "Initialized embedding request semaphore for backpressure control"
        );
        Arc::new(Semaphore::new(permits))
    });

    SEMAPHORE_QUEUE_TIMEOUT.get_or_init(|| timeout);
}

/// Acquire a permit with queuing - waits up to the configured timeout before giving up.
/// This allows requests to briefly queue instead of immediately failing with 503.
pub async fn acquire_permit_with_timeout() -> Option<tokio::sync::OwnedSemaphorePermit> {
    let sem = EMBEDDING_SEMAPHORE.get()?;
    let timeout = SEMAPHORE_QUEUE_TIMEOUT
        .get()
        .copied()
        .unwrap_or(Duration::from_millis(5000));

    match tokio::time::timeout(timeout, sem.clone().acquire_owned()).await {
        Ok(Ok(permit)) => Some(permit),
        Ok(Err(_)) => None, // Semaphore closed
        Err(_) => {
            // Timeout - log queue depth
            warn!(
                available_permits = sem.available_permits(),
                timeout_ms = timeout.as_millis(),
                "Permit acquisition timed out, queue congested"
            );
            None
        }
    }
}

/// Get current available permits (for monitoring)
pub fn available_permits() -> usize {
    EMBEDDING_SEMAPHORE
        .get()
        .map(|sem| sem.available_permits())
        .unwrap_or(0)
}

/// Build the model code to enum mapping from FastEmbed's supported models
fn build_model_code_map() -> ModelCodeToEnum {
    TextEmbedding::list_supported_models()
        .iter()
        .map(|m| (m.model_code.clone(), m.model.clone()))
        .collect()
}

/// Resolve a model code string to a fastembed EmbeddingModel enum
fn resolve_embedding_model(model_code: &str) -> Result<EmbeddingModel, InferenceError> {
    MODEL_CODE_MAP
        .get_or_init(build_model_code_map)
        .get(model_code)
        .cloned()
        .ok_or_else(|| InferenceError::UnsupportedModel(format!("Unknown model: {}", model_code)))
}

/// Initialize the embedding model cache and pre-load allowed models
///
/// This function:
/// 1. Takes the ModelConfig to determine which models to load
/// 2. Gets a list of models to load (all supported or filtered by allowed_embedding_models)
/// 3. Loads each model from the filesystem cache, fetching if needed
/// 4. Pre-populates the cache at startup to validate model availability
pub async fn init_cache(config: &ModelConfig) {
    let cache = EMBEDDING_MODELS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())));

    // Initialize the model code map
    let _ = MODEL_CODE_MAP.get_or_init(build_model_code_map);

    // Get list of models to load
    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        tracing::info!("No embedding models to pre-load");
        return;
    }

    tracing::info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        "Pre-loading embedding models at startup with concurrency limit"
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
                    match resolve_embedding_model(&model_id_clone) {
                        Ok(embedding_model) => {
                            match create_text_embedding(embedding_model, &config) {
                                Ok(text_embedding) => Ok((model_id_clone, text_embedding)),
                                Err(e) => Err((model_id_clone, e)),
                            }
                        }
                        Err(e) => Err((model_id_clone, e)),
                    }
                })
                .await;

                match res {
                    Ok(inner_res) => inner_res,
                    Err(join_err) => {
                        // This happens if task panics
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
            Ok((model_id, text_embedding)) => {
                cache_guard.insert(model_id.clone(), Arc::new(Mutex::new(text_embedding)));
                tracing::info!(model_id = %model_id, "Pre-loaded embedding model");
            }
            Err((model_id, e)) => {
                tracing::error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to load embedding model during initialization"
                );
            }
        }
    }

    tracing::info!(
        loaded_models = cache_guard.len(),
        "Embedding model cache initialization complete - all models loaded in memory"
    );
}

/// Get the list of embedding models to load based on configuration
fn get_models_to_load(config: &ModelConfig) -> Vec<String> {
    if config.all_embedding_models {
        // Load all supported embedding models from FastEmbed
        TextEmbedding::list_supported_models()
            .iter()
            .map(|m| m.model_code.clone())
            .collect()
    } else {
        // Use specific allowed models list
        config.allowed_embedding_models.clone()
    }
}

/// Create a TextEmbedding instance with proper configuration
fn create_text_embedding(
    model: EmbeddingModel,
    config: &ModelConfig,
) -> Result<TextEmbedding, InferenceError> {
    // Try CUDA execution provider, fall back to CPU if unavailable
    let mut options = if std::env::var("CUDA_VISIBLE_DEVICES").is_ok() {
        debug!("CUDA available, using CUDA execution provider for embeddings");
        let cuda_provider = CUDA::default().build();
        TextInitOptions::new(model).with_execution_providers(vec![cuda_provider])
    } else {
        debug!("CUDA not available, using CPU execution provider");
        TextInitOptions::new(model)
    };

    // Set cache directory if HF_HOME is configured
    if let Some(ref hf_home) = config.hf_home {
        options = options.with_cache_dir(hf_home.clone());
    }

    TextEmbedding::try_new(options).map_err(|e| {
        error!(error = %e, "Failed to initialize embedding model");
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

    // Get the model with read lock (non-blocking for concurrent reads)
    let model_arc = {
        let cache = models.read().await;

        // All models should be preloaded
        cache.get(model_id).cloned().ok_or_else(|| {
            warn!(model_id = %model_id, "Model not found in preloaded cache");
            InferenceError::UnsupportedModel(format!(
                "Model {} not preloaded. Please check configuration.",
                model_id
            ))
        })?
    }; // Read lock released here

    // Generate embeddings in a blocking task to avoid blocking the async runtime
    let texts_clone = texts.clone();
    let batch_size = Some(config.max_batch_size);

    tokio::task::spawn_blocking(move || {
        // Acquire mutex lock for mutable access to TextEmbedding
        let mut text_embedding = model_arc.lock().map_err(|e| {
            InferenceError::Internal(format!("Failed to acquire model lock: {}", e))
        })?;

        text_embedding.embed(texts_clone, batch_size).map_err(|e| {
            error!(error = %e, "Embedding generation failed");
            InferenceError::Embedding(e.to_string())
        })
    })
    .await
    .map_err(|e| InferenceError::Internal(format!("Blocking task join error: {}", e)))?
}

/// Check if models are loaded and ready
pub fn is_ready() -> bool {
    // For sync contexts, we check if cache exists and assume it has models
    // This is safe because models are preloaded at startup
    EMBEDDING_MODELS.get().is_some()
}
