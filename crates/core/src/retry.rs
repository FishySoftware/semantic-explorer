//! Configurable retry policies with exponential backoff and jitter.
//!
//! This module provides a generic retry mechanism for transient failures
//! when interacting with external services like Qdrant, S3, and inference APIs.

use anyhow::Result;
use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 = no retries, just the initial attempt)
    pub max_attempts: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier (e.g., 2.0 for exponential backoff)
    pub backoff_multiplier: f64,
    /// Add random jitter to prevent thundering herd (0.0 - 1.0)
    pub jitter_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

impl RetryPolicy {
    /// Load retry policy from environment variables with optional prefix
    pub fn from_env_with_prefix(prefix: &str) -> Self {
        let max_attempts = std::env::var(format!("{}_MAX_ATTEMPTS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        let initial_delay_ms = std::env::var(format!("{}_INITIAL_DELAY_MS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        let max_delay_ms = std::env::var(format!("{}_MAX_DELAY_MS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10000);

        let backoff_multiplier = std::env::var(format!("{}_BACKOFF_MULTIPLIER", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2.0);

        let jitter_factor = std::env::var(format!("{}_JITTER_FACTOR", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.1);

        Self {
            max_attempts,
            initial_delay: Duration::from_millis(initial_delay_ms),
            max_delay: Duration::from_millis(max_delay_ms),
            backoff_multiplier,
            jitter_factor,
        }
    }

    /// Load default retry policies from environment
    pub fn from_env() -> Self {
        Self::from_env_with_prefix("RETRY")
    }

    /// Calculate delay for a given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi((attempt - 1) as i32);
        let capped_delay = base_delay.min(self.max_delay.as_millis() as f64);

        // Add jitter
        let jitter = if self.jitter_factor > 0.0 {
            let jitter_range = capped_delay * self.jitter_factor;
            (rand::random::<f64>() - 0.5) * 2.0 * jitter_range
        } else {
            0.0
        };

        let final_delay = (capped_delay + jitter).max(0.0);
        Duration::from_millis(final_delay as u64)
    }
}

/// Trait to determine if an error is retryable
pub trait RetryableError {
    fn is_retryable(&self) -> bool;
}

/// Default implementation for anyhow::Error - check common retryable patterns
impl RetryableError for anyhow::Error {
    fn is_retryable(&self) -> bool {
        let error_str = self.to_string().to_lowercase();

        // Retryable network/transient errors
        let retryable_patterns = [
            "timeout",
            "connection refused",
            "connection reset",
            "temporarily unavailable",
            "service unavailable",
            "too many requests",
            "rate limit",
            "503",
            "502",
            "504",
            "429",
            "econnreset",
            "econnrefused",
            "etimedout",
            "broken pipe",
        ];

        retryable_patterns
            .iter()
            .any(|pattern| error_str.contains(pattern))
    }
}

/// Execute an async operation with retry policy
///
/// # Arguments
/// * `policy` - The retry policy to use
/// * `operation_name` - Name for logging/metrics
/// * `operation` - The async operation to execute
///
/// # Example
/// ```ignore
/// let policy = RetryPolicy::from_env_with_prefix("QDRANT_RETRY");
/// let result = retry_with_policy(&policy, "qdrant_upsert", || async {
///     qdrant_client.upsert_points(...).await
/// }).await?;
/// ```
pub async fn retry_with_policy<F, Fut, T, E>(
    policy: &RetryPolicy,
    operation_name: &str,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display + RetryableError,
{
    let mut attempt = 0;
    let max_attempts = policy.max_attempts + 1; // +1 for initial attempt

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    debug!(
                        operation = operation_name,
                        attempt = attempt,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                let is_retryable = e.is_retryable();
                let has_attempts_left = attempt < max_attempts;

                if is_retryable && has_attempts_left {
                    let delay = policy.delay_for_attempt(attempt);
                    warn!(
                        operation = operation_name,
                        attempt = attempt,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis() as u64,
                        error = %e,
                        "Retryable error, will retry after delay"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                if !is_retryable {
                    debug!(
                        operation = operation_name,
                        attempt = attempt,
                        error = %e,
                        "Non-retryable error, failing immediately"
                    );
                } else {
                    warn!(
                        operation = operation_name,
                        attempt = attempt,
                        max_attempts = max_attempts,
                        error = %e,
                        "Max retry attempts exhausted"
                    );
                }

                return Err(e);
            }
        }
    }
}

/// Convenience function for retrying with default policy
pub async fn retry<F, Fut, T, E>(operation_name: &str, operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display + RetryableError,
{
    retry_with_policy(&RetryPolicy::default(), operation_name, operation).await
}

/// Retry policy specifically configured for Qdrant operations
pub fn qdrant_retry_policy() -> RetryPolicy {
    RetryPolicy::from_env_with_prefix("QDRANT_RETRY")
}

/// Retry policy specifically configured for S3 operations
pub fn s3_retry_policy() -> RetryPolicy {
    RetryPolicy::from_env_with_prefix("S3_RETRY")
}

/// Retry policy specifically configured for inference API operations
pub fn inference_retry_policy() -> RetryPolicy {
    RetryPolicy::from_env_with_prefix("INFERENCE_RETRY")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_calculation() {
        let policy = RetryPolicy {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.0, // Disable jitter for deterministic test
        };

        assert_eq!(policy.delay_for_attempt(0), Duration::ZERO);
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
        assert_eq!(policy.delay_for_attempt(4), Duration::from_millis(800));
        // Should cap at max_delay
        assert_eq!(policy.delay_for_attempt(10), Duration::from_secs(10));
    }

    #[test]
    fn test_retryable_error_patterns() {
        let timeout_err = anyhow::anyhow!("Request timeout after 30s");
        assert!(timeout_err.is_retryable());

        let rate_limit_err = anyhow::anyhow!("Rate limit exceeded (429)");
        assert!(rate_limit_err.is_retryable());

        let not_found_err = anyhow::anyhow!("Resource not found");
        assert!(!not_found_err.is_retryable());

        let validation_err = anyhow::anyhow!("Invalid input parameter");
        assert!(!validation_err.is_retryable());
    }
}
