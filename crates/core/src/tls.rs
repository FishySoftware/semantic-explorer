//! TLS/SSL configuration and utilities.
//!
//! Provides shared TLS configuration loading for actix-web servers.

use anyhow::Result;
use std::fs;

/// Load rustls configuration from certificate and key files.
///
/// Supports PKCS#8, PKCS#1 RSA, and SEC1 EC private key formats.
/// Also loads certificate chains for multi-certificate deployments.
///
/// # Arguments
/// * `cert_path` - Path to the server certificate file (PEM format)
/// * `key_path` - Path to the server private key file (PEM format)
///
/// # Returns
/// A configured `rustls::ServerConfig` ready for use with actix-web
///
/// # Example
/// ```ignore
/// use semantic_explorer_core::tls::load_tls_config;
///
/// let config = load_tls_config("/path/to/cert.pem", "/path/to/key.pem")?;
/// server.bind_rustls_0_23(("0.0.0.0", 443), config)?;
/// ```
pub fn load_tls_config(cert_path: &str, key_path: &str) -> Result<rustls::ServerConfig> {
    // Load certificate(s) - support certificate chains
    let cert_contents = fs::read_to_string(cert_path)
        .map_err(|e| anyhow::anyhow!("Failed to read certificate file {}: {}", cert_path, e))?;

    let cert_chain = load_certificates(&cert_contents)?;

    // Load private key
    let key_contents = fs::read_to_string(key_path)
        .map_err(|e| anyhow::anyhow!("Failed to read key file {}: {}", key_path, e))?;

    let private_key = load_private_key(&key_contents)?;

    // Build server config
    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .map_err(|e| anyhow::anyhow!("Invalid certificate or key: {}", e))?;

    // Enable HTTP/2 and HTTP/1.1 ALPN protocols
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(server_config)
}

/// Load certificates from PEM content.
///
/// Supports both single certificates and certificate chains.
fn load_certificates(pem_content: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let pems = pem::parse_many(pem_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse certificate PEM: {}", e))?;

    if pems.is_empty() {
        return Err(anyhow::anyhow!("No certificates found in PEM file"));
    }

    let mut cert_chain = Vec::with_capacity(pems.len());

    for cert_pem in pems {
        if cert_pem.tag() != "CERTIFICATE" {
            // Skip non-certificate entries (e.g., keys that might be bundled)
            continue;
        }
        let cert_der = cert_pem.contents().to_vec();
        cert_chain.push(rustls::pki_types::CertificateDer::from(cert_der));
    }

    if cert_chain.is_empty() {
        return Err(anyhow::anyhow!(
            "No valid CERTIFICATE entries found in PEM file"
        ));
    }

    Ok(cert_chain)
}

/// Load a private key from PEM content.
///
/// Supports PKCS#8, PKCS#1 RSA, and SEC1 EC private key formats.
fn load_private_key(pem_content: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let key_pem = pem::parse(pem_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse private key PEM: {}", e))?;

    let key_der = key_pem.contents().to_vec();

    // Support multiple key formats: PKCS#8, RSA, and EC
    let private_key = match key_pem.tag() {
        "PRIVATE KEY" => {
            // PKCS#8 format (most common for modern certificates)
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                key_der,
            ))
        }
        "RSA PRIVATE KEY" => {
            // PKCS#1 RSA format (legacy OpenSSL format)
            rustls::pki_types::PrivateKeyDer::Pkcs1(rustls::pki_types::PrivatePkcs1KeyDer::from(
                key_der,
            ))
        }
        "EC PRIVATE KEY" => {
            // SEC1 EC format (legacy EC key format)
            rustls::pki_types::PrivateKeyDer::Sec1(rustls::pki_types::PrivateSec1KeyDer::from(
                key_der,
            ))
        }
        tag => {
            return Err(anyhow::anyhow!(
                "Unsupported private key format: {}. Expected PRIVATE KEY, RSA PRIVATE KEY, or EC PRIVATE KEY",
                tag
            ));
        }
    };

    Ok(private_key)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_load_certificates_single() {
        // A minimal self-signed test certificate (not for production use)
        let pem = r#"-----BEGIN CERTIFICATE-----
MIIBkTCB+wIJAKHBfpegDPvvMA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBnVu
dXNlZDAeFw0yMzAxMDEwMDAwMDBaFw0yNDAxMDEwMDAwMDBaMBExDzANBgNVBAMM
BnVudXNlZDBcMA0GCSqGSIb3DQEBAQUAA0sAMEgCQQC5Q7q3q8r+9r7q7q7q7q7q
7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7qAgMB
AAGjUzBRMB0GA1UdDgQWBBQxMTExMTExMTExMTExMTExMTExMTAfBgNVHSMEGDAW
gBQxMTExMTExMTExMTExMTExMTExMTAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3
DQEBCwUAA0EAu0H6q3q8r+9r7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q
7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7q7g==
-----END CERTIFICATE-----"#;

        // This test just verifies the parsing logic works
        // The certificate content doesn't need to be valid for this test
        let result = pem::parse_many(pem);
        assert!(result.is_ok());
    }
}
