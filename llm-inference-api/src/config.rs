//! Configuration for the LLM inference API service.
//!
//! All configuration is loaded from environment variables at startup.
//! Supports airgapped deployments with HF_HOME and HF_ENDPOINT configuration.
//! Supports TLS/SSL for secure deployments using shared core configuration.

use anyhow::{Context, Result};
use semantic_explorer_core::config::TlsConfig;
use std::env;
use std::path::PathBuf;

/// LLM Inference API configuration
#[derive(Debug, Clone)]
pub struct LlmInferenceConfig {
    pub server: ServerConfig,
    pub models: ModelConfig,
    pub generation: GenerationConfig,
    pub observability: ObservabilityConfig,
    pub tls: TlsConfig,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub hostname: String,
    pub port: u16,
    pub cors_allowed_origins: Vec<String>,
}

/// Model loading and caching configuration
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// HuggingFace cache directory (HF_HOME)
    pub hf_home: Option<PathBuf>,
    /// HuggingFace endpoint URL for downloading models (HF_ENDPOINT)
    /// Use this to point to an Artifactory proxy or local mirror
    pub hf_endpoint: Option<String>,
    /// Custom model directory for user-provided models
    pub model_path: Option<PathBuf>,
    /// List of allowed LLM models
    pub allowed_models: Vec<String>,
    /// Maximum number of concurrent inference requests
    pub max_concurrent_requests: usize,
    /// Queue timeout in milliseconds - how long to wait for a permit before returning 503
    pub queue_timeout_ms: u64,
    /// Enable ISQ (In-situ Quantization) for regular HF models
    /// Note: This is slow on first load. Prefer pre-quantized GGUF models.
    pub enable_isq: bool,
    /// ISQ quantization type (Q4_K, Q8_0, etc.)
    pub isq_type: Option<String>,
    /// Paged attention block size (default: 32)
    pub paged_attention_block_size: usize,
    /// Paged attention GPU memory context size (default: 1024)
    pub paged_attention_context_size: usize,
    /// Paged attention cache type: "auto" (native dtype) or "f8e4m3" (FP8 KV cache)
    /// FP8 KV cache reduces memory usage and improves performance on Hopper+ GPUs (H100/H200)
    pub paged_cache_type: PagedCacheType,
    /// Enable prefix caching for multi-turn conversations and RAG workloads
    /// Significantly accelerates repeated prompts by reusing KV cache for shared prefixes
    pub enable_prefix_caching: bool,
    /// GPU pressure threshold percentage — reject requests above this % VRAM or compute utilization
    pub gpu_pressure_threshold: f64,
}

/// Text generation configuration
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    /// Default temperature for generation (0.0 - 2.0)
    pub default_temperature: f32,
    /// Default top_p for nucleus sampling (0.0 - 1.0)
    pub default_top_p: f32,
    /// Default maximum tokens to generate
    pub default_max_tokens: usize,
    /// Hard limit on maximum tokens (safety)
    pub max_tokens_limit: usize,
}

/// Observability configuration
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub service_name: String,
    pub otlp_endpoint: String,
    pub log_format: LogFormat,
}

/// Log format type
#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    Json,
    Pretty,
}

/// Paged attention cache type for KV cache storage
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PagedCacheType {
    /// Use native dtype (fp16/bf16) - compatible with all GPUs
    #[default]
    Auto,
    /// Use FP8 E4M3 format - reduces memory ~50%, faster on Hopper+ (H100/H200)
    F8E4M3,
}

impl LlmInferenceConfig {
    /// Load configuration from environment variables.
    ///
    /// This should be called once at application startup.
    pub fn from_env() -> Result<Self> {
        let config = Self {
            server: ServerConfig::from_env()?,
            models: ModelConfig::from_env()?,
            generation: GenerationConfig::from_env()?,
            observability: ObservabilityConfig::from_env()?,
            tls: TlsConfig::from_env()?,
        };

        // Log configuration for airgapped deployments
        config.log_environment_config();

        Ok(config)
    }

