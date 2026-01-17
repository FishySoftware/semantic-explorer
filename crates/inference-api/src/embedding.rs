use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use once_cell::sync::OnceCell;
use ort::ep::CUDA;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info};

use crate::config::ModelConfig;
use crate::errors::InferenceError;

/// Type alias for the embedding model cache to reduce complexity
type EmbeddingCache = Arc<Mutex<HashMap<String, Arc<TokioMutex<TextEmbedding>>>>>;

/// Global embedding model cache - using per-model mutexes for concurrent access
/// The outer Mutex protects the HashMap structure, while each model has its own Tokio Mutex
static EMBEDDING_MODELS: OnceCell<EmbeddingCache> = OnceCell::new();

/// Initialize the embedding model cache
pub fn init_cache() {
    EMBEDDING_MODELS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
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
            cache.insert(
                model_id.to_string(),
                Arc::new(TokioMutex::new(text_embedding)),
            );
        }

        Arc::clone(
            cache
                .get(model_id)
                .ok_or_else(|| InferenceError::Internal(format!("Model {} not found", model_id)))?,
        )
    }; // HashMap lock released here

    // Lock only this specific model for inference - allows concurrent requests to different models
    let mut text_embedding = model_mutex.lock().await;

    // Use config batch size for faster processing
    let batch_size = Some(config.max_batch_size);
    text_embedding.embed(texts, batch_size).map_err(|e| {
        error!(error = %e, "Embedding generation failed");
        InferenceError::Embedding(e.to_string())
    })
}

/// Check if models are loaded and ready
pub fn is_ready() -> bool {
    EMBEDDING_MODELS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|cache| !cache.is_empty())
        .unwrap_or(false)
}
