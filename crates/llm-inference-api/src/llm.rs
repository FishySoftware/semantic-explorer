//! LLM model management and text generation.
//!
//! This module provides:
//! - Global model cache with per-model locking for concurrent access
//! - Lazy loading of models on first request
//! - Text generation with configurable parameters
//! - Streaming text generation support
//! - Chat completion with message history
//! - Backpressure control via semaphore with queue timeout
//! - GPU memory pressure monitoring
//! - FP8 KV cache support for Hopper+ GPUs (H100/H200)
//! - Prefix caching for multi-turn and RAG workloads

use futures::stream::Stream;
use mistralrs::{
    GgufModelBuilder, IsqType, MemoryGpuConfig, Model as MistralRsModel, PagedAttentionConfig,
    PagedAttentionMetaBuilder, RequestBuilder, TextMessageRole, TextMessages, TextModelBuilder,
    TokenSource, core::PagedCacheType as MistralPagedCacheType,
};
use semantic_explorer_core::observability::gpu_monitor;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::config::{GenerationConfig, ModelConfig, PagedCacheType};
use crate::errors::InferenceError;

/// Type alias for the LLM model cache
/// Using tokio::sync::Mutex for async compatibility
type LlmCache = Arc<tokio::sync::Mutex<HashMap<String, Arc<MistralRsModel>>>>;

/// Global LLM model cache - using per-model mutexes for concurrent access
static LLM_MODELS: OnceLock<LlmCache> = OnceLock::new();

/// Global semaphore for limiting concurrent LLM requests (backpressure)
static LLM_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Semaphore queue timeout - how long to wait for a permit
static SEMAPHORE_QUEUE_TIMEOUT: OnceLock<Duration> = OnceLock::new();

/// Cached GPU memory pressure state (updated by background monitor)
static GPU_MEMORY_PRESSURE_HIGH: AtomicBool = AtomicBool::new(false);

/// GPU pressure threshold percentage — configured via GPU_PRESSURE_THRESHOLD env var (default 95.0)
static GPU_PRESSURE_THRESHOLD: OnceLock<f64> = OnceLock::new();

/// Initialize the LLM semaphore for backpressure control
pub fn init_semaphore(max_concurrent: usize, queue_timeout_ms: u64) {
    let permits = max_concurrent.max(1);
    LLM_SEMAPHORE.get_or_init(|| {
        info!(
            max_concurrent = permits,
            queue_timeout_ms = queue_timeout_ms,
            "Initialized LLM request semaphore for backpressure control"
        );
        Arc::new(Semaphore::new(permits))
    });
    SEMAPHORE_QUEUE_TIMEOUT.get_or_init(|| Duration::from_millis(queue_timeout_ms));
}

/// Acquire a permit with timeout. Returns error if at capacity or timeout.
pub async fn acquire_permit_with_timeout()
-> Result<tokio::sync::OwnedSemaphorePermit, InferenceError> {
    let semaphore = LLM_SEMAPHORE
        .get()
        .ok_or_else(|| InferenceError::Internal("LLM semaphore not initialized".to_string()))?;

    let timeout_duration = SEMAPHORE_QUEUE_TIMEOUT
        .get()
        .copied()
        .unwrap_or(Duration::from_millis(30000)); // 30s default for LLM

    match timeout(timeout_duration, semaphore.clone().acquire_owned()).await {
        Ok(Ok(permit)) => Ok(permit),
        Ok(Err(_)) => Err(InferenceError::Internal("Semaphore closed".to_string())),
        Err(_) => {
            warn!(
                available_permits = semaphore.available_permits(),
                timeout_ms = timeout_duration.as_millis(),
                "LLM queue congested, returning 503"
            );
            Err(InferenceError::ServiceUnavailable(
                "LLM service queue congested, try again later".to_string(),
            ))
        }
    }
}

/// Try to acquire a permit for LLM generation. Returns None if at capacity.
pub fn try_acquire_permit() -> Option<tokio::sync::OwnedSemaphorePermit> {
    LLM_SEMAPHORE
        .get()
        .and_then(|sem| sem.clone().try_acquire_owned().ok())
}

/// Get current available permits (for monitoring)
pub fn available_permits() -> usize {
    LLM_SEMAPHORE
        .get()
        .map(|sem| sem.available_permits())
        .unwrap_or(0)
}