    /// Log environment configuration for debugging airgapped deployments
    fn log_environment_config(&self) {
        // Log HF_HOME if configured
        if let Some(ref hf_home) = self.models.hf_home {
            tracing::info!(hf_home = %hf_home.display(), "Using custom HF_HOME cache directory");
        }

        // Log HF_ENDPOINT if configured (for Artifactory/mirror proxies)
        if let Some(ref hf_endpoint) = self.models.hf_endpoint {
            tracing::info!(hf_endpoint = %hf_endpoint, "Using custom HF_ENDPOINT for model downloads");
        }

        // Log custom model path if configured
        if let Some(ref model_path) = self.models.model_path {
            if model_path.exists() {
                tracing::info!(model_path = %model_path.display(), "Custom model path configured and exists");
            } else {
                tracing::warn!(model_path = %model_path.display(), "Custom model path configured but does not exist");
            }
        }

        // Log allowed models (always required now)
        tracing::info!(
            allowed_models = ?self.models.allowed_models,
            count = self.models.allowed_models.len(),
            "Allowed models configured"
        );
    }
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            hostname: env::var("LLM_INFERENCE_HOSTNAME").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("LLM_INFERENCE_PORT")
                .unwrap_or_else(|_| "8091".to_string())
                .parse()
                .context("LLM_INFERENCE_PORT must be a number")?,
            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "*".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        })
    }
}

impl ModelConfig {
    pub fn from_env() -> Result<Self> {
        // LLM_ALLOWED_MODELS is required
        let allowed_models_raw = env::var("LLM_ALLOWED_MODELS")
            .context("LLM_ALLOWED_MODELS is required (comma-separated list of model IDs)")?;

        let allowed_models: Vec<String> = allowed_models_raw
            .split(',')
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .collect();

        if allowed_models.is_empty() {
            anyhow::bail!("LLM_ALLOWED_MODELS must contain at least one model");
        }

        Ok(Self {
            hf_home: env::var("HF_HOME").ok().map(PathBuf::from),
            hf_endpoint: env::var("HF_ENDPOINT").ok(),
            model_path: env::var("LLM_MODEL_PATH").ok().map(PathBuf::from),
            allowed_models,
            max_concurrent_requests: env::var("LLM_MAX_CONCURRENT_REQUESTS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("LLM_MAX_CONCURRENT_REQUESTS must be a number")?,
            queue_timeout_ms: env::var("LLM_QUEUE_TIMEOUT_MS")
                .unwrap_or_else(|_| "30000".to_string())
                .parse()
                .context("LLM_QUEUE_TIMEOUT_MS must be a number")?,
            enable_isq: env::var("LLM_ENABLE_ISQ")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .context("LLM_ENABLE_ISQ must be true or false")?,
            isq_type: env::var("LLM_ISQ_TYPE").ok(),
            paged_attention_block_size: env::var("LLM_PAGED_ATTENTION_BLOCK_SIZE")
                .unwrap_or_else(|_| "32".to_string())
                .parse()
                .context("LLM_PAGED_ATTENTION_BLOCK_SIZE must be a number")?,
            paged_attention_context_size: env::var("LLM_PAGED_ATTENTION_CONTEXT_SIZE")
                .unwrap_or_else(|_| "1024".to_string())
                .parse()
                .context("LLM_PAGED_ATTENTION_CONTEXT_SIZE must be a number")?,
            paged_cache_type: match env::var("LLM_PAGED_CACHE_TYPE")
                .unwrap_or_else(|_| "auto".to_string())
                .to_lowercase()
                .as_str()
            {
                "f8e4m3" | "fp8" => PagedCacheType::F8E4M3,
                _ => PagedCacheType::Auto,
            },
            enable_prefix_caching: env::var("LLM_ENABLE_PREFIX_CACHING")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .context("LLM_ENABLE_PREFIX_CACHING must be true or false")?,
            gpu_pressure_threshold: env::var("GPU_PRESSURE_THRESHOLD")
                .unwrap_or_else(|_| "95.0".to_string())
                .parse()
                .context("GPU_PRESSURE_THRESHOLD must be a number")?,
        })
    }
}

impl GenerationConfig {
    pub fn from_env() -> Result<Self> {
        let default_temperature = env::var("LLM_DEFAULT_TEMPERATURE")
            .unwrap_or_else(|_| "0.7".to_string())
            .parse()
            .context("LLM_DEFAULT_TEMPERATURE must be a number")?;

        let default_top_p = env::var("LLM_DEFAULT_TOP_P")
            .unwrap_or_else(|_| "0.9".to_string())
            .parse()
            .context("LLM_DEFAULT_TOP_P must be a number")?;

        let default_max_tokens = env::var("LLM_DEFAULT_MAX_TOKENS")
            .unwrap_or_else(|_| "512".to_string())
            .parse()
            .context("LLM_DEFAULT_MAX_TOKENS must be a number")?;

        let max_tokens_limit = env::var("LLM_MAX_TOKENS_LIMIT")
            .unwrap_or_else(|_| "4096".to_string())
            .parse()
            .context("LLM_MAX_TOKENS_LIMIT must be a number")?;

        Ok(Self {
            default_temperature,
            default_top_p,
            default_max_tokens,
            max_tokens_limit,
        })
    }

