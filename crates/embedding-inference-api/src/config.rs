//! Configuration for the inference API service.
//!
//! All configuration is loaded from environment variables at startup.
//! Supports airgapped deployments with HF_HOME and HF_ENDPOINT configuration.
//! Supports TLS/SSL for secure deployments using shared core configuration.

use anyhow::{Context, Result};
use semantic_explorer_core::config::TlsConfig;
use std::env;
use std::path::PathBuf;

/// Parse a human-readable byte size string (e.g. "4G", "512M", "1024K", "8589934592")
/// into a byte count. Supports suffixes: K/KB, M/MB, G/GB, T/TB (case-insensitive).
fn parse_byte_size(s: &str) -> Result<usize> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("empty byte size string");
    }

    // Find where the numeric part ends and the suffix begins
    let (num_part, suffix) = match s.find(|c: char| c.is_alphabetic()) {
        Some(idx) => (s[..idx].trim(), s[idx..].trim().to_uppercase()),
        None => {
            return s
                .parse::<usize>()
                .context("invalid byte size: not a number");
        }
    };

    let base: f64 = num_part
        .parse()
        .with_context(|| format!("invalid numeric part in byte size: {num_part}"))?;

    let multiplier: u64 = match suffix.as_str() {
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        "T" | "TB" => 1024 * 1024 * 1024 * 1024,
        other => anyhow::bail!("unknown byte size suffix: {other}"),
    };

    Ok((base * multiplier as f64) as usize)
}

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
    /// GPU pressure threshold percentage — reject requests above this % VRAM or compute utilization
    pub gpu_pressure_threshold: f64,
    /// CUDA memory arena size limit in bytes.
    /// When set, limits how much GPU VRAM the ONNX Runtime arena can allocate.
    /// When None (default), uses all available GPU memory (usize::MAX).
    pub cuda_arena_size: Option<usize>,
    /// Strategy for extending the CUDA memory arena when more memory is needed.
    /// NextPowerOfTwo (default): each extension doubles — fewer but larger allocations.
    /// SameAsRequested: each extension is exactly the requested size — more granular.
    pub cuda_arena_extend_strategy: CudaArenaExtendStrategy,
}

