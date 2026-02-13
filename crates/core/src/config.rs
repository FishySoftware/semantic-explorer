//! Centralized configuration management.
//!
//! All configuration is loaded from environment variables at startup.
//! This provides a single source of truth and fails fast if required config is missing.

use anyhow::{Context, Result};
use std::env;
use std::time::Duration;

/// Main application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub nats: NatsConfig,
    pub qdrant: QdrantConfig,
    pub s3: S3Config,
    pub server: ServerConfig,
    pub observability: ObservabilityConfig,
    pub tls: TlsConfig,
    pub oidc_session: OidcSessionConfig,
    pub oidc: OidcConfig,
    pub inference: EmbeddingInferenceConfig,
    pub llm_inference: LlmInferenceConfig,
    pub worker: WorkerConfig,
    pub valkey: ValkeyConfig,
}

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

/// NATS configuration
#[derive(Debug, Clone)]
pub struct NatsConfig {
    pub url: String,
    pub replicas: u32,
}

/// Qdrant vector database configuration
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    /// Quantization type: "none", "scalar", or "product"
    pub quantization_type: String,
    /// Scalar quantization parameters
    pub quantization_scalar_enabled: bool,
    /// Product quantization parameters
    pub quantization_product_enabled: bool,
}

/// S3-compatible storage configuration
#[derive(Debug, Clone)]
pub struct S3Config {
    pub region: String,
    /// Optional access key ID for static credentials
    /// If None, the AWS SDK will use the default credential provider chain
    pub access_key_id: Option<String>,
    /// Optional secret access key for static credentials
    /// If None, the AWS SDK will use the default credential provider chain
    pub secret_access_key: Option<String>,
    pub endpoint_url: String,
    /// Single S3 bucket for all collections
    pub bucket_name: String,
    /// Maximum file size for downloads via API (in bytes)
    /// Prevents memory exhaustion and DoS attacks
    pub max_download_size_bytes: i64,
    /// Maximum file size for uploads via API (in bytes)
    /// Should match server's multipart form limits
    pub max_upload_size_bytes: i64,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub hostname: String,
    pub port: u16,
    pub static_files_dir: String,
    pub cors_allowed_origins: Vec<String>,
    pub shutdown_timeout_secs: Option<u64>,
    /// Public URL for external access (used for OIDC callbacks)
    /// If not set, defaults to http://{hostname}:{port}
    pub public_url: Option<String>,
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

/// TLS configuration for server and client certificates
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Enable TLS/SSL on the server
    pub server_ssl_enabled: bool,
    /// Path to server certificate file (PEM format)
    pub server_cert_path: Option<String>,
    /// Path to server private key file (PEM format)
    pub server_key_path: Option<String>,
    /// Enable mutual TLS for outbound HTTP clients
    pub client_mtls_enabled: bool,
    /// Path to client certificate file (PEM format)
    pub client_cert_path: Option<String>,
    /// Path to client private key file (PEM format)
    pub client_key_path: Option<String>,
    /// Path to CA certificate bundle for verifying server certificates
    /// If None, the system's native certificate store will be used
    pub ca_cert_path: Option<String>,
}

/// OIDC session management configuration
#[derive(Debug, Clone)]
pub struct OidcSessionConfig {
    /// Enable enhanced session management features
    pub enabled: bool,
    /// Session timeout in seconds (default: 3600 = 1 hour)
    pub session_timeout_secs: u64,
    /// Enable refresh token rotation for enhanced security
    pub refresh_token_rotation_enabled: bool,
    /// Maximum number of concurrent sessions per user
    pub max_concurrent_sessions: u32,
    /// Session inactivity timeout in seconds (default: 1800 = 30 minutes)
    pub inactivity_timeout_secs: u64,
}