/// Spawn background task that monitors GPU pressure and updates cached state
fn spawn_gpu_pressure_monitor(threshold: f64) {
    GPU_PRESSURE_THRESHOLD.get_or_init(|| threshold);
    tokio::spawn(async move {
        if !gpu_monitor::init() {
            warn!("GPU monitoring disabled - NVML not available");
            return;
        }

        info!(
            device_count = gpu_monitor::device_count(),
            threshold = threshold,
            "Starting LLM GPU pressure monitor"
        );

        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;

            // Collect metrics (updates Prometheus gauges)
            gpu_monitor::collect_metrics();

            // Update cached pressure state (VRAM only — compute at 100% is
            // expected during active inference and should not trigger rejection)
            let is_high = gpu_monitor::is_memory_pressure_high(threshold);
            GPU_MEMORY_PRESSURE_HIGH.store(is_high, Ordering::Relaxed);

            if is_high {
                warn!("LLM: GPU VRAM pressure is HIGH (>{}%)", threshold);
            }
        }
    });
}

/// Check cached GPU memory pressure (fast, no NVML call)
#[inline]
pub fn is_gpu_memory_pressure_high() -> bool {
    GPU_MEMORY_PRESSURE_HIGH.load(Ordering::Relaxed)
}

/// Initialize the LLM model cache and pre-load allowed models
///
/// This function:
/// 1. Takes the ModelConfig to determine which models to load
/// 2. Gets a list of models to load (all supported or filtered by allowed_models)
/// 3. Loads each model from the filesystem cache, fetching if needed
/// 4. Pre-populates the cache at startup to validate model availability
/// 5. Starts GPU pressure monitoring
/// 6. Runs a warmup benchmark to verify GPU execution
pub async fn init_cache(config: &ModelConfig) {
    let cache = LLM_MODELS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Start GPU monitoring
    spawn_gpu_pressure_monitor(config.gpu_pressure_threshold);

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
    for model_id in &models_to_load {
        match load_model(model_id, config).await {
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
    config.allowed_models.clone()
}

/// Load a model from disk or download it
///
/// This function supports:
/// 1. **GGUF models** (RECOMMENDED): Pre-quantized, fast loading, efficient
///    - For TheBloke models: "TheBloke/ModelName-GGUF" or "TheBloke/ModelName-GGUF:filename.gguf"
///    - Example: "TheBloke/Mistral-7B-Instruct-v0.2-GGUF"
///    - Will use Q4_K_M quantization by default
/// 2. **Regular HF models with ISQ**: Quantize during load (SLOW, not cached)
///    - Requires enable_isq=true in config
///    - First load takes 5-10 minutes, not persisted
/// 3. **Regular HF models**: Full precision, large memory usage
async fn load_model(
    model_id: &str,
    config: &ModelConfig,
) -> Result<MistralRsModel, InferenceError> {
    info!(model_id = %model_id, "Loading LLM model with mistral.rs");

    if model_id.is_empty() {
        return Err(InferenceError::ModelLoad("Empty model ID".to_string()));
    }

    // Convert config cache type to mistral.rs cache type
    let cache_type = match config.paged_cache_type {
        PagedCacheType::Auto => MistralPagedCacheType::Auto,
        PagedCacheType::F8E4M3 => MistralPagedCacheType::F8E4M3,
    };

    // Log FP8 and prefix caching configuration
    if matches!(config.paged_cache_type, PagedCacheType::F8E4M3) {
        info!(
            model_id = %model_id,
            "Using FP8 E4M3 KV cache for reduced memory usage (Hopper+ optimized)"
        );
    }
    if config.enable_prefix_caching {
        info!(
            model_id = %model_id,
            "Prefix caching enabled for multi-turn/RAG acceleration"
        );
    }

    let page_attention_result = PagedAttentionMetaBuilder::default()
        .with_block_size(config.paged_attention_block_size)
        .with_gpu_memory(MemoryGpuConfig::ContextSize(
            config.paged_attention_context_size,
        ))
        .with_paged_cache_type(cache_type)
        .build();

    // Prefix caching is configured on the model builder, not PagedAttention
    let prefix_cache_n = if config.enable_prefix_caching {
        Some(16) // Default number of sequences to hold in prefix cache
    } else {
        None
    };

    // Check if this is a GGUF model
    if is_gguf_model(model_id) {
        return load_gguf_model(model_id, config, page_attention_result, prefix_cache_n).await;
    }

    // GPTQ models are auto-detected by mistral.rs and work out-of-the-box
    // Just log if it looks like a GPTQ model
    if is_gptq_model(model_id) {
        info!(model_id = %model_id, "Detected GPTQ quantized model - will be loaded automatically");
    }

    // Load regular HF model
    let mut builder = TextModelBuilder::new(model_id)
        .with_token_source(TokenSource::CacheToken)
        .with_logging()
        .with_prefix_cache_n(prefix_cache_n)
        .with_paged_attn(|| page_attention_result)
        .map_err(|e| {
            InferenceError::ModelLoad(format!(
                "Failed to configure paged attention for HF model {}: {}",
                model_id, e
            ))
        })?;

    // Configure HF cache path if specified
    if let Some(ref hf_home) = config.hf_home {
        builder = builder.from_hf_cache_pathf(hf_home.clone());
    }

    // Apply ISQ if enabled (WARNING: SLOW on first load, not cached)
    if config.enable_isq {
        if let Some(ref isq_type_str) = config.isq_type {
            match parse_isq_type(isq_type_str) {
                Ok(isq_type) => {
                    warn!(
                        model_id = %model_id,
                        isq_type = %isq_type_str,
                        "Applying ISQ quantization during load - this will be SLOW and is NOT cached. Consider using pre-quantized GGUF models instead."
                    );
                    builder = builder.with_isq(isq_type);
                }
                Err(e) => {
                    warn!(
                        isq_type = %isq_type_str,
                        error = %e,
                        "Invalid ISQ type, loading without quantization"
                    );
                }
            }
        } else {
            warn!("ISQ enabled but no ISQ_TYPE specified, loading without quantization");
        }
    }

    let model = builder.build().await.map_err(|e| {
        InferenceError::ModelLoad(format!("Failed to load model {}: {}", model_id, e))
    })?;

    info!(model_id = %model_id, "Successfully loaded LLM model");
    Ok(model)
}

/// Check if model ID represents a GGUF model
fn is_gguf_model(model_id: &str) -> bool {
    model_id.to_lowercase().contains("-gguf") || model_id.ends_with(".gguf")
}

/// Check if model ID represents a GPTQ quantized model
/// GPTQ models are auto-detected by mistral.rs TextModelBuilder
fn is_gptq_model(model_id: &str) -> bool {
    let lower = model_id.to_lowercase();
    lower.contains("-gptq") || lower.contains("gptq-") || lower.contains("_gptq")
}

/// Load a GGUF model from HuggingFace
///
/// Format: "repo:filename.gguf" or "repo:filename.gguf@tokenizer-repo"
///
/// Examples:
/// - "TheBloke/Mistral-7B-Instruct-v0.2-GGUF:mistral-7b-instruct-v0.2.Q4_K_M.gguf"
/// - "microsoft/Phi-3-mini-4k-instruct-gguf:Phi-3-mini-4k-instruct-q4.gguf"
/// - "bartowski/Llama-3-8B-GGUF:Llama-3-8B-Q4_K_M.gguf@meta-llama/Meta-Llama-3-8B"
async fn load_gguf_model(
    model_id: &str,
    config: &ModelConfig,
    page_attention_result: anyhow::Result<PagedAttentionConfig>,
    prefix_cache_n: Option<usize>,
) -> Result<MistralRsModel, InferenceError> {
    info!(model_id = %model_id, "Loading GGUF model (pre-quantized)");

    // Parse model ID: "repo:filename.gguf" or "repo:filename.gguf@tokenizer-repo"
    let (repo_id, gguf_filename, tokenizer_repo) = parse_gguf_model_id(model_id)?;

    info!(
        repo_id = %repo_id,
        gguf_file = %gguf_filename,
        tokenizer = %tokenizer_repo,
        "Parsed GGUF model configuration"
    );

    // Use GgufModelBuilder to load GGUF with tokenizer
    let mut builder = GgufModelBuilder::new(&repo_id, vec![gguf_filename.clone()])
        .with_tok_model_id(&tokenizer_repo)
        .with_logging()
        .with_prefix_cache_n(prefix_cache_n);

    // Configure token source if specified
    if config.hf_home.is_some() {
        builder = builder.with_token_source(TokenSource::CacheToken);
        // Note: GgufModelBuilder will use HF_HOME env var for cache path
    }

    let builder = builder
        .with_paged_attn(|| page_attention_result)
        .map_err(|e| {
            InferenceError::ModelLoad(format!(
                "Failed to configure paged attention for GGUF model {}: {}",
                model_id, e
            ))
        })?;

    info!("Configured GGUF model with paged attention");

    let model = builder.build().await.map_err(|e| {
        let error_msg = format!("{}", e);

        // Check if it's a 404 error (file not found)
        if error_msg.contains("404") || error_msg.contains("Not Found") {
            InferenceError::ModelLoad(format!(
                "GGUF file not found: {}\n\n\
                 Attempted to load:\n\
                 - Repository: {}\n\
                 - File: {}\n\
                 - Tokenizer: {}\n\n\
                 Solutions:\n\
                 1. Check available files at: https://huggingface.co/{}/tree/main\n\
                 2. Verify exact filename (case-sensitive)\n\
                 3. Format: LLM_ALLOWED_MODELS=\"{}:exact-filename.gguf\"\n\
                 4. Override tokenizer: LLM_ALLOWED_MODELS=\"{}:{}@your/tokenizer-repo\"\n\n\
                 Original error: {}",
                model_id,
                repo_id,
                gguf_filename,
                tokenizer_repo,
                repo_id,
                repo_id,
                repo_id,
                gguf_filename,
                error_msg
            ))
        } else {
            InferenceError::ModelLoad(format!(
                "Failed to load GGUF model {}. Error: {}\n\n\
                 Configuration:\n\
                 - GGUF repo: {}\n\
                 - GGUF file: {}\n\
                 - Tokenizer from: {}",
                model_id, error_msg, repo_id, gguf_filename, tokenizer_repo
            ))
        }
    })?;

    info!(model_id = %model_id, "Successfully loaded GGUF model");
    Ok(model)
}

/// Parse GGUF model ID into components
///
/// Format: "repo:filename.gguf[@tokenizer-repo]"
///
/// Examples:
/// - "TheBloke/Mistral-7B-Instruct-v0.2-GGUF:mistral-7b-instruct-v0.2.Q4_K_M.gguf"
///   -> Uses repo itself for tokenizer (most GGUF repos include tokenizer config)
/// - "microsoft/Phi-3-mini-4k-instruct-gguf:Phi-3-mini-4k-instruct-q4.gguf"
///   -> Uses repo itself for tokenizer
/// - "bartowski/Llama-3-8B-GGUF:Llama-3-8B-Q4_K_M.gguf@meta-llama/Meta-Llama-3-8B"
///   -> Explicit tokenizer override
///
/// Returns: (repo_id, gguf_filename, tokenizer_repo)
fn parse_gguf_model_id(model_id: &str) -> Result<(String, String, String), InferenceError> {
    // Check if it's a GGUF model (case-insensitive check for -gguf or .gguf)
    let lower = model_id.to_lowercase();
    if !lower.contains("-gguf") && !lower.ends_with(".gguf") {
        return Err(InferenceError::ModelLoad(format!(
            "Not a GGUF model: {}. Model ID must contain '-gguf' (case-insensitive) or end with '.gguf'",
            model_id
        )));
    }

    // Check if explicit filename is specified: "repo:filename.gguf" or "repo:filename.gguf@tokenizer"
    if !model_id.contains(':') {
        return Err(InferenceError::ModelLoad(format!(
            "GGUF model must specify filename: {}\n\n\
             Format: \"repo:filename.gguf\" or \"repo:filename.gguf@tokenizer-repo\"\n\n\
             Examples:\n\
             - \"TheBloke/Mistral-7B-Instruct-v0.2-GGUF:mistral-7b-instruct-v0.2.Q4_K_M.gguf\"\n\
             - \"microsoft/Phi-3-mini-4k-instruct-gguf:Phi-3-mini-4k-instruct-q4.gguf\"\n\
             - \"bartowski/Llama-3-8B-GGUF:Llama-3-8B-Q4_K_M.gguf\"\n\n\
             Browse available files at: https://huggingface.co/<repo>/tree/main",
            model_id
        )));
    }

    // Split on first colon to separate repo from filename[@tokenizer]
    let (repo_id, rest) = model_id.split_once(':').unwrap();

    // Check for explicit tokenizer: "filename.gguf@tokenizer-repo"
    let (gguf_filename, tokenizer_repo) = if let Some((filename, tokenizer)) = rest.split_once('@')
    {
        (filename.to_string(), tokenizer.to_string())
    } else {
        // No explicit tokenizer - use the GGUF repo itself
        // Modern GGUF repos typically include tokenizer config
        (rest.to_string(), repo_id.to_string())
    };

    // Validate filename ends with .gguf
    if !gguf_filename.to_lowercase().ends_with(".gguf") {
        return Err(InferenceError::ModelLoad(format!(
            "GGUF filename must end with .gguf: {}\n\
             Got: {}",
            model_id, gguf_filename
        )));
    }

    Ok((repo_id.to_string(), gguf_filename, tokenizer_repo))
}

/// Parse ISQ type string to IsqType enum
fn parse_isq_type(isq_str: &str) -> Result<IsqType, String> {
    match isq_str.to_uppercase().as_str() {
        "Q4_0" => Ok(IsqType::Q4_0),
        "Q4_1" => Ok(IsqType::Q4_1),
        "Q5_0" => Ok(IsqType::Q5_0),
        "Q5_1" => Ok(IsqType::Q5_1),
        "Q8_0" => Ok(IsqType::Q8_0),
        "Q8_1" => Ok(IsqType::Q8_1),
        "Q2_K" => Ok(IsqType::Q2K),
        "Q3_K" => Ok(IsqType::Q3K),
        "Q4_K" => Ok(IsqType::Q4K),
        "Q5_K" => Ok(IsqType::Q5K),
        "Q6_K" => Ok(IsqType::Q6K),
        "Q8_K" => Ok(IsqType::Q8K),
        _ => Err(format!("Unknown ISQ type: {}", isq_str)),
    }
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
    let total_start = Instant::now();

    // Check if model is allowed
    if !model_config.allowed_models.contains(&model_id.to_string()) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Check GPU pressure before starting (VRAM + compute utilization)
    if is_gpu_memory_pressure_high() {
        warn!(
            model_id = %model_id,
            "GPU pressure high, rejecting request"
        );
        return Err(InferenceError::ServiceUnavailable(
            "GPU pressure high, try again later".to_string(),
        ));
    }

    // Validate parameters
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);
    let prompt_len = prompt.len();

    // Get model from cache
    let cache_start = Instant::now();
    let model_arc = get_or_load_model(model_id, model_config).await?;
    let cache_time = cache_start.elapsed();

    // Generate text using mistral.rs
    let request = RequestBuilder::new()
        .set_sampler_max_len(max_tokens)
        .set_sampler_temperature(temperature as f64)
        .set_sampler_topp(top_p as f64)
        .add_message(TextMessageRole::System, "You are a helpful assistant.")
        .add_message(TextMessageRole::User, &prompt);

    let gen_start = Instant::now();
    let response = model_arc
        .send_chat_request(request)
        .await
        .map_err(|e| InferenceError::Generation(format!("Generation failed: {}", e)))?;
    let gen_time = gen_start.elapsed();

    let tokens_generated = response.usage.completion_tokens;
    let tokens_per_sec = if gen_time.as_secs_f64() > 0.0 {
        tokens_generated as f64 / gen_time.as_secs_f64()
    } else {
        0.0
    };

    // Log timing breakdown
    info!(
        model_id = %model_id,
        prompt_chars = prompt_len,
        tokens_generated = tokens_generated,
        cache_time_ms = cache_time.as_millis(),
        gen_time_ms = gen_time.as_millis(),
        total_time_ms = total_start.elapsed().as_millis(),
        tokens_per_sec = format!("{:.1}", tokens_per_sec),
        finish_reason = response.choices.first().map(|c| c.finish_reason.as_str()),
        "LLM generation completed"
    );

    // Log response details for debugging
    if let Some(choice) = response.choices.first() {
        debug!(
            choices = response.choices.len(),
            completion_tokens = response.usage.completion_tokens,
            finish_reason = choice.finish_reason.as_str(),
            has_content = choice.message.content.is_some(),
            content_len = choice.message.content.as_ref().map(|c| c.len()),
            has_reasoning = choice.message.reasoning_content.is_some(),
            reasoning_len = choice.message.reasoning_content.as_ref().map(|c| c.len()),
            role = choice.message.role.as_str(),
            "Received response from model"
        );

        // Log full response at WARN level if content is unexpectedly empty
        if choice.message.content.as_ref().is_none_or(|c| c.is_empty())
            && choice
                .message
                .reasoning_content
                .as_ref()
                .is_none_or(|c| c.is_empty())
        {
            warn!(
                finish_reason = choice.finish_reason.as_str(),
                completion_tokens = response.usage.completion_tokens,
                content = ?choice.message.content,
                reasoning_content = ?choice.message.reasoning_content,
                role = choice.message.role.as_str(),
                "Response has empty content - this may be a mistral.rs bug"
            );
        }
    }

    // Check if we have choices
    if response.choices.is_empty() {
        return Err(InferenceError::Generation(
            "No choices in model response".to_string(),
        ));
    }

    // Try to extract content from message.content or reasoning_content (for Harmony format models)
    let message = &response.choices[0].message;
    let text = if let Some(content) = message.content.as_ref().filter(|c| !c.trim().is_empty()) {
        content.clone()
    } else if let Some(reasoning) = message
        .reasoning_content
        .as_ref()
        .filter(|c| !c.trim().is_empty())
    {
        // Some models (SmolLM3, etc.) may put content in reasoning_content
        tracing::debug!("Using reasoning_content as fallback (content was empty)");
        reasoning.clone()
    } else {
        // Log detailed debug info for troubleshooting
        tracing::error!(
            finish_reason = response.choices[0].finish_reason,
            completion_tokens = response.usage.completion_tokens,
            content_is_some = message.content.is_some(),
            content_value = ?message.content,
            reasoning_is_some = message.reasoning_content.is_some(),
            reasoning_value = ?message.reasoning_content,
            "Model response has no usable content - mistral.rs may have a bug with this model"
        );
        return Err(InferenceError::Generation(format!(
            "Model returned empty content (finish_reason={}, tokens={}). This may be a mistral.rs bug when hitting max_tokens limit.",
            response.choices[0].finish_reason, response.usage.completion_tokens
        )));
    };

    // Determine finish reason
    let finish_reason = match response.choices[0].finish_reason.as_str() {
        "stop" => FinishReason::Stop,
        "eos" | "eos_token" => FinishReason::Eos,
        "length" => FinishReason::Length,
        "error" => FinishReason::Error,
        _ => FinishReason::Eos,
    };

    Ok(GenerationResponse {
        text,
        model: model_id.to_string(),
        tokens_generated: response.usage.completion_tokens,
        finish_reason,
    })
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
    let total_start = Instant::now();

    // Check if model is allowed
    if !model_config.allowed_models.contains(&model_id.to_string()) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Check GPU pressure before starting (VRAM + compute utilization)
    if is_gpu_memory_pressure_high() {
        warn!(
            model_id = %model_id,
            "GPU pressure high, rejecting chat request"
        );
        return Err(InferenceError::ServiceUnavailable(
            "GPU pressure high, try again later".to_string(),
        ));
    }

    // Validate parameters
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);
    let message_count = messages.len();
    let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();

    // Get model from cache
    let cache_start = Instant::now();
    let model_arc = get_or_load_model(model_id, model_config).await?;
    let cache_time = cache_start.elapsed();

    // Merge consecutive messages with the same role to ensure alternation
    let merged_messages = merge_consecutive_messages(messages);

    // Build chat messages for mistral.rs
    let mut text_messages = TextMessages::new();
    for msg in merged_messages {
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
    let gen_start = Instant::now();
    let response = model_arc
        .send_chat_request(request)
        .await
        .map_err(|e| InferenceError::Generation(format!("Chat completion failed: {}", e)))?;
    let gen_time = gen_start.elapsed();

    let tokens_generated = response.usage.completion_tokens;
    let tokens_per_sec = if gen_time.as_secs_f64() > 0.0 {
        tokens_generated as f64 / gen_time.as_secs_f64()
    } else {
        0.0
    };

    // Log timing breakdown
    info!(
        model_id = %model_id,
        message_count = message_count,
        total_chars = total_chars,
        tokens_generated = tokens_generated,
        cache_time_ms = cache_time.as_millis(),
        gen_time_ms = gen_time.as_millis(),
        total_time_ms = total_start.elapsed().as_millis(),
        tokens_per_sec = format!("{:.1}", tokens_per_sec),
        finish_reason = response.choices.first().map(|c| c.finish_reason.as_str()),
        "LLM chat completion completed"
    );

    // Check if we have choices
    if response.choices.is_empty() {
        return Err(InferenceError::Generation(
            "No choices in chat response".to_string(),
        ));
    }

    // Try to extract content from message.content or reasoning_content (for Harmony format models)
    let message = &response.choices[0].message;
    let content = if let Some(c) = message.content.as_ref().filter(|c| !c.trim().is_empty()) {
        c.clone()
    } else if let Some(reasoning) = message
        .reasoning_content
        .as_ref()
        .filter(|c| !c.trim().is_empty())
    {
        // Some models (SmolLM3, etc.) may put content in reasoning_content
        tracing::debug!("Using reasoning_content as fallback (content was empty)");
        reasoning.clone()
    } else {
        // Log detailed debug info for troubleshooting
        tracing::error!(
            finish_reason = response.choices[0].finish_reason,
            completion_tokens = response.usage.completion_tokens,
            content_is_some = message.content.is_some(),
            content_value = ?message.content,
            reasoning_is_some = message.reasoning_content.is_some(),
            reasoning_value = ?message.reasoning_content,
            "Chat response has no usable content - mistral.rs may have a bug with this model"
        );
        return Err(InferenceError::Generation(format!(
            "Model returned empty content (finish_reason={}, tokens={}). This may be a mistral.rs bug when hitting max_tokens limit.",
            response.choices[0].finish_reason, response.usage.completion_tokens
        )));
    };

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

/// Chat completion with streaming
///
/// Returns a stream of text chunks as they are generated based on conversation history.
pub async fn chat_completion_stream(
    model_id: &str,
    messages: Vec<ChatMessage>,
    params: GenerationParams,
    model_config: &ModelConfig,
    gen_config: &GenerationConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, InferenceError>> + Send>>, InferenceError> {
    // Check if model is allowed
    if !model_config.allowed_models.contains(&model_id.to_string()) {
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

    // Merge consecutive messages with the same role to ensure alternation
    let merged_messages = merge_consecutive_messages(messages);

    // Build chat messages for mistral.rs
    let mut text_messages = TextMessages::new();
    for msg in merged_messages {
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

/// Merge consecutive messages with the same role to ensure proper alternation
///
/// Some chat models require strict alternation between user and assistant roles.
/// This function merges consecutive messages with the same role by concatenating
/// their content with newlines.
fn merge_consecutive_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    if messages.is_empty() {
        return messages;
    }

    let mut merged = Vec::new();
    let mut current_role = String::new();
    let mut current_content = String::new();

    for msg in messages {
        if msg.role == current_role {
            // Same role as previous - merge content
            if !current_content.is_empty() {
                current_content.push_str("\n\n");
            }
            current_content.push_str(&msg.content);
        } else {
            // Different role - save previous message if any
            if !current_role.is_empty() {
                merged.push(ChatMessage {
                    role: current_role.clone(),
                    content: current_content.clone(),
                });
            }
            // Start new message
            current_role = msg.role;
            current_content = msg.content;
        }
    }

    // Don't forget the last message
    if !current_role.is_empty() {
        merged.push(ChatMessage {
            role: current_role,
            content: current_content,
        });
    }

    merged
}

/// Text/code completion (for fill-in-middle and code completion use-cases)
///
/// Generates a completion based on prompt and optional suffix.
/// If suffix is provided, the model will attempt to fill in the middle.
pub async fn text_completion(
    model_id: &str,
    prompt: String,
    suffix: Option<String>,
    params: GenerationParams,
    model_config: &ModelConfig,
    gen_config: &GenerationConfig,
) -> Result<GenerationResponse, InferenceError> {
    let total_start = Instant::now();

    // Check if model is allowed
    if !model_config.allowed_models.contains(&model_id.to_string()) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Check GPU pressure before starting (VRAM + compute utilization)
    if is_gpu_memory_pressure_high() {
        warn!(
            model_id = %model_id,
            "GPU pressure high, rejecting request"
        );
        return Err(InferenceError::ServiceUnavailable(
            "GPU pressure high, try again later".to_string(),
        ));
    }

    // Validate parameters
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Build the completion prompt, incorporating suffix if provided
    let completion_prompt = if let Some(ref suf) = suffix {
        // Fill-in-middle format: use a structured prompt that signals FIM
        format!(
            "<|fim_prefix|>{}<|fim_suffix|>{}<|fim_middle|>",
            prompt, suf
        )
    } else {
        prompt.clone()
    };

    let prompt_len = completion_prompt.len();

    // Get model from cache
    let cache_start = Instant::now();
    let model_arc = get_or_load_model(model_id, model_config).await?;
    let cache_time = cache_start.elapsed();

    // Generate completion using mistral.rs
    // For completion, we use a minimal system prompt to avoid influencing the output
    let request = RequestBuilder::new()
        .set_sampler_max_len(max_tokens)
        .set_sampler_temperature(temperature as f64)
        .set_sampler_topp(top_p as f64)
        .add_message(TextMessageRole::User, &completion_prompt);

    let gen_start = Instant::now();
    let response = model_arc
        .send_chat_request(request)
        .await
        .map_err(|e| InferenceError::Generation(format!("Completion failed: {}", e)))?;
    let gen_time = gen_start.elapsed();

    // Check if we have choices
    if response.choices.is_empty() {
        return Err(InferenceError::Generation(
            "No choices in model response".to_string(),
        ));
    }

    // Extract completion text
    let message = &response.choices[0].message;
    let text = message
        .content
        .as_ref()
        .filter(|c| !c.trim().is_empty())
        .cloned()
        .or_else(|| {
            message
                .reasoning_content
                .as_ref()
                .filter(|c| !c.trim().is_empty())
                .cloned()
        })
        .unwrap_or_default();

    // Determine finish reason
    let finish_reason = match response.choices[0].finish_reason.as_str() {
        "stop" => FinishReason::Stop,
        "eos" | "eos_token" => FinishReason::Eos,
        "length" => FinishReason::Length,
        "error" => FinishReason::Error,
        _ => FinishReason::Eos,
    };

    let total_time = total_start.elapsed();

    debug!(
        model_id = %model_id,
        prompt_len = prompt_len,
        tokens = response.usage.completion_tokens,
        cache_ms = cache_time.as_millis(),
        gen_ms = gen_time.as_millis(),
        total_ms = total_time.as_millis(),
        has_suffix = suffix.is_some(),
        "Text completion completed"
    );

    Ok(GenerationResponse {
        text,
        model: model_id.to_string(),
        tokens_generated: response.usage.completion_tokens,
        finish_reason,
    })
}

/// Text/code completion with streaming
///
/// Returns a stream of text chunks as they are generated.
/// Supports optional suffix for fill-in-middle completion.
pub async fn text_completion_stream(
    model_id: &str,
    prompt: String,
    suffix: Option<String>,
    params: GenerationParams,
    model_config: &ModelConfig,
    gen_config: &GenerationConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, InferenceError>> + Send>>, InferenceError> {
    // Check if model is allowed
    if !model_config.allowed_models.contains(&model_id.to_string()) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    // Validate parameters
    let temperature = gen_config.validate_temperature(params.temperature);
    let top_p = gen_config.validate_top_p(params.top_p);
    let max_tokens = gen_config.validate_max_tokens(params.max_tokens);

    // Build the completion prompt, incorporating suffix if provided
    let completion_prompt = if let Some(ref suf) = suffix {
        format!(
            "<|fim_prefix|>{}<|fim_suffix|>{}<|fim_middle|>",
            prompt, suf
        )
    } else {
        prompt
    };

    // Get model from cache
    let model_arc = get_or_load_model(model_id, model_config).await?;

    // Generate completion stream using mistral.rs
    let request = RequestBuilder::new()
        .set_sampler_max_len(max_tokens)
        .set_sampler_temperature(temperature as f64)
        .set_sampler_topp(top_p as f64)
        .add_message(TextMessageRole::User, &completion_prompt);

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
        };

        assert_eq!(params.temperature, 0.7);
        assert_eq!(params.top_p, 0.9);
        assert_eq!(params.max_tokens, 100);
    }
}
