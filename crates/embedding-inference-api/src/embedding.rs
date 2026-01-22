use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use futures::stream::StreamExt;
use once_cell::sync::OnceCell;
use ort::ep::CUDA;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::config::ModelConfig;
use crate::errors::InferenceError;


type EmbeddingCache = Arc<RwLock<HashMap<String, Arc<Mutex<TextEmbedding>>>>>;

/// Global embedding model cache - using async RwLock for better concurrency
static EMBEDDING_MODELS: OnceCell<EmbeddingCache> = OnceCell::new();

/// Global semaphore for limiting concurrent embedding requests (backpressure)
static EMBEDDING_SEMAPHORE: OnceCell<Arc<Semaphore>> = OnceCell::new();

/// Initialize the embedding semaphore for backpressure control
pub fn init_semaphore(max_concurrent: usize) {
    let permits = max_concurrent.max(1);
    EMBEDDING_SEMAPHORE.get_or_init(|| {
        info!(
            max_concurrent = permits,
            "Initialized embedding request semaphore for backpressure control"
        );
        Arc::new(Semaphore::new(permits))
    });
}

/// Try to acquire a permit for embedding. Returns None if at capacity.
pub fn try_acquire_permit() -> Option<tokio::sync::OwnedSemaphorePermit> {
    EMBEDDING_SEMAPHORE
        .get()
        .and_then(|sem| sem.clone().try_acquire_owned().ok())
}

/// Get current available permits (for monitoring)
pub fn available_permits() -> usize {
    EMBEDDING_SEMAPHORE
        .get()
        .map(|sem| sem.available_permits())
        .unwrap_or(0)
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
        // Load all supported embedding models
        get_all_supported_embedding_models()
    } else {
        // Use specific allowed models list
        config.allowed_embedding_models.clone()
    }
}

/// Get all supported embedding model IDs
fn get_all_supported_embedding_models() -> Vec<String> {
    vec![
        "Alibaba-NLP/gte-base-en-v1.5".to_string(),
        "Alibaba-NLP/gte-base-en-v1.5-Q".to_string(),
        "Alibaba-NLP/gte-large-en-v1.5".to_string(),
        "Alibaba-NLP/gte-large-en-v1.5-Q".to_string(),
        "BAAI/bge-base-en-v1.5".to_string(),
        "BAAI/bge-base-en-v1.5-Q".to_string(),
        "BAAI/bge-large-en-v1.5".to_string(),
        "BAAI/bge-large-en-v1.5-Q".to_string(),
        "BAAI/bge-large-zh-v1.5".to_string(),
        "BAAI/bge-m3".to_string(),
        "BAAI/bge-small-en-v1.5".to_string(),
        "BAAI/bge-small-en-v1.5-Q".to_string(),
        "BAAI/bge-small-zh-v1.5".to_string(),
        "intfloat/multilingual-e5-base".to_string(),
        "intfloat/multilingual-e5-large".to_string(),
        "intfloat/multilingual-e5-small".to_string(),
        "jinaai/jina-embeddings-v2-base-code".to_string(),
        "jinaai/jina-embeddings-v2-base-en".to_string(),
        "lightonai/modernbert-embed-large".to_string(),
        "mixedbread-ai/mxbai-embed-large-v1".to_string(),
        "mixedbread-ai/mxbai-embed-large-v1-Q".to_string(),
        "nomic-ai/nomic-embed-text-v1".to_string(),
        "nomic-ai/nomic-embed-text-v1.5".to_string(),
        "nomic-ai/nomic-embed-text-v1.5-Q".to_string(),
        "onnx-community/embeddinggemma-300m-ONNX".to_string(),
        "Qdrant/clip-ViT-B-32-text".to_string(),
        "sentence-transformers/all-MiniLM-L12-v2".to_string(),
        "sentence-transformers/all-MiniLM-L12-v2-Q".to_string(),
        "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        "sentence-transformers/all-MiniLM-L6-v2-Q".to_string(),
        "sentence-transformers/all-mpnet-base-v2".to_string(),
        "sentence-transformers/paraphrase-MiniLM-L6-v2".to_string(),
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2".to_string(),
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2-Q".to_string(),
        "sentence-transformers/paraphrase-multilingual-mpnet-base-v2".to_string(),
        "snowflake/snowflake-arctic-embed-l".to_string(),
        "snowflake/snowflake-arctic-embed-l-Q".to_string(),
        "snowflake/snowflake-arctic-embed-m".to_string(),
        "snowflake/snowflake-arctic-embed-m-long".to_string(),
        "snowflake/snowflake-arctic-embed-m-long-Q".to_string(),
        "snowflake/snowflake-arctic-embed-m-Q".to_string(),
        "snowflake/snowflake-arctic-embed-s".to_string(),
        "snowflake/snowflake-arctic-embed-s-Q".to_string(),
        "snowflake/snowflake-arctic-embed-xs".to_string(),
        "snowflake/snowflake-arctic-embed-xs-Q".to_string(),
    ]
}

