//! Model discovery and listing functionality.
//!
//! This module provides endpoints for discovering available LLM models.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::ModelConfig;

/// Information about an available LLM model
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelInfo {
    /// Model identifier (e.g., "mistralai/Mistral-7B-Instruct-v0.2")
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Model description
    pub description: String,
    /// Model size (parameters, e.g., "7B")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Model capabilities (e.g., ["text-generation", "chat"])
    pub capabilities: Vec<String>,
}

/// Get list of LLM models based on configuration
///
/// Returns the models configured in LLM_ALLOWED_MODELS.
pub fn get_llm_models(config: &ModelConfig) -> Vec<ModelInfo> {
    config
        .allowed_models
        .iter()
        .map(|model_id| create_model_info_from_id(model_id))
        .collect()
}

/// Create ModelInfo from a model ID
/// Handles GGUF, GPTQ, and standard HuggingFace models
fn create_model_info_from_id(model_id: &str) -> ModelInfo {
    let is_gguf = model_id.to_lowercase().contains("-gguf") || model_id.ends_with(".gguf");
    let is_gptq = model_id.to_lowercase().contains("gptq");

    // Extract a friendly name from the model ID
    let name = model_id
        .split('/')
        .next_back()
        .unwrap_or(model_id)
        .replace(['-', '_'], " ");

    let mut description = format!("LLM model: {}", model_id);
    let mut capabilities = vec!["text-generation".to_string()];

    // Add quantization info to description
    if is_gguf {
        description.push_str(" (GGUF quantized)");
    } else if is_gptq {
        description.push_str(" (GPTQ quantized)");
    }

    // Most models support chat
    if model_id.to_lowercase().contains("chat") || model_id.to_lowercase().contains("instruct") {
        capabilities.push("chat".to_string());
    }

    // Try to extract size from model ID (e.g., "7B", "13B")
    let size = extract_model_size(model_id);

    ModelInfo {
        id: model_id.to_string(),
        name,
        description,
        size,
        capabilities,
    }
}

/// Extract model size from model ID (e.g., "7B", "13B", "1.1B")
fn extract_model_size(model_id: &str) -> Option<String> {
    let id_lower = model_id.to_lowercase();

    // Common patterns: 7b, 13b, 70b, 1.1b, 3.8b, etc.
    let size_patterns = [
        "70b", "65b", "33b", "34b", "13b", "8b", "7b", "3.8b", "3b", "1.1b", "1b",
    ];

    for pattern in &size_patterns {
        if id_lower.contains(pattern) {
            return Some(pattern.to_uppercase());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_model_info_from_id() {
        let model = create_model_info_from_id("mistralai/Mistral-7B-Instruct-v0.2");
        assert_eq!(model.id, "mistralai/Mistral-7B-Instruct-v0.2");
        assert!(model.capabilities.contains(&"text-generation".to_string()));
        assert!(model.capabilities.contains(&"chat".to_string()));
        assert_eq!(model.size, Some("7B".to_string()));
    }

    #[test]
    fn test_gguf_model_info() {
        let model = create_model_info_from_id("TheBloke/Mistral-7B-Instruct-v0.2-GGUF");
        assert!(model.description.contains("GGUF quantized"));
        assert_eq!(model.size, Some("7B".to_string()));
    }

    #[test]
    fn test_gptq_model_info() {
        let model = create_model_info_from_id("TheBloke/Mistral-7B-Instruct-v0.2-GPTQ");
        assert!(model.description.contains("GPTQ quantized"));
        assert_eq!(model.size, Some("7B".to_string()));
    }

    #[test]
    fn test_model_info_serialization() {
        let model = ModelInfo {
            id: "test/model".to_string(),
            name: "Test Model".to_string(),
            description: "A test model".to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string()],
        };

        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("test/model"));
        assert!(json.contains("Test Model"));
    }
}
