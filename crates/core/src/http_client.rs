//! Unified HTTP client with TLS support
//!
//! Provides a shared `reqwest::Client` configured with:
//! - Optional CA certificate verification (falls back to system roots if not provided)
//! - Optional client certificate for mutual TLS
//! - Proper timeout configuration

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tracing::info;

/// Global HTTP client instance shared across the application
/// Automatically configured with TLS settings from environment variables
pub static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    build_http_client_from_env().expect("Failed to build HTTP client from environment")
});

/// Initialize the global HTTP client with TLS configuration
pub fn initialize(tls_config: &crate::config::TlsConfig) -> Result<()> {
    // Verify CA cert path exists if provided
    if let Some(ca_path) = &tls_config.ca_cert_path {
        if !Path::new(ca_path).exists() {
            return Err(anyhow::anyhow!("CA certificate not found at: {}", ca_path));
        }
    } else {
        info!("No custom CA certificate configured, using system native roots");
    }

    // Validate server certificates if enabled
    if tls_config.server_ssl_enabled {
        if let Some(cert_path) = &tls_config.server_cert_path
            && !Path::new(cert_path).exists()
        {
            return Err(anyhow::anyhow!(
                "Server certificate not found at: {}",
                cert_path
            ));
        }
        if let Some(key_path) = &tls_config.server_key_path
            && !Path::new(key_path).exists()
        {
            return Err(anyhow::anyhow!(
                "Server private key not found at: {}",
                key_path
            ));
        }
    }

    // Validate client mTLS certificates if enabled
    if tls_config.client_mtls_enabled {
        if let Some(cert_path) = &tls_config.client_cert_path
            && !Path::new(cert_path).exists()
        {
            return Err(anyhow::anyhow!(
                "Client certificate not found at: {}",
                cert_path
            ));
        }
        if let Some(key_path) = &tls_config.client_key_path
            && !Path::new(key_path).exists()
        {
            return Err(anyhow::anyhow!(
                "Client private key not found at: {}",
                key_path
            ));
        }
    }

    info!(
        "HTTP client TLS initialized: server_ssl_enabled={}, client_mtls_enabled={}",
        tls_config.server_ssl_enabled, tls_config.client_mtls_enabled
    );

    Ok(())
}

/// Build HTTP client from environment variables
/// Optionally loads CA certificate (truststore) if configured or default exists
/// Optionally loads client certificates for mTLS if CLIENT_MTLS_ENABLED=true
fn build_http_client_from_env() -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90));

    // Load CA certificate if explicitly configured or default exists
    let ca_cert_path = match env::var("TLS_CA_CERT_PATH") {
        Ok(path) => Some(path),
        Err(_) => {
            let default_path = "/app/certs/ca-bundle.crt";
            if Path::new(default_path).exists() {
                Some(default_path.to_string())
            } else {
                None
            }
        }
    };

    if let Some(path) = ca_cert_path {
        let ca_cert =
            load_ca_cert(&path).context("Failed to load CA certificate from environment")?;
        builder = builder.add_root_certificate(ca_cert);
        info!("CA certificate loaded from: {}", path);
    } else {
        info!("No custom CA certificate found, using system native roots");
    }

    // Optionally load client certificates for mTLS
    let client_mtls_enabled = env::var("CLIENT_MTLS_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if client_mtls_enabled {
        let cert_path = env::var("TLS_CLIENT_CERT_PATH")
            .context("TLS_CLIENT_CERT_PATH required when CLIENT_MTLS_ENABLED=true")?;
        let key_path = env::var("TLS_CLIENT_KEY_PATH")
            .context("TLS_CLIENT_KEY_PATH required when CLIENT_MTLS_ENABLED=true")?;

        let client_identity = load_client_identity(&cert_path, &key_path)
            .context("Failed to load client certificate and key from environment")?;
        builder = builder.identity(client_identity);
        info!("Client mutual TLS enabled");
    }

    builder
        .build()
        .context("Failed to build HTTP client from environment")
}

/// Build a new HTTP client with the given TLS configuration
pub fn build_client(tls_config: &crate::config::TlsConfig) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(30));

    // Load CA certificate for server verification if provided
    if let Some(ca_path) = &tls_config.ca_cert_path {
        let ca_cert = load_ca_cert(ca_path).context("Failed to load CA certificate")?;
        builder = builder.add_root_certificate(ca_cert);
        info!("CA certificate loaded from: {}", ca_path);
    } else {
        info!("Using system native roots for TLS verification");
    }

    // Load client certificate for mutual TLS if enabled
    if tls_config.client_mtls_enabled
        && let (Some(cert_path), Some(key_path)) =
            (&tls_config.client_cert_path, &tls_config.client_key_path)
    {
        let client_identity = load_client_identity(cert_path, key_path)
            .context("Failed to load client certificate and key")?;
        builder = builder.identity(client_identity);
        info!("Client mutual TLS enabled");
    }

    builder
        .build()
        .context("Failed to build HTTP client with TLS config")
}

/// Load CA certificate from PEM file
fn load_ca_cert(path: &str) -> Result<reqwest::Certificate> {
    let cert_pem =
        fs::read_to_string(path).context(format!("Failed to read CA certificate from {}", path))?;
    let cert = reqwest::Certificate::from_pem(cert_pem.as_bytes())
        .context("Failed to parse CA certificate as PEM")?;
    Ok(cert)
}

/// Load client certificate and private key from PEM files
fn load_client_identity(cert_path: &str, key_path: &str) -> Result<reqwest::Identity> {
    let cert_pem = fs::read_to_string(cert_path).context(format!(
        "Failed to read client certificate from {}",
        cert_path
    ))?;
    let key_pem = fs::read_to_string(key_path)
        .context(format!("Failed to read client key from {}", key_path))?;

    let identity_bytes = format!("{}{}", cert_pem, key_pem);
    let identity = reqwest::Identity::from_pem(identity_bytes.as_bytes())
        .context("Failed to parse client certificate and key as PEM")?;
    Ok(identity)
}
