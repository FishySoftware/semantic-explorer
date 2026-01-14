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
    pub redis: RedisConfig,
    pub rate_limit: RateLimitConfig,
    pub oidc_session: OidcSessionConfig,
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
    pub replicas: u32, // Number of replicas for JetStream streams
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
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint_url: String,
    /// Single S3 bucket for all collections (replaces per-collection buckets)
    pub bucket_name: String,
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
    pub ca_cert_path: String,
}

/// Redis cluster configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub cluster_nodes: Vec<String>,
    pub pool_size: u32,
    pub connect_timeout: Duration,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub default_requests_per_minute: u64,
    pub search_requests_per_minute: u64,
    pub chat_requests_per_minute: u64,
    pub transform_requests_per_minute: u64,
    pub test_requests_per_minute: u64,
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
            redis: RedisConfig::from_env()?,
            rate_limit: RateLimitConfig::from_env()?,
            oidc_session: OidcSessionConfig::from_env()?,
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
                    .unwrap_or_else(|_| "30".to_string())
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
        Ok(Self {
            region: env::var("AWS_REGION").context("AWS_REGION is required")?,
            access_key_id: env::var("AWS_ACCESS_KEY_ID")
                .context("AWS_ACCESS_KEY_ID is required")?,
            secret_access_key: env::var("AWS_SECRET_ACCESS_KEY")
                .context("AWS_SECRET_ACCESS_KEY is required")?,
            endpoint_url: env::var("AWS_ENDPOINT_URL").context("AWS_ENDPOINT_URL is required")?,
            bucket_name: env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME is required")?,
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

        let ca_cert_path =
            env::var("TLS_CA_CERT_PATH").unwrap_or_else(|_| "/app/certs/ca-bundle.crt".to_string());

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

impl RedisConfig {
    pub fn from_env() -> Result<Self> {
        let cluster_nodes_str = env::var("REDIS_CLUSTER_NODES").unwrap_or_else(|_| {
            "redis://localhost:7000,redis://localhost:7001,redis://localhost:7002".to_string()
        });

        let cluster_nodes = cluster_nodes_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self {
            cluster_nodes,
            pool_size: env::var("REDIS_POOL_SIZE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("REDIS_POOL_SIZE must be a number")?,
            connect_timeout: Duration::from_secs(
                env::var("REDIS_CONNECT_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .context("REDIS_CONNECT_TIMEOUT_SECS must be a number")?,
            ),
        })
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            enabled: env::var("RATE_LIMIT_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                == "true",
            default_requests_per_minute: env::var("RATE_LIMIT_DEFAULT_REQUESTS_PER_MINUTE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .context("RATE_LIMIT_DEFAULT_REQUESTS_PER_MINUTE must be a number")?,
            search_requests_per_minute: env::var("RATE_LIMIT_SEARCH_REQUESTS_PER_MINUTE")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .context("RATE_LIMIT_SEARCH_REQUESTS_PER_MINUTE must be a number")?,
            chat_requests_per_minute: env::var("RATE_LIMIT_CHAT_REQUESTS_PER_MINUTE")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .context("RATE_LIMIT_CHAT_REQUESTS_PER_MINUTE must be a number")?,
            transform_requests_per_minute: env::var("RATE_LIMIT_TRANSFORM_REQUESTS_PER_MINUTE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .context("RATE_LIMIT_TRANSFORM_REQUESTS_PER_MINUTE must be a number")?,
            test_requests_per_minute: env::var("RATE_LIMIT_TEST_REQUESTS_PER_MINUTE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .context("RATE_LIMIT_TEST_REQUESTS_PER_MINUTE must be a number")?,
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
