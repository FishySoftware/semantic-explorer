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
}

/// Qdrant vector database configuration
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub connect_timeout: Duration,
}

/// S3-compatible storage configuration
#[derive(Debug, Clone)]
pub struct S3Config {
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint_url: String,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub hostname: String,
    pub port: u16,
    pub static_files_dir: String,
    pub cors_allowed_origins: Vec<String>,
    pub shutdown_timeout_secs: Option<u64>,
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
        })
    }
}

impl DatabaseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "50".to_string())
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
        })
    }
}

impl QdrantConfig {
    pub fn from_env() -> Result<Self> {
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