/// Strategy for extending the CUDA memory arena.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CudaArenaExtendStrategy {
    /// Each subsequent extension doubles in size (default).
    /// Reaches target size faster = fewer reallocations during model loading.
    NextPowerOfTwo,
    /// Each extension is exactly the size requested.
    /// More predictable memory usage but more frequent allocations.
    SameAsRequested,
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

        // Log CUDA arena config
        match self.models.cuda_arena_size {
            Some(size) => tracing::info!(
                cuda_arena_size_bytes = size,
                cuda_arena_size_human = %format!("{}MB", size / (1024 * 1024)),
                cuda_arena_extend_strategy = ?self.models.cuda_arena_extend_strategy,
                source = if env::var("CUDA_ARENA_SIZE").is_ok() { "explicit" } else { "auto-sized" },
                "CUDA memory arena configured"
            ),
            None => tracing::info!(
                cuda_arena_extend_strategy = ?self.models.cuda_arena_extend_strategy,
                "CUDA memory arena using all available GPU memory (no GPU detected for auto-sizing)"
            ),
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
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("INFERENCE_MAX_CONCURRENT_REQUESTS must be a number")?,
            queue_timeout_ms: env::var("INFERENCE_QUEUE_TIMEOUT_MS")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .context("INFERENCE_QUEUE_TIMEOUT_MS must be a number")?,
            gpu_pressure_threshold: env::var("GPU_PRESSURE_THRESHOLD")
                .unwrap_or_else(|_| "95.0".to_string())
                .parse()
                .context("GPU_PRESSURE_THRESHOLD must be a number")?,
            cuda_arena_size: match env::var("CUDA_ARENA_SIZE") {
                Ok(val) if !val.trim().is_empty() && val.trim() != "0" => {
                    Some(parse_byte_size(&val).context(
                        "CUDA_ARENA_SIZE must be a byte size (e.g. '4G', '512M', '8589934592')",
                    )?)
                }
                _ => None, // Default: use all available GPU memory
            },
            cuda_arena_extend_strategy: match env::var("CUDA_ARENA_EXTEND_STRATEGY") {
                Ok(val) => match val.trim().to_lowercase().as_str() {
                    "same" | "same_as_requested" | "sameasrequested" => {
                        CudaArenaExtendStrategy::SameAsRequested
                    }
                    "power_of_two" | "nextpoweroftwo" | "next_power_of_two" | "default" | "" => {
                        CudaArenaExtendStrategy::NextPowerOfTwo
                    }
                    other => anyhow::bail!(
                        "CUDA_ARENA_EXTEND_STRATEGY must be 'next_power_of_two' (default) or 'same_as_requested', got: {other}"
                    ),
                },
                _ => CudaArenaExtendStrategy::NextPowerOfTwo,
            },
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

    /// Resolve the effective CUDA arena size.
    ///
    /// If `CUDA_ARENA_SIZE` was explicitly set, use that value.
    /// Otherwise, auto-compute from GPU total VRAM x (gpu_pressure_threshold / 100)
    /// so the arena never grows past the pressure rejection threshold.
    ///
    /// This must be called after NVML is initialized (before model loading).
    pub fn resolve_effective_arena_size(&mut self) {
        use semantic_explorer_core::observability::gpu_monitor;

        if self.cuda_arena_size.is_some() {
            // Explicit size set — respect it
            return;
        }

        // Auto-compute from GPU VRAM and pressure threshold.
        // Use the minimum VRAM across all visible devices (incl. MIG slices)
        // so the arena cap is safe regardless of which device ORT targets.
        if let Some(min_vram) = gpu_monitor::get_min_device_vram() {
            let fraction = self.gpu_pressure_threshold / 100.0;
            // Leave a 5% additional margin below the threshold for CUDA runtime overhead
            // (cuDNN workspaces, kernel launches, etc. that live outside the ORT arena)
            let effective_fraction = (fraction - 0.05).max(0.5);
            let arena_limit = (min_vram as f64 * effective_fraction) as usize;

            // Log per-device VRAM for visibility in multi-GPU / MIG setups
            let per_device = gpu_monitor::get_vram_per_device();
            tracing::info!(
                devices = per_device.len(),
                per_device_vram_mb = ?per_device.iter().map(|(idx, v)| (*idx, v / (1024 * 1024))).collect::<Vec<_>>(),
                min_vram_mb = min_vram / (1024 * 1024),
                gpu_pressure_threshold = self.gpu_pressure_threshold,
                effective_fraction_pct = effective_fraction * 100.0,
                arena_limit_mb = arena_limit / (1024 * 1024),
                "Auto-sizing CUDA arena from minimum device VRAM to stay below GPU pressure threshold"
            );

            self.cuda_arena_size = Some(arena_limit);
        } else {
            tracing::warn!(
                "No GPU detected via NVML — cannot auto-size CUDA arena. \
                 Arena will use all available GPU memory (ONNX Runtime default). \
                 Set CUDA_ARENA_SIZE explicitly if needed."
            );
        }
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
            gpu_pressure_threshold: 95.0,
            cuda_arena_size: None,
            cuda_arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
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
            gpu_pressure_threshold: 95.0,
            cuda_arena_size: None,
            cuda_arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
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
            gpu_pressure_threshold: 95.0,
            cuda_arena_size: None,
            cuda_arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
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
            gpu_pressure_threshold: 95.0,
            cuda_arena_size: None,
            cuda_arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
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

    #[test]
    fn test_parse_byte_size() {
        // Raw bytes
        assert_eq!(parse_byte_size("1024").unwrap(), 1024);
        assert_eq!(parse_byte_size("8589934592").unwrap(), 8589934592);

        // Kilobytes
        assert_eq!(parse_byte_size("1K").unwrap(), 1024);
        assert_eq!(parse_byte_size("1KB").unwrap(), 1024);

        // Megabytes
        assert_eq!(parse_byte_size("512M").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_byte_size("512MB").unwrap(), 512 * 1024 * 1024);

        // Gigabytes
        assert_eq!(parse_byte_size("4G").unwrap(), 4 * 1024 * 1024 * 1024);
        assert_eq!(parse_byte_size("4GB").unwrap(), 4 * 1024 * 1024 * 1024);

        // Terabytes
        assert_eq!(parse_byte_size("1T").unwrap(), 1024 * 1024 * 1024 * 1024);

        // Case insensitive
        assert_eq!(parse_byte_size("4g").unwrap(), 4 * 1024 * 1024 * 1024);
        assert_eq!(parse_byte_size("512m").unwrap(), 512 * 1024 * 1024);

        // With whitespace
        assert_eq!(parse_byte_size("  4G  ").unwrap(), 4 * 1024 * 1024 * 1024);

        // Fractional
        assert_eq!(
            parse_byte_size("1.5G").unwrap(),
            (1.5 * 1024.0 * 1024.0 * 1024.0) as usize
        );

        // Errors
        assert!(parse_byte_size("").is_err());
        assert!(parse_byte_size("abc").is_err());
        assert!(parse_byte_size("4X").is_err());
    }
}
