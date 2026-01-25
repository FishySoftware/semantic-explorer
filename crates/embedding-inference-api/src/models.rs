//! Model discovery and listing service.
//!
//! Provides model information and listing functionality.

use fastembed::{EmbeddingModel, RerankerModel, TextEmbedding, TextRerank};
use serde::Serialize;
use utoipa::ToSchema;

use crate::config::ModelConfig;

/// Information about an available model
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelInfo {
    /// Model identifier (from FastEmbed's model_code)
    pub id: String,
    /// Human-readable model name (extracted from model_code)
    pub name: String,
    /// Model description
    pub description: String,
    /// Model type (embedding or reranker)
    pub model_type: String,
    /// Output dimensions (for embeddings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
    /// Whether this is a quantized model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_quantized: Option<bool>,
}

/// Get information about available embedding models
pub fn get_embedding_models(config: &ModelConfig) -> Vec<ModelInfo> {
    let supported_models = TextEmbedding::list_supported_models();

    let mut models: Vec<ModelInfo> = supported_models
        .iter()
        .filter_map(|m| {
            let model_id = m.model_code.clone();
            let is_quantized = detect_quantized(&m.model_code, &m.model);

            // Filter by allowed models if configured
            if !config.is_embedding_model_allowed(&model_id) {
                return None;
            }

            Some(ModelInfo {
                id: model_id.clone(),
                name: extract_model_name(&model_id),
                description: m.description.clone(),
                model_type: "embedding".to_string(),
                dimensions: Some(m.dim),
                is_quantized: Some(is_quantized),
            })
        })
        .collect();

    // Sort by name
    models.sort_by(|a, b| a.name.cmp(&b.name));

    models
}

/// Get information about available reranker models
pub fn get_reranker_models(config: &ModelConfig) -> Vec<ModelInfo> {
    let supported_models = TextRerank::list_supported_models();

    let mut models: Vec<ModelInfo> = supported_models
        .iter()
        .filter_map(|m| {
            let model_id = m.model_code.clone();
            let is_quantized = detect_quantized_reranker(&m.model_code, &m.model);

            // Filter by allowed models if configured
            if !config.is_rerank_model_allowed(&model_id) {
                return None;
            }

            Some(ModelInfo {
                id: model_id.clone(),
                name: extract_model_name(&model_id),
                description: m.description.clone(),
                model_type: "reranker".to_string(),
                dimensions: None,
                is_quantized: Some(is_quantized),
            })
        })
        .collect();

    // Sort by name
    models.sort_by(|a, b| a.name.cmp(&b.name));

    models
}

/// Detect if an embedding model is quantized
fn detect_quantized(model_code: &str, model_enum: &EmbeddingModel) -> bool {
    // Check enum variant name (quantized variants end with 'Q')
    if format!("{:?}", model_enum).ends_with('Q') {
        return true;
    }
    // Check model_code for quantization indicators
    model_code.contains("-Q")
        || model_code.contains("-quantized")
        || model_code.contains("quantized")
        || model_code.contains("Qdrant/all-MiniLM-L6-v2-v1")
}

/// Detect if a reranker model is quantized
fn detect_quantized_reranker(model_code: &str, model_enum: &RerankerModel) -> bool {
    // Check enum variant name (quantized variants end with 'Q')
    if format!("{:?}", model_enum).ends_with('Q') {
        return true;
    }
    // Check model_code for quantization indicators
    model_code.contains("-Q")
        || model_code.contains("-quantized")
        || model_code.contains("quantized")
}

/// Extract model name from HuggingFace model code
fn extract_model_name(model_code: &str) -> String {
    // Extract model name from HuggingFace ID
    // e.g., "Qdrant/all-MiniLM-L6-v2-v1" -> "all-MiniLM-L6-v2-v1"
    // e.g., "BAAI/bge-small-en-v1.5" -> "bge-small-en-v1.5"
    model_code
        .split('/')
        .next_back()
        .unwrap_or(model_code)
        .to_string()
}
