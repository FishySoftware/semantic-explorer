//! Secure handling of sensitive data like API keys and passwords.
//!
//! This module provides types for safely storing and using secrets without
//! accidentally exposing them in logs, error messages, or debug output.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A wrapper type for sensitive strings that prevents accidental exposure.
///
/// `SecretString` provides:
/// - `Debug` implementation that shows `[REDACTED]` instead of the actual value
/// - `Display` implementation that shows `[REDACTED]`
/// - Safe serialization/deserialization
/// - Memory zeroing on drop (when zeroize feature is enabled)
///
/// # Example
/// ```
/// use semantic_explorer_core::secrets::SecretString;
///
/// let api_key = SecretString::new("sk-secret-key".to_string());
/// println!("{:?}", api_key); // Prints: SecretString([REDACTED])
/// assert_eq!(api_key.expose_secret(), "sk-secret-key");
/// ```
#[derive(Clone)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    /// Create a new SecretString from a String.
    pub fn new(secret: String) -> Self {
        Self { inner: secret }
    }

    /// Expose the secret value.
    ///
    /// Use this method sparingly and only when absolutely necessary.
    /// The exposed value should never be logged or included in error messages.
    #[inline]
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }

    /// Consume the SecretString and return the inner value.
    ///
    /// Use this method sparingly. Prefer `expose_secret()` when possible.
    #[inline]
    pub fn into_inner(mut self) -> String {
        // Take the inner value and replace with empty string
        // This avoids move-out-of-Drop issues
        std::mem::take(&mut self.inner)
    }

    /// Check if the secret is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the length of the secret.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretString").field(&"[REDACTED]").finish()
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl Default for SecretString {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        // Use constant-time comparison to prevent timing attacks
        constant_time_eq(self.inner.as_bytes(), other.inner.as_bytes())
    }
}

impl Eq for SecretString {}

// Serialize as a regular string (for when secrets need to be transmitted)
impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SecretString::new(s))
    }
}

/// Zeroize the memory on drop to prevent secrets from lingering in memory.
impl Drop for SecretString {
    fn drop(&mut self) {
        // Overwrite the string content with zeros
        // SAFETY: We're replacing the content with zeros, which is always valid UTF-8
        unsafe {
            let bytes = self.inner.as_bytes_mut();
            for byte in bytes.iter_mut() {
                std::ptr::write_volatile(byte, 0);
            }
        }
        // Prevent the compiler from optimizing away the zeroing
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    }
}

/// Constant-time comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Optional SecretString that safely handles Option<String> patterns.
#[derive(Clone, Default)]
pub struct OptionalSecret(Option<SecretString>);

impl OptionalSecret {
    pub fn new(secret: Option<String>) -> Self {
        Self(secret.map(SecretString::new))
    }

    pub fn expose_secret(&self) -> Option<&str> {
        self.0.as_ref().map(|s| s.expose_secret())
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn as_ref(&self) -> Option<&SecretString> {
        self.0.as_ref()
    }
}

impl fmt::Debug for OptionalSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(_) => f
                .debug_tuple("OptionalSecret")
                .field(&"[REDACTED]")
                .finish(),
            None => f.debug_tuple("OptionalSecret").field(&"None").finish(),
        }
    }
}

impl fmt::Display for OptionalSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(_) => write!(f, "[REDACTED]"),
            None => write!(f, "None"),
        }
    }
}

impl From<Option<String>> for OptionalSecret {
    fn from(s: Option<String>) -> Self {
        Self::new(s)
    }
}

impl Serialize for OptionalSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.0 {
            Some(secret) => serializer.serialize_some(&secret.inner),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for OptionalSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        Ok(OptionalSecret::new(opt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_string_debug_redaction() {
        let secret = SecretString::new("my-secret-api-key".to_string());
        let debug_output = format!("{:?}", secret);
        assert!(!debug_output.contains("my-secret-api-key"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn test_secret_string_display_redaction() {
        let secret = SecretString::new("my-secret-api-key".to_string());
        let display_output = format!("{}", secret);
        assert!(!display_output.contains("my-secret-api-key"));
        assert!(display_output.contains("[REDACTED]"));
    }

    #[test]
    fn test_secret_string_expose() {
        let secret = SecretString::new("my-secret-api-key".to_string());
        assert_eq!(secret.expose_secret(), "my-secret-api-key");
    }

    #[test]
    fn test_secret_string_equality() {
        let secret1 = SecretString::new("same-secret".to_string());
        let secret2 = SecretString::new("same-secret".to_string());
        let secret3 = SecretString::new("different-secret".to_string());

        assert_eq!(secret1, secret2);
        assert_ne!(secret1, secret3);
    }

    #[test]
    fn test_optional_secret_debug() {
        let some_secret = OptionalSecret::new(Some("secret".to_string()));
        let none_secret = OptionalSecret::new(None);

        let some_debug = format!("{:?}", some_secret);
        let none_debug = format!("{:?}", none_secret);

        assert!(some_debug.contains("[REDACTED]"));
        assert!(!some_debug.contains("secret"));
        assert!(none_debug.contains("None"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = SecretString::new("test-secret".to_string());
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SecretString = serde_json::from_str(&json).unwrap();

        assert_eq!(original.expose_secret(), deserialized.expose_secret());
    }
}
