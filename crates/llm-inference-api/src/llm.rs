//! LLM model management and text generation.
//!
//! This module provides:
//! - Global model cache with per-model locking for concurrent access
//! - Lazy loading of models on first request
//! - Text generation with configurable parameters
//! - Streaming text generation support
//! - Chat completion with message history

use futures::stream::Stream;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};

use crate::config::{GenerationConfig, ModelConfig};
use crate::errors::InferenceError;

// NOTE: mistral.rs integration will need to be implemented based on the actual API
// This is a placeholder structure that follows the embedding.rs pattern
// TODO: Replace with actual mistral.rs types once API is confirmed

/// Placeholder for mistral.rs model type
/// This will be replaced with the actual mistralrs::Model or equivalent
pub struct MistralModel {
    model_id: String,
    // TODO: Add actual mistral.rs model fields
}

/// Type alias for the LLM model cache
type LlmCache = Arc<Mutex<HashMap<String, Arc<Mutex<MistralModel>>>>>;

/// Global LLM model cache - using per-model mutexes for concurrent access
static LLM_MODELS: OnceCell<LlmCache> = OnceCell::new();

/// Initialize the LLM model cache and pre-load allowed models
///
/// This function:
/// 1. Takes the ModelConfig to determine which models to load
/// 2. Gets a list of models to load (all supported or filtered by allowed_models)
/// 3. Loads each model from the filesystem cache, fetching if needed
/// 4. Pre-populates the cache at startup to validate model availability
pub async fn init_cache(config: &ModelConfig) {
    let cache = LLM_MODELS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Get list of models to load
    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        tracing::info!("No LLM models to pre-load");
        return;
    }

    tracing::info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        "Pre-loading LLM models at startup with concurrency limit"
    );

    let concurrency_limit = config.max_concurrent_requests.min(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
    );

    tracing::info!("Using concurrency limit: {}", concurrency_limit);

    // Sequential loading for now - can be parallelized once mistral.rs API is confirmed
    // TODO: Implement parallel loading similar to embedding.rs once mistral.rs integration is complete
    for model_id in models_to_load {
        match load_model(&model_id, config) {
            Ok(model) => {
                let mut cache_guard = match cache.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to acquire LLM cache lock during initialization");
                        continue;
                    }
                };
                cache_guard.insert(model_id.clone(), Arc::new(Mutex::new(model)));
                tracing::info!(model_id = %model_id, "Pre-loaded LLM model");
            }
            Err(e) => {
                tracing::error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to load LLM model during initialization"
                );
            }
        }
    }
}

/// Get the list of LLM models to load based on configuration
fn get_models_to_load(config: &ModelConfig) -> Vec<String> {
    if !config.allowed_models.is_empty() {
        // Use allowed models list if configured
        config.allowed_models.clone()
    } else {
        // Use default model if no restrictions
        vec![config.default_model.clone()]
    }
}

/// Load a model from disk or download it
///
/// TODO: Implement actual mistral.rs model loading
/// This should:
/// 1. Check if model exists in HF_HOME cache
/// 2. Download from HuggingFace if not present
/// 3. Initialize the model with CUDA if available
/// 4. Return the loaded model
fn load_model(model_id: &str, config: &ModelConfig) -> Result<MistralModel, InferenceError> {
    tracing::info!(model_id = %model_id, "Loading LLM model");

    // TODO: Implement actual mistral.rs model loading
    // Example structure:
    let model_loader = mistralrs::Loader::new()
        .with_hf_cache(config.hf_home)
        .with_hf_endpoint(config.hf_endpoint)
        .with_cuda(true);
    let model = model_loader.load(model_id)?;

    // Placeholder implementation
    Ok(MistralModel {
        model_id: model_id.to_string(),
    })
}

/// Get or load a model from the cache
///
/// If the model is not in the cache, it will be loaded and inserted.
/// This allows for lazy loading of models on first request.
async fn get_or_load_model(
    model_id: &str,
    config: &ModelConfig,
) -> Result<Arc<Mutex<MistralModel>>, InferenceError> {
    let cache = LLM_MODELS
        .get()
        .ok_or_else(|| InferenceError::Internal("LLM cache not initialized".to_string()))?;

    // Try to get from cache first
    {
        let cache_guard = cache.lock().map_err(|e| {
            InferenceError::Internal(format!("Failed to acquire cache lock: {}", e))
        })?;

        if let Some(model) = cache_guard.get(model_id) {
            debug!(model_id = %model_id, "LLM model found in cache");
            return Ok(Arc::clone(model));
        }
    }

    // Not in cache - load it
    info!(model_id = %model_id, "LLM model not in cache, loading");

    let model = tokio::task::spawn_blocking({
        let model_id = model_id.to_string();
        let config = config.clone();
        move || load_model(&model_id, &config)
    })
    .await
    .map_err(|e| InferenceError::ModelLoad(format!("Failed to spawn model loading task: {}", e)))?
    .map_err(|e| InferenceError::ModelLoad(e.to_string()))?;

    // Insert into cache
    let model_arc = Arc::new(Mutex::new(model));
    {
        let mut cache_guard = cache.lock().map_err(|e| {
            InferenceError::Internal(format!("Failed to acquire cache lock: {}", e))
        })?;

        cache_guard.insert(model_id.to_string(), Arc::clone(&model_arc));
    }

    Ok(model_arc)
}

/// Parameters for text generation
#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: usize,
    pub stop_sequences: Vec<String>,
}

/// Response from text generation
#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub text: String,
    pub model: String,
    pub tokens_generated: usize,
    pub finish_reason: FinishReason,
}