/// Resolve a model ID string to a fastembed EmbeddingModel enum
fn resolve_embedding_model(model_id: &str) -> Result<EmbeddingModel, InferenceError> {
    let model = match model_id {
        "Alibaba-NLP/gte-base-en-v1.5" => EmbeddingModel::GTEBaseENV15,
        "Alibaba-NLP/gte-base-en-v1.5-Q" => EmbeddingModel::GTEBaseENV15Q,
        "Alibaba-NLP/gte-large-en-v1.5" => EmbeddingModel::GTELargeENV15,
        "Alibaba-NLP/gte-large-en-v1.5-Q" => EmbeddingModel::GTELargeENV15Q,
        "BAAI/bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
        "BAAI/bge-base-en-v1.5-Q" => EmbeddingModel::BGEBaseENV15Q,
        "BAAI/bge-large-en-v1.5" => EmbeddingModel::BGELargeENV15,
        "BAAI/bge-large-en-v1.5-Q" => EmbeddingModel::BGELargeENV15Q,
        "BAAI/bge-large-zh-v1.5" => EmbeddingModel::BGELargeZHV15,
        "BAAI/bge-m3" => EmbeddingModel::BGEM3,
        "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
        "BAAI/bge-small-en-v1.5-Q" => EmbeddingModel::BGESmallENV15Q,
        "BAAI/bge-small-zh-v1.5" => EmbeddingModel::BGESmallZHV15,
        "intfloat/multilingual-e5-base" => EmbeddingModel::MultilingualE5Base,
        "intfloat/multilingual-e5-large" => EmbeddingModel::MultilingualE5Large,
        "intfloat/multilingual-e5-small" => EmbeddingModel::MultilingualE5Small,
        "jinaai/jina-embeddings-v2-base-code" => EmbeddingModel::JinaEmbeddingsV2BaseCode,
        "jinaai/jina-embeddings-v2-base-en" => EmbeddingModel::JinaEmbeddingsV2BaseEN,
        "lightonai/modernbert-embed-large" => EmbeddingModel::ModernBertEmbedLarge,
        "mixedbread-ai/mxbai-embed-large-v1" => EmbeddingModel::MxbaiEmbedLargeV1,
        "mixedbread-ai/mxbai-embed-large-v1-Q" => EmbeddingModel::MxbaiEmbedLargeV1Q,
        "nomic-ai/nomic-embed-text-v1" => EmbeddingModel::NomicEmbedTextV1,
        "nomic-ai/nomic-embed-text-v1.5" => EmbeddingModel::NomicEmbedTextV15,
        "nomic-ai/nomic-embed-text-v1.5-Q" => EmbeddingModel::NomicEmbedTextV15Q,
        "onnx-community/embeddinggemma-300m-ONNX" => EmbeddingModel::EmbeddingGemma300M,
        "Qdrant/clip-ViT-B-32-text" => EmbeddingModel::ClipVitB32,
        "sentence-transformers/all-MiniLM-L12-v2" => EmbeddingModel::AllMiniLML12V2,
        "sentence-transformers/all-MiniLM-L12-v2-Q" => EmbeddingModel::AllMiniLML12V2Q,
        "sentence-transformers/all-MiniLM-L6-v2" => EmbeddingModel::AllMiniLML6V2,
        "sentence-transformers/all-MiniLM-L6-v2-Q" => EmbeddingModel::AllMiniLML6V2Q,
        "sentence-transformers/all-mpnet-base-v2" => EmbeddingModel::AllMpnetBaseV2,
        "sentence-transformers/paraphrase-MiniLM-L6-v2"
        | "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2" => {
            EmbeddingModel::ParaphraseMLMiniLML12V2
        }
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2-Q" => {
            EmbeddingModel::ParaphraseMLMiniLML12V2Q
        }
        "sentence-transformers/paraphrase-multilingual-mpnet-base-v2" => {
            EmbeddingModel::ParaphraseMLMpnetBaseV2
        }
        "snowflake/snowflake-arctic-embed-l" => EmbeddingModel::SnowflakeArcticEmbedL,
        "snowflake/snowflake-arctic-embed-l-Q" => EmbeddingModel::SnowflakeArcticEmbedLQ,
        "snowflake/snowflake-arctic-embed-m" => EmbeddingModel::SnowflakeArcticEmbedM,
        "snowflake/snowflake-arctic-embed-m-long" => EmbeddingModel::SnowflakeArcticEmbedMLong,
        "snowflake/snowflake-arctic-embed-m-long-Q" => EmbeddingModel::SnowflakeArcticEmbedMLongQ,
        "snowflake/snowflake-arctic-embed-m-Q" => EmbeddingModel::SnowflakeArcticEmbedMQ,
        "snowflake/snowflake-arctic-embed-s" => EmbeddingModel::SnowflakeArcticEmbedS,
        "snowflake/snowflake-arctic-embed-s-Q" => EmbeddingModel::SnowflakeArcticEmbedSQ,
        "snowflake/snowflake-arctic-embed-xs" => EmbeddingModel::SnowflakeArcticEmbedXS,
        "snowflake/snowflake-arctic-embed-xs-Q" => EmbeddingModel::SnowflakeArcticEmbedXSQ,
        _ => {
            return Err(InferenceError::UnsupportedModel(format!(
                "Unsupported embedding model: {}",
                model_id
            )));
        }
    };

    Ok(model)
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
