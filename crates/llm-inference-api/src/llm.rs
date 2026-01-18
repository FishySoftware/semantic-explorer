//! LLM model management and text generation.
//!
//! This module provides:
//! - Global model cache with per-model locking for concurrent access
//! - Lazy loading of models on first request
//! - Text generation with configurable parameters
//! - Streaming text generation support
//! - Chat completion with message history

use futures::stream::Stream;
use mistralrs::{
    Model as MistralRsModel, RequestBuilder, TextMessageRole, TextMessages, TextModelBuilder,
    TokenSource,
};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::config::{GenerationConfig, ModelConfig};
use crate::errors::InferenceError;

/// Type alias for the LLM model cache
/// Using tokio::sync::Mutex for async compatibility
type LlmCache = Arc<tokio::sync::Mutex<HashMap<String, Arc<MistralRsModel>>>>;

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

    // Load models sequentially
    for model_id in models_to_load {
        match load_model(&model_id, config).await {
            Ok(model) => {
                let mut cache_guard = cache.lock().await;
                cache_guard.insert(model_id.clone(), Arc::new(model));
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
    // Always use allowed_models (required to be non-empty)
    config.allowed_models.clone()
}

/// Load a model from disk or download it
///
/// This uses mistral.rs TextModelBuilder to:
/// 1. Download model from HuggingFace if not in cache
/// 2. Load model with CUDA support
/// 3. Apply quantization for efficiency
/// 4. Return the loaded model ready for inference
async fn load_model(
    model_id: &str,
    config: &ModelConfig,
) -> Result<MistralRsModel, InferenceError> {
    tracing::info!(model_id = %model_id, "Loading LLM model with mistral.rs");

    if model_id.is_empty() {
        return Err(InferenceError::ModelLoad("Empty model ID".to_string()));
    }

    let mut builder = TextModelBuilder::new(model_id)
        .with_token_source(TokenSource::CacheToken)
        .with_logging();
    // .with_paged_attn(|| PagedAttentionMetaBuilder::default().build())?;

    // Configure HF cache path if specified
    if let Some(ref hf_home) = config.hf_home {
        builder = builder.from_hf_cache_pathf(hf_home.clone());
    }

    // Build the model
    let model = builder.build().await.map_err(|e| {
        InferenceError::ModelLoad(format!("Failed to load model {}: {}", model_id, e))
    })?;

    tracing::info!(model_id = %model_id, "Successfully loaded LLM model");
    Ok(model)
}

/// Get or load a model from the cache
///
/// If the model is not in the cache, it will be loaded and inserted.
/// This allows for lazy loading of models on first request.
async fn get_or_load_model(
    model_id: &str,
    config: &ModelConfig,
) -> Result<Arc<MistralRsModel>, InferenceError> {
    let cache = LLM_MODELS
        .get()
        .ok_or_else(|| InferenceError::Internal("LLM cache not initialized".to_string()))?;

    // Try to get from cache first
    {
        let cache_guard = cache.lock().await;
        if let Some(model) = cache_guard.get(model_id) {
            debug!(model_id = %model_id, "LLM model found in cache");
            return Ok(Arc::clone(model));
        }
    }

    // Not in cache - load it
    info!(model_id = %model_id, "LLM model not in cache, loading");

    let model = load_model(model_id, config).await?;

    // Insert into cache
    let model_arc = Arc::new(model);
    {
        let mut cache_guard = cache.lock().await;
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
    #[allow(dead_code)]
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
#[allow(dead_code)]
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

    // Generate text using mistral.rs
    let request = RequestBuilder::new()
        .set_sampler_max_len(max_tokens)
        .set_sampler_temperature(temperature as f64)
        .set_sampler_topp(top_p as f64)
        .add_message(TextMessageRole::User, &prompt);

    let response = model_arc
        .send_chat_request(request)
        .await
        .map_err(|e| InferenceError::Generation(format!("Generation failed: {}", e)))?;

    // Extract response text
    let text = response.choices[0]
        .message
        .content
        .as_ref()
        .ok_or_else(|| InferenceError::Generation("Empty response from model".to_string()))?
        .clone();

    // Determine finish reason
    let finish_reason = match response.choices[0].finish_reason.as_str() {
        "stop" => FinishReason::Eos,
        "length" => FinishReason::Length,
        _ => FinishReason::Eos,
    };

    Ok(GenerationResponse {
        text,
        model: model_id.to_string(),
        tokens_generated: response.usage.completion_tokens,
        finish_reason,
    })
}

/// Generate text with streaming
///
/// Returns a stream of text chunks as they are generated.
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
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Get model from cache
    let model_arc = get_or_load_model(model_id, model_config).await?;

    // Create streaming request using mistral.rs
    let request = RequestBuilder::new()
        .set_sampler_max_len(max_tokens)
        .set_sampler_temperature(temperature as f64)
        .set_sampler_topp(top_p as f64)
        .add_message(TextMessageRole::User, &prompt);

    // Clone the Arc to move into the stream
    let model_for_stream = model_arc.clone();

    // Create async stream that generates text chunks
    let text_stream = async_stream::try_stream! {
        // Stream the response inside the async block
        let mut stream = model_for_stream
            .stream_chat_request(request)
            .await
            .map_err(|e| InferenceError::Generation(format!("Stream creation failed: {}", e)))?;

        while let Some(response) = stream.next().await {
            match response {
                mistralrs::Response::Chunk(chunk_response) => {
                    // Extract text from the chunk delta
                    if let Some(choice) = chunk_response.choices.first()
                        && let Some(content) = &choice.delta.content
                    {
                        yield content.clone();
                    }
                }
                mistralrs::Response::Done(_) => {
                    // Stream completed successfully
                    break;
                }
                mistralrs::Response::ModelError(msg, _) => {
                    Err(InferenceError::Generation(msg))?;
                }
                mistralrs::Response::ValidationError(e) => {
                    Err(InferenceError::Generation(e.to_string()))?;
                }
                mistralrs::Response::InternalError(e) => {
                    Err(InferenceError::Generation(e.to_string()))?;
                }
                _ => {
                    // Unexpected response type, skip
                    continue;
                }
            }
        }
    };

    Ok(Box::pin(text_stream))
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
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Get model from cache
    let model_arc = get_or_load_model(model_id, model_config).await?;

    // Build chat messages for mistral.rs
    let mut text_messages = TextMessages::new();
    for msg in messages {
        let role = match msg.role.to_lowercase().as_str() {
            "system" => TextMessageRole::System,
            "user" => TextMessageRole::User,
            "assistant" => TextMessageRole::Assistant,
            _ => TextMessageRole::User, // Default to user for unknown roles
        };
        text_messages = text_messages.add_message(role, &msg.content);
    }

    // Build request with parameters
    let request = RequestBuilder::from(text_messages)
        .set_sampler_max_len(max_tokens)
        .set_sampler_temperature(temperature as f64)
        .set_sampler_topp(top_p as f64);

    // Generate chat response using mistral.rs
    let response = model_arc
        .send_chat_request(request)
        .await
        .map_err(|e| InferenceError::Generation(format!("Chat completion failed: {}", e)))?;

    // Extract response
    let content = response.choices[0]
        .message
        .content
        .as_ref()
        .ok_or_else(|| InferenceError::Generation("Empty response from model".to_string()))?
        .clone();

    let finish_reason = match response.choices[0].finish_reason.as_str() {
        "stop" => FinishReason::Eos,
        "length" => FinishReason::Length,
        _ => FinishReason::Eos,
    };

    Ok(ChatResponse {
        message: ChatMessage {
            role: "assistant".to_string(),
            content,
        },
        model: model_id.to_string(),
        tokens_generated: response.usage.completion_tokens,
        finish_reason,
    })
}

/// Check if the LLM service is ready (models loaded)
///
/// Returns true if the cache is initialized.
pub fn is_ready() -> bool {
    LLM_MODELS.get().is_some()
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