/// Local inference API configuration
#[derive(Debug, Clone)]
pub struct EmbeddingInferenceConfig {
    /// URL of the local inference API service
    pub url: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

/// LLM inference API configuration
#[derive(Debug, Clone)]
pub struct LlmInferenceConfig {
    /// URL of the local LLM inference API service
    pub url: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

/// Worker and batch processing configuration
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    // Batch sizes
    /// Batch size for search operations (default: 200)
    pub search_batch_size: u64,
    /// Batch size for chat document inserts (default: 500)
    pub chat_batch_size: usize,
    /// Batch size for dataset processing (default: 1000)
    pub dataset_batch_size: usize,
    /// Batch size for S3 delete operations (default: 1000)
    pub s3_delete_batch_size: usize,
    /// Chunk size for Qdrant uploads (default: 200)
    pub qdrant_upload_chunk_size: usize,
}

/// Valkey (Redis-compatible) cache configuration
#[derive(Debug, Clone)]
pub struct ValkeyConfig {
    /// Valkey connection URL (e.g., redis://localhost:6379)
    pub url: String,
    /// Optional separate read replica URL for read operations
    /// Defaults to the primary URL if not set
    pub read_url: String,
    /// Optional authentication password
    pub password: Option<String>,
    /// Enable TLS for Valkey connections
    pub tls_enabled: bool,
    /// Connection pool size (default: 10)
    pub pool_size: u32,
    /// TTL for bearer token cache entries in seconds (default: 120)
    pub bearer_cache_ttl_secs: u64,
    /// TTL for resource metadata cache entries in seconds (default: 300)
    pub resource_cache_ttl_secs: u64,
    /// Connection timeout in seconds (default: 5)
    pub connect_timeout_secs: u64,
    /// Response timeout in seconds (default: 2)
    pub response_timeout_secs: u64,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// This should be called once at application startup.
    /// It will fail fast if required configuration is missing.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database: DatabaseConfig::from_env()?,
            nats: NatsConfig::from_env()?,
            qdrant: QdrantConfig::from_env()?,
            s3: S3Config::from_env()?,
            server: ServerConfig::from_env()?,
            observability: ObservabilityConfig::from_env()?,
            tls: TlsConfig::from_env()?,
            oidc_session: OidcSessionConfig::from_env()?,
            oidc: OidcConfig::from_env()?,
            inference: EmbeddingInferenceConfig::from_env()?,
            llm_inference: LlmInferenceConfig::from_env()?,
            worker: WorkerConfig::from_env()?,
            valkey: ValkeyConfig::from_env()?,
        })
    }
}

impl DatabaseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .context("DB_MAX_CONNECTIONS must be a number")?,
            min_connections: env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .context("DB_MIN_CONNECTIONS must be a number")?,
            acquire_timeout: Duration::from_secs(
                env::var("DB_ACQUIRE_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .context("DB_ACQUIRE_TIMEOUT_SECS must be a number")?,
            ),
            idle_timeout: Duration::from_secs(
                env::var("DB_IDLE_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .context("DB_IDLE_TIMEOUT_SECS must be a number")?,
            ),
            max_lifetime: Duration::from_secs(
                env::var("DB_MAX_LIFETIME_SECS")
                    .unwrap_or_else(|_| "1800".to_string())
                    .parse()
                    .context("DB_MAX_LIFETIME_SECS must be a number")?,
            ),
        })
    }
}

impl NatsConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string()),
            replicas: env::var("NATS_REPLICAS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .context("NATS_REPLICAS must be a number")?,
        })
    }
}

