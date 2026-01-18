//! Model discovery and listing functionality.
//!
//! This module provides endpoints for discovering available LLM models.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

/// Response for listing available models
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListModelsResponse {
    /// List of available models
    pub models: Vec<ModelInfo>,
}

/// Get all supported LLM models
///
/// This function returns metadata about all models that the API can serve.
/// The actual availability depends on the allowed_models configuration.
pub fn get_supported_models() -> Vec<ModelInfo> {
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
            description: "Meta's Llama 2 7 billion parameter chat model".to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "meta-llama/Llama-2-13b-chat-hf".to_string(),
            name: "Llama 2 13B Chat".to_string(),
            description: "Meta's Llama 2 13 billion parameter chat model".to_string(),
            size: Some("13B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "TheBloke/zephyr-7B-beta-GGUF".to_string(),
            name: "Zephyr 7B Beta".to_string(),
            description: "HuggingFaceH4's Zephyr 7B beta model in GGUF format".to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
        ModelInfo {
            id: "TheBloke/Llama-2-7B-Chat-GGUF".to_string(),
            name: "Llama 2 7B Chat (GGUF)".to_string(),
            description: "Meta's Llama 2 7B chat model in GGUF format for efficient inference"
                .to_string(),
            size: Some("7B".to_string()),
            capabilities: vec!["text-generation".to_string(), "chat".to_string()],
        },
    ]
}

/// Filter models based on allowed_models configuration
///
/// If allowed_models is empty, all models are returned.
/// Otherwise, only models in the allowed list are returned.
pub fn filter_models(all_models: Vec<ModelInfo>, allowed_models: &[String]) -> Vec<ModelInfo> {
    if allowed_models.is_empty() {
        return all_models;
    }

    all_models
        .into_iter()
        .filter(|model| allowed_models.contains(&model.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_supported_models() {
        let models = get_supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("Mistral")));
    }

    #[test]
    fn test_filter_models_empty_allowed() {
        let models = get_supported_models();
        let count = models.len();
        let filtered = filter_models(models, &[]);
        assert_eq!(filtered.len(), count);
    }

    #[test]
    fn test_filter_models_with_allowed_list() {
        let models = get_supported_models();
        let allowed = vec!["mistralai/Mistral-7B-Instruct-v0.2".to_string()];
        let filtered = filter_models(models, &allowed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "mistralai/Mistral-7B-Instruct-v0.2");
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
