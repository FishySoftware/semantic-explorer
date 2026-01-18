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
/// Returns all catalog models, filtered by allowed_models configuration if set.
/// This matches the embedders pattern where the library provides a catalog
/// and configuration filters what's actually available.
pub fn get_llm_models(config: &ModelConfig) -> Vec<ModelInfo> {
    let catalog = get_model_catalog();

    let models: Vec<ModelInfo> = catalog
        .into_iter()
        .filter(|model| config.is_model_allowed(&model.id))
        .collect();

    models
}

/// Get the complete catalog of known/tested LLM models
///
/// This is a curated list of models that are known to work well with mistral.rs.
/// These models must have tokenizer.json available in their HuggingFace repo.
/// Users can add any HuggingFace model ID to LLM_ALLOWED_MODELS, but this catalog
/// provides a starting point of tested models.
pub fn get_model_catalog() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "mistralai/Mistral-7B-Instruct-v0.2".to_string(),
            name: "Mistral 7B Instruct v0.2".to_string(),
            description: "Mistral AI's 7 billion parameter instruction-tuned model".to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "mistralai/Mistral-7B-v0.1".to_string(),
            name: "Mistral 7B v0.1".to_string(),
            description: "Mistral AI's base 7 billion parameter model".to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string()],
        },
        ModelInfo {
            id: "mistralai/Mixtral-8x7B-Instruct-v0.1".to_string(),
            name: "Mixtral 8x7B Instruct".to_string(),
            description:
                "Mistral AI's mixture of experts model with 8 experts of 7B parameters each"
                    .to_string(),
            size: Some("8x7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "meta-llama/Llama-2-7b-chat-hf".to_string(),
            name: "Llama 2 7B Chat".to_string(),
            description: "Meta's Llama 2 7 billion parameter chat model (requires HF token)"
                .to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "meta-llama/Llama-2-13b-chat-hf".to_string(),
            name: "Llama 2 13B Chat".to_string(),
            description: "Meta's Llama 2 13 billion parameter chat model (requires HF token)"
                .to_string(),
            size: Some("13B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "HuggingFaceH4/zephyr-7b-beta".to_string(),
            name: "Zephyr 7B Beta".to_string(),
            description: "HuggingFaceH4's Zephyr 7B beta model - fine-tuned Mistral".to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "microsoft/Phi-3-mini-4k-instruct".to_string(),
            name: "Phi-3 Mini 4K Instruct".to_string(),
            description: "Microsoft's compact 3.8B parameter model with 4K context".to_string(),
            size: Some("3.8B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "TinyLlama/TinyLlama-1.1B-Chat-v1.0".to_string(),
            name: "TinyLlama 1.1B Chat".to_string(),
            description: "Compact 1.1B model for resource-constrained environments".to_string(),
            size: Some("1.1B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_catalog() {
        let models = get_model_catalog();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("Mistral")));
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