impl QdrantConfig {
    pub fn from_env() -> Result<Self> {
        let quantization_type = env::var("QDRANT_QUANTIZATION_TYPE")
            .unwrap_or_else(|_| "none".to_string())
            .to_lowercase();

        // Validate quantization type
        if !["none", "scalar", "product"].contains(&quantization_type.as_str()) {
            anyhow::bail!(
                "QDRANT_QUANTIZATION_TYPE must be 'none', 'scalar', or 'product', got '{}'",
                quantization_type
            );
        }

        Ok(Self {
            url: env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string()),
            api_key: env::var("QDRANT_API_KEY").ok(),
            timeout: Duration::from_secs(
                env::var("QDRANT_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .context("QDRANT_TIMEOUT_SECS must be a number")?,
            ),
            connect_timeout: Duration::from_secs(
                env::var("QDRANT_CONNECT_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .context("QDRANT_CONNECT_TIMEOUT_SECS must be a number")?,
            ),
            quantization_type: quantization_type.clone(),
            quantization_scalar_enabled: quantization_type == "scalar"
                || env::var("QDRANT_QUANTIZATION_SCALAR_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
            quantization_product_enabled: quantization_type == "product"
                || env::var("QDRANT_QUANTIZATION_PRODUCT_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
        })
    }
}

impl S3Config {
    pub fn from_env() -> Result<Self> {
        // Default limits: 100MB for downloads, 1GB for uploads
        let default_max_download = (100 * 1024 * 1024).to_string(); // 100MB
        let default_max_upload = (1024 * 1024 * 1024).to_string(); // 1GB

        // Make credentials optional to support IAM roles, instance profiles, etc.
        let access_key_id = env::var("AWS_ACCESS_KEY_ID").ok();
        let secret_access_key = env::var("AWS_SECRET_ACCESS_KEY").ok();

        Ok(Self {
            region: env::var("AWS_REGION").context("AWS_REGION is required")?,
            access_key_id,
            secret_access_key,
            endpoint_url: env::var("AWS_ENDPOINT_URL").context("AWS_ENDPOINT_URL is required")?,
            bucket_name: env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME is required")?,
            max_download_size_bytes: env::var("S3_MAX_DOWNLOAD_SIZE_BYTES")
                .unwrap_or(default_max_download)
                .parse()
                .context("S3_MAX_DOWNLOAD_SIZE_BYTES must be a number")?,
            max_upload_size_bytes: env::var("S3_MAX_UPLOAD_SIZE_BYTES")
                .unwrap_or(default_max_upload)
                .parse()
                .context("S3_MAX_UPLOAD_SIZE_BYTES must be a number")?,
        })
    }
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let cors_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        let shutdown_timeout_secs = env::var("SHUTDOWN_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok());

        // PUBLIC_URL is used for external-facing URLs like OIDC callbacks
        let public_url = env::var("PUBLIC_URL").ok();

        Ok(Self {
            hostname: env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("PORT must be a number")?,
            static_files_dir: env::var("STATIC_FILES_DIR")
                .unwrap_or_else(|_| "./semantic-explorer-ui/".to_string()),
            cors_allowed_origins: cors_origins,
            shutdown_timeout_secs,
            public_url,
        })
    }
}

impl ObservabilityConfig {
    pub fn from_env() -> Result<Self> {
        let log_format = match env::var("LOG_FORMAT")
            .unwrap_or_else(|_| "json".to_string())
            .to_lowercase()
            .as_str()
        {
            "pretty" | "human" | "text" => LogFormat::Pretty,
            _ => LogFormat::Json,
        };

        Ok(Self {
            service_name: env::var("SERVICE_NAME")
                .unwrap_or_else(|_| "semantic-explorer".to_string()),
            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            log_format,
        })
    }
}

/// OIDC authentication configuration
#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub client_id: String,
    pub client_secret: String,
    pub issuer_url: String,
    pub use_pkce: bool,
}

impl OidcConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            client_id: env::var("OIDC_CLIENT_ID").context("OIDC_CLIENT_ID is required")?,
            client_secret: env::var("OIDC_CLIENT_SECRET")
                .context("OIDC_CLIENT_SECRET is required")?,
            issuer_url: env::var("OIDC_ISSUER_URL").context("OIDC_ISSUER_URL is required")?,
            use_pkce: env::var("OIDC_USE_PKCE")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
        })
    }
}

impl TlsConfig {
    pub fn from_env() -> Result<Self> {
        let server_ssl_enabled = env::var("SERVER_SSL_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        let client_mtls_enabled = env::var("CLIENT_MTLS_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        // Load CA certificate path if explicitly configured or if the default path exists
        let ca_cert_path = match env::var("TLS_CA_CERT_PATH") {
            Ok(path) => Some(path),
            Err(_) => {
                // Check if default path exists
                let default_path = "/app/certs/ca-bundle.crt";
                if std::path::Path::new(default_path).exists() {
                    Some(default_path.to_string())
                } else {
                    // Fall back to native system roots
                    None
                }
            }
        };

        // Validate server SSL configuration
        let (server_cert_path, server_key_path) = if server_ssl_enabled {
            let cert_path = env::var("TLS_SERVER_CERT_PATH")
                .context("TLS_SERVER_CERT_PATH is required when SERVER_SSL_ENABLED=true")?;
            let key_path = env::var("TLS_SERVER_KEY_PATH")
                .context("TLS_SERVER_KEY_PATH is required when SERVER_SSL_ENABLED=true")?;
            (Some(cert_path), Some(key_path))
        } else {
            (None, None)
        };

        // Validate client mTLS configuration
        let (client_cert_path, client_key_path) = if client_mtls_enabled {
            let cert_path = env::var("TLS_CLIENT_CERT_PATH")
                .context("TLS_CLIENT_CERT_PATH is required when CLIENT_MTLS_ENABLED=true")?;
            let key_path = env::var("TLS_CLIENT_KEY_PATH")
                .context("TLS_CLIENT_KEY_PATH is required when CLIENT_MTLS_ENABLED=true")?;
            (Some(cert_path), Some(key_path))
        } else {
            (None, None)
        };

        Ok(Self {
            server_ssl_enabled,
            server_cert_path,
            server_key_path,
            client_mtls_enabled,
            client_cert_path,
            client_key_path,
            ca_cert_path,
        })
    }
}

impl OidcSessionConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            enabled: env::var("OIDC_SESSION_MANAGEMENT_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                == "true",
            session_timeout_secs: env::var("OIDC_SESSION_TIMEOUT_SECS")
                .unwrap_or_else(|_| "3600".to_string()) // 1 hour default
                .parse()
                .context("OIDC_SESSION_TIMEOUT_SECS must be a number")?,
            refresh_token_rotation_enabled: env::var("OIDC_REFRESH_TOKEN_ROTATION_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                == "true",
            max_concurrent_sessions: env::var("OIDC_MAX_CONCURRENT_SESSIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("OIDC_MAX_CONCURRENT_SESSIONS must be a number")?,
            inactivity_timeout_secs: env::var("OIDC_INACTIVITY_TIMEOUT_SECS")
                .unwrap_or_else(|_| "1800".to_string()) // 30 minutes default
                .parse()
                .context("OIDC_INACTIVITY_TIMEOUT_SECS must be a number")?,
        })
    }
}