/// Reason why generation stopped
#[derive(Debug, Clone, PartialEq)]
pub enum FinishReason {
    /// Reached max tokens limit
    Length,
    /// Hit a stop sequence
    Stop,
    /// Model generated EOS token
    Eos,
    /// Error occurred
    Error,
}

impl std::fmt::Display for FinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FinishReason::Length => write!(f, "length"),
            FinishReason::Stop => write!(f, "stop"),
            FinishReason::Eos => write!(f, "eos"),
            FinishReason::Error => write!(f, "error"),
        }
    }
}

/// Generate text from a prompt
///
/// This is the main text generation function for non-streaming requests.
pub async fn generate_text(
    model_id: &str,
    prompt: String,
    params: GenerationParams,
    model_config: &ModelConfig,
    gen_config: &GenerationConfig,
) -> Result<GenerationResponse, InferenceError> {
    // Check if model is allowed
    if !model_config.is_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Validate parameters
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Get model from cache
    let model_arc = get_or_load_model(model_id, model_config).await?;

    // Generate text
    // TODO: Implement actual mistral.rs generation
    // This should run in spawn_blocking since it's CPU-intensive
    let result = tokio::task::spawn_blocking({
        let model_id = model_id.to_string();
        move || {
            // Placeholder implementation
            // TODO: Replace with actual mistral.rs generation:
            // let model_guard = model_arc.lock()?;
            // let output = model_guard.generate(&prompt, temperature, top_p, max_tokens)?;
            // Ok(GenerationResponse {
            //     text: output.text,
            //     model: model_id,
            //     tokens_generated: output.tokens.len(),
            //     finish_reason: map_finish_reason(output.finish_reason),
            // })

            Ok(GenerationResponse {
                text: format!("Generated response for prompt: {}", prompt),
                model: model_id,
                tokens_generated: 10,
                finish_reason: FinishReason::Length,
            })
        }
    })
    .await
    .map_err(|e| InferenceError::Generation(format!("Generation task failed: {}", e)))??;

    Ok(result)
}

/// Generate text with streaming
///
/// Returns a stream of text chunks as they are generated.
/// TODO: Implement actual streaming once mistral.rs API is confirmed
pub async fn generate_text_stream(
    model_id: &str,
    prompt: String,
    params: GenerationParams,
    model_config: &ModelConfig,
    gen_config: &GenerationConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, InferenceError>> + Send>>, InferenceError> {
    // Check if model is allowed
    if !model_config.is_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Validate parameters
    let _temperature = gen_config.validate_temperature(params.temperature);
    let _top_p = gen_config.validate_top_p(params.top_p);
    let _max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Get model from cache
    let _model_arc = get_or_load_model(model_id, model_config).await?;

    // TODO: Implement actual streaming
    // This should create a stream that yields tokens as they're generated
    // For now, return a placeholder stream
    use futures::stream;
    let chunks = vec![
        Ok("Generated ".to_string()),
        Ok("streaming ".to_string()),
        Ok("response".to_string()),
    ];
    Ok(Box::pin(stream::iter(chunks)))
}

/// Message for chat completion
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

/// Response from chat completion
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub model: String,
    pub tokens_generated: usize,
    pub finish_reason: FinishReason,
}

/// Chat completion with message history
///
/// Generates a response based on conversation history.
pub async fn chat_completion(
    model_id: &str,
    messages: Vec<ChatMessage>,
    params: GenerationParams,
    model_config: &ModelConfig,
    gen_config: &GenerationConfig,
) -> Result<ChatResponse, InferenceError> {
    // Check if model is allowed
    if !model_config.is_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Validate parameters
    let _temperature = gen_config.validate_temperature(params.temperature);
    let _top_p = gen_config.validate_top_p(params.top_p);
    let _max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Get model from cache
    let _model_arc = get_or_load_model(model_id, model_config).await?;

    // Generate chat response
    // TODO: Implement actual mistral.rs chat completion
    let result = tokio::task::spawn_blocking({
        let model_id = model_id.to_string();
        move || {
            // Placeholder implementation
            // TODO: Replace with actual mistral.rs chat completion:
            // let model_guard = model_arc.lock()?;
            // let output = model_guard.chat(&messages, temperature, top_p, max_tokens)?;

            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: format!("Chat response based on {} messages", messages.len()),
                },
                model: model_id,
                tokens_generated: 15,
                finish_reason: FinishReason::Eos,
            })
        }
    })
    .await
    .map_err(|e| InferenceError::Generation(format!("Chat completion task failed: {}", e)))??;

    Ok(result)
}

/// Check if the LLM service is ready (models loaded)
///
/// Returns true if the cache is initialized and contains at least one model.
pub fn is_ready() -> bool {
    LLM_MODELS
        .get()
        .and_then(|m| {
            m.lock()
                .map_err(|e| {
                    error!("Failed to lock LLM models cache: {}", e);
                    e
                })
                .ok()
        })
        .map(|cache| !cache.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finish_reason_display() {
        assert_eq!(FinishReason::Length.to_string(), "length");
        assert_eq!(FinishReason::Stop.to_string(), "stop");
        assert_eq!(FinishReason::Eos.to_string(), "eos");
        assert_eq!(FinishReason::Error.to_string(), "error");
    }

    #[test]
    fn test_generation_params() {
        let params = GenerationParams {
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 100,
            stop_sequences: vec!["STOP".to_string()],
        };

        assert_eq!(params.temperature, 0.7);
        assert_eq!(params.top_p, 0.9);
        assert_eq!(params.max_tokens, 100);
        assert_eq!(params.stop_sequences.len(), 1);
    }
}
