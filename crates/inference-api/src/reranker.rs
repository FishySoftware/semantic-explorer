use fastembed::{RerankInitOptions, RerankerModel, TextRerank};
use once_cell::sync::OnceCell;
use ort::ep::CUDA;
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

/// Initialize the reranker model cache
pub fn init_cache() {
    RERANKER_MODELS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
}

/// Resolve a model ID string to a fastembed RerankerModel enum
fn resolve_reranker_model(model_id: &str) -> Result<RerankerModel, InferenceError> {
    let model = match model_id {
        // BGE Rerankers
        "BAAI/bge-reranker-base" => RerankerModel::BGERerankerBase,
        "rozgo/bge-reranker-v2-m3" | "BAAI/bge-reranker-v2-m3" => RerankerModel::BGERerankerV2M3,

        // Jina Rerankers
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
fn create_text_rerank(
    model: RerankerModel,
    config: &ModelConfig,
) -> Result<TextRerank, InferenceError> {
    // Try CUDA execution provider, fall back to CPU if unavailable
    let mut options = if std::env::var("CUDA_VISIBLE_DEVICES").is_ok() {
        info!("CUDA available, using CUDA execution provider for reranking");
        let cuda_provider = CUDA::default().build();
        RerankInitOptions::new(model).with_execution_providers(vec![cuda_provider])
    } else {
        info!("CUDA not available, using CPU execution provider");
        RerankInitOptions::new(model)
    };

    // Set cache directory if HF_HOME is configured
    if let Some(ref hf_home) = config.hf_home {
        options = options.with_cache_dir(hf_home.clone());
    }

    TextRerank::try_new(options).map_err(|e| {
        error!(error = %e, "Failed to initialize reranker model");
        InferenceError::ModelLoad(e.to_string())
    })
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
    if !config.is_model_allowed(model_id) {
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
