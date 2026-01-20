use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as base64_engine};
use rand::Rng;
use std::env;

/// Encryption service for API keys and secrets
/// Uses AES-256-GCM for authenticated encryption
#[derive(Clone)]
pub struct EncryptionService {
    master_key: [u8; 32], // 256-bit key for AES-256
}

impl EncryptionService {
    /// Initialize the encryption service from environment variable
    pub fn from_env() -> Result<Self> {
        let master_key_str = env::var("ENCRYPTION_MASTER_KEY")
            .map_err(|_| anyhow!("ENCRYPTION_MASTER_KEY environment variable not set"))?;

        let master_key_bytes = hex::decode(&master_key_str).map_err(|_| {
            anyhow!("ENCRYPTION_MASTER_KEY must be valid hex string of 64 characters (32 bytes)")
        })?;

        if master_key_bytes.len() != 32 {
            return Err(anyhow!(
                "ENCRYPTION_MASTER_KEY must be exactly 32 bytes (64 hex characters), got {}",
                master_key_bytes.len()
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&master_key_bytes);

        Ok(EncryptionService { master_key: key })
    }

    /// Generate a new random master key (for key rotation or initial setup)
    pub fn generate_master_key() -> String {
        let mut rng = rand::rng();
        let key: [u8; 32] = rng.random();
        hex::encode(key)
    }

    /// Encrypt a secret (API key) using AES-256-GCM
    /// Returns base64-encoded ciphertext with nonce prepended
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.master_key));

        // Generate a random nonce (12 bytes for GCM)
        let mut rng = rand::rng();
        let mut nonce_bytes = [0u8; 12];
        rng.fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the plaintext
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Combine nonce + ciphertext and encode as base64
        // Format: base64(nonce || ciphertext)
        let mut encrypted_data = nonce_bytes.to_vec();
        encrypted_data.extend_from_slice(&ciphertext);

        Ok(base64_engine.encode(&encrypted_data))
    }

    /// Decrypt a secret (API key) encrypted with AES-256-GCM
    /// Expects base64-encoded input with nonce prepended
    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.master_key));

        // Decode from base64
        let encrypted_data = base64_engine
            .decode(encrypted)
            .map_err(|e| anyhow!("Failed to decode base64: {}", e))?;

        // Extract nonce (first 12 bytes) and ciphertext (rest)
        if encrypted_data.len() < 12 {
            return Err(anyhow!(
                "Encrypted data too short (must contain at least 12-byte nonce)"
            ));
        }

        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt the ciphertext
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow!("Decrypted data is not valid UTF-8: {}", e))
    }

    /// Check if a string looks like encrypted data
    pub fn is_encrypted(&self, data: &str) -> bool {
        // Try to decode as base64 and check if length is plausible
        base64_engine
            .decode(data)
            .map(|decoded| decoded.len() >= 12) // At least nonce + some ciphertext
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_master_key() {
        let key = EncryptionService::generate_master_key();
        assert_eq!(key.len(), 64); // 32 bytes = 64 hex characters
        assert!(hex::decode(&key).is_ok());
    }

    #[test]
    fn test_encrypt_decrypt() {
        let service = EncryptionService {
            master_key: *b"01234567890123456789012345678901",
        };

        let plaintext = "sk-1234567890abcdef";
        let encrypted = service.encrypt(plaintext).expect("Encryption failed");

        // Encrypted should be different and base64-encoded
        assert_ne!(plaintext, encrypted);
        assert!(base64_engine.decode(&encrypted).is_ok());

        let decrypted = service.decrypt(&encrypted).expect("Decryption failed");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_is_encrypted() {
        let service = EncryptionService {
            master_key: *b"01234567890123456789012345678901",
        };

        let plaintext = "not_encrypted_key";
        assert!(!service.is_encrypted(plaintext));

        let encrypted = service.encrypt(plaintext).expect("Encryption failed");
        assert!(service.is_encrypted(&encrypted));
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let service1 = EncryptionService {
            master_key: *b"01234567890123456789012345678901",
        };

        let service2 = EncryptionService {
            master_key: *b"10987654321fedcba9876543210fedcb",
        };

        let plaintext = "secret_api_key";
        let encrypted = service1.encrypt(plaintext).expect("Encryption failed");

        let result = service2.decrypt(&encrypted);
        assert!(result.is_err(), "Decryption with wrong key should fail");
    }
}
