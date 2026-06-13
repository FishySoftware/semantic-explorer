//! Model discovery and listing service.
//!
//! Provides model information and listing functionality.

use fastembed::TextRerank;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{config::ModelConfig, embedding};

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
}

/// Get information about available embedding models
pub fn get_embedding_models(config: &ModelConfig) -> Vec<ModelInfo> {
    let supported_models = embedding::get_all_available_embedding_models(config);

    let mut models: Vec<ModelInfo> = supported_models
        .iter()
        .map(|m| ModelInfo {
            id: m.model_code.clone(),
            name: m.model_code.clone(),
            description: m.description.clone(),
            model_type: "embedding".to_string(),
            dimensions: Some(m.dim),
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
            if !config.is_rerank_model_allowed(&model_id) {
                return None;
            }

            Some(ModelInfo {
                id: model_id.clone(),
                name: model_id.clone(),
                description: m.description.clone(),
                model_type: "reranker".to_string(),
                dimensions: None,
            })
        })
        .collect();

    // Sort by name
    models.sort_by(|a, b| a.name.cmp(&b.name));

    models
}
