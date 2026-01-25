//! Configuration for the inference API service.
//!
//! All configuration is loaded from environment variables at startup.
//! Supports airgapped deployments with HF_HOME and HF_ENDPOINT configuration.
//! Supports TLS/SSL for secure deployments using shared core configuration.

use anyhow::{Context, Result};
use semantic_explorer_core::config::TlsConfig;
use std::env;
use std::path::PathBuf;

/// Inference API configuration
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    pub server: ServerConfig,
    pub models: ModelConfig,
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
    /// Custom model directory for user-provided ONNX models
    pub model_path: Option<PathBuf>,
    /// Allowed embedding models configuration
    /// If all_embedding_models is true, all models are allowed
    /// Otherwise, only models in allowed_embedding_models are allowed
    pub all_embedding_models: bool,
    pub allowed_embedding_models: Vec<String>,
    /// Allowed rerank models configuration
    /// If all_rerank_models is true, all models are allowed
    /// If both are false/empty, no rerankers are loaded
    pub all_rerank_models: bool,
    pub allowed_rerank_models: Vec<String>,
    /// Maximum batch size for embedding requests
    pub max_batch_size: usize,
    /// Maximum concurrent embedding requests (for backpressure)
    pub max_concurrent_requests: usize,
    /// Queue timeout in milliseconds - how long to wait for a permit before 503
    /// Setting this higher allows requests to queue briefly instead of immediate rejection
    pub queue_timeout_ms: u64,
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