    /// Validate and clamp temperature to valid range
    pub fn validate_temperature(&self, temperature: f32) -> f32 {
        temperature.clamp(0.0, 2.0)
    }

    /// Validate and clamp top_p to valid range
    pub fn validate_top_p(&self, top_p: f32) -> f32 {
        top_p.clamp(0.0, 1.0)
    }

    /// Validate and clamp max_tokens to limit
    pub fn validate_max_tokens(&self, max_tokens: usize) -> usize {
        max_tokens.min(self.max_tokens_limit)
    }
}

impl ObservabilityConfig {
    pub fn from_env() -> Result<Self> {
        let log_format = match env::var("LOG_FORMAT")
            .unwrap_or_else(|_| "json".to_string())
            .to_lowercase()
            .as_str()
        {
            "pretty" => LogFormat::Pretty,
            _ => LogFormat::Json,
        };

        Ok(Self {
            service_name: env::var("SERVICE_NAME")
                .unwrap_or_else(|_| "llm-inference-api".to_string()),
            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            log_format,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_fields() {
        let model = ModelConfig {
            hf_home: Some(PathBuf::from("/tmp/hf_cache")),
            hf_endpoint: Some("https://hf-mirror.example.com".to_string()),
            model_path: Some(PathBuf::from("/models/custom")),
            allowed_models: vec![
                "mistralai/Mistral-7B-Instruct-v0.2".to_string(),
                "meta-llama/Llama-2-7b-chat-hf".to_string(),
            ],
            max_concurrent_requests: 10,
            enable_isq: false,
            isq_type: None,
            paged_attention_block_size: 32,
            paged_attention_context_size: 1024,
            queue_timeout_ms: 30000,
            paged_cache_type: PagedCacheType::Auto,
            enable_prefix_caching: false,
            gpu_pressure_threshold: 95.0,
        };

        assert_eq!(model.allowed_models.len(), 2);
        assert_eq!(model.hf_home, Some(PathBuf::from("/tmp/hf_cache")));
    }

    #[test]
    fn test_model_filtering() {
        // Specific allowed list
        let config_restricted = ModelConfig {
            hf_home: None,
            hf_endpoint: None,
            model_path: None,
            allowed_models: vec!["mistralai/Mistral-7B-Instruct-v0.2".to_string()],
            max_concurrent_requests: 10,
            enable_isq: false,
            isq_type: None,
            paged_attention_block_size: 32,
            paged_attention_context_size: 1024,
            queue_timeout_ms: 30000,
            paged_cache_type: PagedCacheType::F8E4M3,
            enable_prefix_caching: true,
            gpu_pressure_threshold: 95.0,
        };
        assert_eq!(config_restricted.allowed_models.len(), 1);
        assert!(
            config_restricted
                .allowed_models
                .contains(&"mistralai/Mistral-7B-Instruct-v0.2".to_string())
        );
    }

    #[test]
    fn test_generation_config_validation() {
        let config = GenerationConfig {
            default_temperature: 0.7,
            default_top_p: 0.9,
            default_max_tokens: 512,
            max_tokens_limit: 4096,
        };

        // Test temperature clamping
        assert_eq!(config.validate_temperature(0.5), 0.5);
        assert_eq!(config.validate_temperature(-1.0), 0.0);
        assert_eq!(config.validate_temperature(3.0), 2.0);

        // Test top_p clamping
        assert_eq!(config.validate_top_p(0.9), 0.9);
        assert_eq!(config.validate_top_p(-0.1), 0.0);
        assert_eq!(config.validate_top_p(1.5), 1.0);

        // Test max_tokens clamping
        assert_eq!(config.validate_max_tokens(100), 100);
        assert_eq!(config.validate_max_tokens(5000), 4096);
    }
}
