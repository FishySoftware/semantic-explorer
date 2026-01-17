use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use once_cell::sync::OnceCell;
use ort::ep::CUDA;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

use crate::config::ModelConfig;
use crate::errors::InferenceError;

/// Type alias for the embedding model cache to reduce complexity
/// Using std::sync::Mutex for the inner lock to allow blocking operations in spawn_blocking
type EmbeddingCache = Arc<Mutex<HashMap<String, Arc<Mutex<TextEmbedding>>>>>;

/// Global embedding model cache - using per-model mutexes for concurrent access
static EMBEDDING_MODELS: OnceCell<EmbeddingCache> = OnceCell::new();

/// Initialize the embedding model cache and pre-load allowed models
///
/// This function:
/// 1. Takes the ModelConfig to determine which models to load
/// 2. Gets a list of models to load (all supported or filtered by allowed_models)
/// 3. Loads each model from the filesystem cache, fetching if needed
/// 4. Pre-populates the cache at startup to validate model availability
pub fn init_cache(config: &ModelConfig) {
    let cache = EMBEDDING_MODELS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Get list of models to load
    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        tracing::info!("No embedding models to pre-load");
        return;
    }

    tracing::info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        "Pre-loading embedding models at startup"
    );

    let mut cache_guard = match cache.lock() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::error!(error = %e, "Failed to acquire embedding cache lock during initialization");
            return;
        }
    };

    for model_id in models_to_load {
        match resolve_embedding_model(&model_id) {
            Ok(embedding_model) => match create_text_embedding(embedding_model, config) {
                Ok(text_embedding) => {
                    cache_guard.insert(model_id.clone(), Arc::new(Mutex::new(text_embedding)));
                    tracing::info!(model_id = %model_id, "Pre-loaded embedding model");
                }
                Err(e) => {
                    tracing::error!(
                        model_id = %model_id,
                        error = %e,
                        "Failed to load embedding model during initialization"
                    );
                }
            },
            Err(e) => {
                tracing::error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to resolve embedding model during initialization"
                );
            }
        }
    }
}

/// Get the list of embedding models to load based on configuration
fn get_models_to_load(config: &ModelConfig) -> Vec<String> {
    if !config.allowed_models.is_empty() {
        // Use allowed models list if configured
        config.allowed_models.clone()
    } else {
        // Use all supported embedding models if no restrictions
        get_all_supported_embedding_models()
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
        info!("CUDA available, using CUDA execution provider for embeddings");
        let cuda_provider = CUDA::default().build();
        TextInitOptions::new(model).with_execution_providers(vec![cuda_provider])
    } else {
        info!("CUDA not available, using CPU execution provider");
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
    if !config.is_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    let models = EMBEDDING_MODELS
        .get()
        .ok_or_else(|| InferenceError::Internal("Embedding cache not initialized".to_string()))?;

    // Get or create the model with minimal lock time on the HashMap
    let model_mutex = {
        let mut cache = models
            .lock()
            .map_err(|e| InferenceError::Internal(format!("Failed to acquire lock: {}", e)))?;

        // Check if model is in cache, if not load it
        if !cache.contains_key(model_id) {
            info!(model_id = %model_id, "Loading embedding model on demand");
            let embedding_model = resolve_embedding_model(model_id)?;
            let text_embedding = create_text_embedding(embedding_model, config)?;
            cache.insert(model_id.to_string(), Arc::new(Mutex::new(text_embedding)));
        }

        Arc::clone(
            cache
                .get(model_id)
                .ok_or_else(|| InferenceError::Internal(format!("Model {} not found", model_id)))?,
        )
    }; // HashMap lock released here

    // Generate embeddings in a blocking task to avoid blocking the async runtime
    let texts_clone = texts.clone();
    let batch_size = Some(config.max_batch_size);
    let model_clone = model_mutex.clone();

    tokio::task::spawn_blocking(move || {
        let mut text_embedding = model_clone.lock().map_err(|e| {
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
    EMBEDDING_MODELS
        .get()
        .and_then(|m| {
            m.lock()
                .map_err(|e| {
                    error!("Failed to lock embedding models cache: {}", e);
                    e
                })
                .ok()
        })
        .map(|cache| !cache.is_empty())
        .unwrap_or(false)
}