impl InferenceConfig {
    /// Load configuration from environment variables.
    ///
    /// This should be called once at application startup.
    pub fn from_env() -> Result<Self> {
        let config = Self {
            server: ServerConfig::from_env()?,
            models: ModelConfig::from_env()?,
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
    }
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            hostname: env::var("INFERENCE_HOSTNAME").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("INFERENCE_PORT")
                .unwrap_or_else(|_| "8090".to_string())
                .parse()
                .context("INFERENCE_PORT must be a number")?,
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
        let embedding_models_raw = env::var("INFERENCE_ALLOWED_EMBEDDING_MODELS")
            .context("INFERENCE_ALLOWED_EMBEDDING_MODELS is required. Set to '*' for all models or a comma-separated list.")?;

        let (all_embedding_models, allowed_embedding_models) = if embedding_models_raw.trim() == "*"
        {
            (true, Vec::new())
        } else {
            let models: Vec<String> = embedding_models_raw
                .split(',')
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect();
            if models.is_empty() {
                anyhow::bail!(
                    "INFERENCE_ALLOWED_EMBEDDING_MODELS must contain at least one model or '*' for all"
                );
            }
            (false, models)
        };

        // INFERENCE_ALLOWED_RERANK_MODELS is optional - empty means no rerankers
        let (all_rerank_models, allowed_rerank_models) =
            match env::var("INFERENCE_ALLOWED_RERANK_MODELS") {
                Ok(val) if val.trim() == "*" => (true, Vec::new()),
                Ok(val) if !val.trim().is_empty() => {
                    let models: Vec<String> = val
                        .split(',')
                        .map(|m| m.trim().to_string())
                        .filter(|m| !m.is_empty())
                        .collect();
                    (false, models)
                }
                _ => (false, Vec::new()), // No rerankers configured
            };

        Ok(Self {
            hf_home: env::var("HF_HOME").ok().map(PathBuf::from),
            hf_endpoint: env::var("HF_ENDPOINT").ok(),
            model_path: env::var("INFERENCE_MODEL_PATH").ok().map(PathBuf::from),
            all_embedding_models,
            allowed_embedding_models,
            all_rerank_models,
            allowed_rerank_models,
            max_batch_size: env::var("INFERENCE_MAX_BATCH_SIZE")
                .unwrap_or_else(|_| "256".to_string())
                .parse()
                .context("INFERENCE_MAX_BATCH_SIZE must be a number")?,
            max_concurrent_requests: env::var("INFERENCE_MAX_CONCURRENT_REQUESTS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .context("INFERENCE_MAX_CONCURRENT_REQUESTS must be a number")?,
            queue_timeout_ms: env::var("INFERENCE_QUEUE_TIMEOUT_MS")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .context("INFERENCE_QUEUE_TIMEOUT_MS must be a number")?,
        })
    }

    /// Check if an embedding model is allowed based on configuration
    pub fn is_embedding_model_allowed(&self, model_id: &str) -> bool {
        self.all_embedding_models
            || self
                .allowed_embedding_models
                .contains(&model_id.to_string())
    }

    /// Check if a rerank model is allowed based on configuration
    pub fn is_rerank_model_allowed(&self, model_id: &str) -> bool {
        self.all_rerank_models || self.allowed_rerank_models.contains(&model_id.to_string())
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
                .unwrap_or_else(|_| "embedding-inference-api".to_string()),
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
            all_embedding_models: false,
            allowed_embedding_models: vec![
                "BAAI/bge-small-en-v1.5".to_string(),
                "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            ],
            all_rerank_models: false,
            allowed_rerank_models: vec![],
            max_batch_size: 256,
            max_concurrent_requests: 2,
            queue_timeout_ms: 5000,
        };

        assert_eq!(model.allowed_embedding_models.len(), 2);
        assert_eq!(model.hf_home, Some(PathBuf::from("/tmp/hf_cache")));
        assert_eq!(
            model.hf_endpoint,
            Some("https://hf-mirror.example.com".to_string())
        );
        assert_eq!(model.model_path, Some(PathBuf::from("/models/custom")));
    }

    #[test]
    fn test_model_filtering() {
        // all_embedding_models = true means all models allowed
        let config_all_allowed = ModelConfig {
            hf_home: None,
            hf_endpoint: None,
            model_path: None,
            all_embedding_models: true,
            allowed_embedding_models: vec![],
            all_rerank_models: true,
            allowed_rerank_models: vec![],
            max_batch_size: 256,
            max_concurrent_requests: 2,
            queue_timeout_ms: 5000,
        };
        assert!(config_all_allowed.is_embedding_model_allowed("any-model"));
        assert!(config_all_allowed.is_rerank_model_allowed("any-model"));

        // Specific allowed list
        let config_restricted = ModelConfig {
            hf_home: None,
            hf_endpoint: None,
            model_path: None,
            all_embedding_models: false,
            allowed_embedding_models: vec![
                "BAAI/bge-small-en-v1.5".to_string(),
                "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            ],
            all_rerank_models: false,
            allowed_rerank_models: vec!["BAAI/bge-reranker-base".to_string()],
            max_batch_size: 256,
            max_concurrent_requests: 2,
            queue_timeout_ms: 5000,
        };
        // Embedding checks
        assert!(config_restricted.is_embedding_model_allowed("BAAI/bge-small-en-v1.5"));
        assert!(
            config_restricted.is_embedding_model_allowed("sentence-transformers/all-MiniLM-L6-v2")
        );
        assert!(!config_restricted.is_embedding_model_allowed("BAAI/bge-large-en-v1.5"));

        // Rerank checks
        assert!(config_restricted.is_rerank_model_allowed("BAAI/bge-reranker-base"));
        assert!(!config_restricted.is_rerank_model_allowed("BAAI/bge-reranker-v2-m3"));

        // No rerankers configured
        let config_no_rerankers = ModelConfig {
            hf_home: None,
            hf_endpoint: None,
            model_path: None,
            all_embedding_models: false,
            allowed_embedding_models: vec!["BAAI/bge-small-en-v1.5".to_string()],
            all_rerank_models: false,
            allowed_rerank_models: vec![],
            max_batch_size: 256,
            max_concurrent_requests: 2,
            queue_timeout_ms: 5000,
        };
        assert!(!config_no_rerankers.is_rerank_model_allowed("any-model"));
    }

    #[test]
    fn test_log_format_parsing() {
        assert_eq!(LogFormat::Json, LogFormat::Json);
        assert_eq!(LogFormat::Pretty, LogFormat::Pretty);
        assert_ne!(LogFormat::Json, LogFormat::Pretty);
    }

    #[test]
    fn test_server_config_fields() {
        let server = ServerConfig {
            hostname: "127.0.0.1".to_string(),
            port: 8080,
            cors_allowed_origins: vec!["http://localhost:3000".to_string()],
        };

        assert_eq!(server.hostname, "127.0.0.1");
        assert_eq!(server.port, 8080);
        assert_eq!(server.cors_allowed_origins.len(), 1);
    }
}
