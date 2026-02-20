//! Unified HTTP client with TLS support
//!
//! Provides a shared `reqwest::Client` configured with:
//! - Optional CA certificate verification (falls back to system roots if not provided)
//! - Optional client certificate for mutual TLS
//! - Connection pool tuning and timeout configuration
//!
//! TLS settings are loaded from [`TlsConfig`] (centralized env-based config),
//! eliminating scattered `env::var` reads.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tracing::info;

/// Global HTTP client instance shared across the application.
/// Configured from TLS environment variables via [`TlsConfig`] on first access.
pub static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    let tls_config = crate::config::TlsConfig::from_env()
        .expect("Failed to load TLS configuration from environment");
    build_client(&tls_config).expect("Failed to build HTTP client")
});

/// Build a `reqwest::Client` from the given TLS configuration.
///
/// Validates that referenced certificate files exist on disk, loads CA
/// certificates for server verification, and optionally configures mutual TLS
/// with client certificates.
pub fn build_client(tls_config: &crate::config::TlsConfig) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90));

    if let Some(ca_path) = &tls_config.ca_cert_path {
        if !Path::new(ca_path).exists() {
            return Err(anyhow::anyhow!("CA certificate not found at: {}", ca_path));
        }
        let ca_cert = load_ca_cert(ca_path).context("Failed to load CA certificate")?;
        builder = builder.add_root_certificate(ca_cert);
        info!("CA certificate loaded from: {}", ca_path);
    } else {
        info!("No custom CA certificate configured, using system native roots");
    }

    if tls_config.client_mtls_enabled {
        let cert_path = tls_config
            .client_cert_path
            .as_deref()
            .context("client_cert_path required when client_mtls_enabled=true")?;
        let key_path = tls_config
            .client_key_path
            .as_deref()
            .context("client_key_path required when client_mtls_enabled=true")?;

        if !Path::new(cert_path).exists() {
            return Err(anyhow::anyhow!(
                "Client certificate not found at: {}",
                cert_path
            ));
        }
        if !Path::new(key_path).exists() {
            return Err(anyhow::anyhow!(
                "Client private key not found at: {}",
                key_path
            ));
        }

        let client_identity = load_client_identity(cert_path, key_path)
            .context("Failed to load client certificate and key")?;
        builder = builder.identity(client_identity);
        info!("Client mutual TLS enabled");
    }

    info!(
        "HTTP client initialized: client_mtls_enabled={}",
        tls_config.client_mtls_enabled
    );

    builder.build().context("Failed to build HTTP client")
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