impl EmbeddingInferenceConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: env::var("EMBEDDING_INFERENCE_API_URL")
                .unwrap_or_else(|_| "http://localhost:8090".to_string()),
            timeout_secs: env::var("EMBEDDING_INFERENCE_API_TIMEOUT_SECS")
                .unwrap_or_else(|_| "120".to_string())
                .parse()
                .context("EMBEDDING_INFERENCE_API_TIMEOUT_SECS must be a number")?,
        })
    }
}

impl LlmInferenceConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: env::var("LLM_INFERENCE_API_URL")
                .unwrap_or_else(|_| "http://localhost:8091".to_string()),
            timeout_secs: env::var("LLM_INFERENCE_API_TIMEOUT_SECS")
                .unwrap_or_else(|_| "120".to_string())
                .parse()
                .context("LLM_INFERENCE_API_TIMEOUT_SECS must be a number")?,
        })
    }
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            // Batch sizes
            search_batch_size: env::var("WORKER_SEARCH_BATCH_SIZE")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .context("WORKER_SEARCH_BATCH_SIZE must be a number")?,
            chat_batch_size: env::var("WORKER_CHAT_BATCH_SIZE")
                .unwrap_or_else(|_| "500".to_string())
                .parse()
                .context("WORKER_CHAT_BATCH_SIZE must be a number")?,
            dataset_batch_size: env::var("WORKER_DATASET_BATCH_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .context("WORKER_DATASET_BATCH_SIZE must be a number")?,
            s3_delete_batch_size: env::var("WORKER_S3_DELETE_BATCH_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .context("WORKER_S3_DELETE_BATCH_SIZE must be a number")?,
            qdrant_upload_chunk_size: env::var("WORKER_QDRANT_UPLOAD_CHUNK_SIZE")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .context("WORKER_QDRANT_UPLOAD_CHUNK_SIZE must be a number")?,
        })
    }
}

impl ValkeyConfig {
    pub fn from_env() -> Result<Self> {
        let url = env::var("VALKEY_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let read_url = env::var("VALKEY_READ_URL").unwrap_or_else(|_| url.clone());

        Ok(Self {
            url,
            read_url,
            password: env::var("VALKEY_PASSWORD").ok().filter(|s| !s.is_empty()),
            tls_enabled: env::var("VALKEY_TLS_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
            pool_size: env::var("VALKEY_POOL_SIZE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("VALKEY_POOL_SIZE must be a number")?,
            bearer_cache_ttl_secs: env::var("VALKEY_BEARER_CACHE_TTL_SECS")
                .unwrap_or_else(|_| "120".to_string())
                .parse()
                .context("VALKEY_BEARER_CACHE_TTL_SECS must be a number")?,
            resource_cache_ttl_secs: env::var("VALKEY_RESOURCE_CACHE_TTL_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .context("VALKEY_RESOURCE_CACHE_TTL_SECS must be a number")?,
            connect_timeout_secs: env::var("VALKEY_CONNECT_TIMEOUT_SECS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("VALKEY_CONNECT_TIMEOUT_SECS must be a number")?,
            response_timeout_secs: env::var("VALKEY_RESPONSE_TIMEOUT_SECS")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .context("VALKEY_RESPONSE_TIMEOUT_SECS must be a number")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_parsing() {
        // Test that defaults work
        let config = ObservabilityConfig {
            service_name: "test".to_string(),
            otlp_endpoint: "http://localhost:4317".to_string(),
            log_format: LogFormat::Json,
        };
        assert_eq!(config.log_format, LogFormat::Json);
    }
}
