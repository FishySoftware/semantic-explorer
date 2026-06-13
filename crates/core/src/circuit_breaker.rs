//! Circuit breaker pattern implementation for resilient external service calls.
//!
//! The circuit breaker prevents cascading failures by failing fast when
//! an external service (Qdrant, S3, inference APIs) is experiencing issues.
//!
//! States:
//! - Closed: Normal operation, requests pass through
//! - Open: Service is failing, requests fail immediately
//! - HalfOpen: Testing if service has recovered

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Service is failing - requests fail immediately
    Open,
    /// Testing recovery - limited requests pass through
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

/// Configuration for a circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Name for logging and metrics
    pub name: String,
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open state before closing
    pub success_threshold: u32,
    /// Time to wait in open state before transitioning to half-open
    pub timeout: Duration,
    /// Time window for counting failures (resets after this duration)
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(60),
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a new circuit breaker config with the given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Load circuit breaker config from environment variables with prefix
    pub fn from_env_with_prefix(name: &str, prefix: &str) -> Self {
        let failure_threshold = std::env::var(format!("{}_FAILURE_THRESHOLD", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let success_threshold = std::env::var(format!("{}_SUCCESS_THRESHOLD", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        let timeout_secs = std::env::var(format!("{}_TIMEOUT_SECS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let failure_window_secs = std::env::var(format!("{}_FAILURE_WINDOW_SECS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        Self {
            name: name.to_string(),
            failure_threshold,
            success_threshold,
            timeout: Duration::from_secs(timeout_secs),
            failure_window: Duration::from_secs(failure_window_secs),
        }
    }
}

/// Internal state tracking for the circuit breaker
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    opened_at: Option<Instant>,
    /// Number of currently in-flight probes while in HalfOpen state.
    /// At most `success_threshold` probes are allowed concurrently.
    half_open_in_flight: u32,
}

/// Thread-safe circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitBreakerState>,
    // Atomic counters for metrics (faster than locking)
    total_requests: AtomicU64,
    total_failures: AtomicU64,
    total_rejections: AtomicU64,
    state_transitions: AtomicU32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            state: RwLock::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
                opened_at: None,
                half_open_in_flight: 0,
            }),
            total_requests: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_rejections: AtomicU64::new(0),
            state_transitions: AtomicU32::new(0),
        })
    }

    /// Get the current circuit state
    pub async fn state(&self) -> CircuitState {
        self.state.read().await.state
    }

    /// Get the circuit breaker name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if a request should be allowed through.
    ///
    /// `Closed` state uses a read lock — no mutation needed when the failure window
    /// hasn't expired, which is the common case. A write lock is acquired only when
    /// the window needs resetting or a state transition occurs.
    ///
    /// In `HalfOpen` state at most `success_threshold` probes are allowed
    /// concurrently; additional requests are rejected until a probe resolves.
    pub async fn should_allow(&self) -> bool {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Fast path: read lock sufficient when the circuit is Closed and the failure window has not expired.
        {
            let state = self.state.read().await;
            if state.state == CircuitState::Closed {
                // Check whether the failure window has expired — if not, we're done.
                let window_expired = state
                    .last_failure_time
                    .is_some_and(|t| t.elapsed() > self.config.failure_window);
                if !window_expired {
                    return true;
                }
                // Window expired — need to reset counts; fall through to write path.
            }
        }

        // --- Write path for transitions and HalfOpen management ---
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                // Reset failure count now that the window has expired.
                state.failure_count = 0;
                state.last_failure_time = None;
                true
            }
            CircuitState::Open => {
                // Check if timeout has elapsed to transition to half-open
                if let Some(opened_at) = state.opened_at {
                    if opened_at.elapsed() > self.config.timeout {
                        info!(
                            circuit_breaker = %self.config.name,
                            "Circuit transitioning from open to half-open"
                        );
                        state.state = CircuitState::HalfOpen;
                        state.success_count = 0;
                        state.half_open_in_flight = 1; // This probe is the first
                        self.state_transitions.fetch_add(1, Ordering::Relaxed);
                        true
                    } else {
                        self.total_rejections.fetch_add(1, Ordering::Relaxed);
                        debug!(
                            circuit_breaker = %self.config.name,
                            "Circuit is open, rejecting request"
                        );
                        false
                    }
                } else {
                    // Shouldn't happen, but allow anyway
                    true
                }
            }
            CircuitState::HalfOpen => {
                // Limit concurrent half-open probes to success_threshold.
                // This prevents a thundering herd of probes from all succeeding
                // (or all failing) at once and making the transition noisy.
                if state.half_open_in_flight < self.config.success_threshold {
                    state.half_open_in_flight += 1;
                    debug!(
                        circuit_breaker = %self.config.name,
                        in_flight = state.half_open_in_flight,
                        max = self.config.success_threshold,
                        "Admitting half-open probe"
                    );
                    true
                } else {
                    self.total_rejections.fetch_add(1, Ordering::Relaxed);
                    debug!(
                        circuit_breaker = %self.config.name,
                        in_flight = state.half_open_in_flight,
                        "Half-open probe limit reached, rejecting request"
                    );
                    false
                }
            }
        }
    }

    /// Record a successful operation.
    ///
    /// In HalfOpen state, decrements the in-flight probe counter and transitions
    /// to Closed once `success_threshold` probes have succeeded.
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                state.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                state.half_open_in_flight = state.half_open_in_flight.saturating_sub(1);
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    info!(
                        circuit_breaker = %self.config.name,
                        success_count = state.success_count,
                        "Circuit transitioning from half-open to closed"
                    );
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.half_open_in_flight = 0;
                    state.opened_at = None;
                    self.state_transitions.fetch_add(1, Ordering::Relaxed);
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but ignore
            }
        }
    }

    /// Record a failed operation.
    ///
    /// In HalfOpen state, decrements the in-flight probe counter and transitions
    /// back to Open.
    pub async fn record_failure(&self) {
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                state.failure_count += 1;
                state.last_failure_time = Some(Instant::now());

                if state.failure_count >= self.config.failure_threshold {
                    warn!(
                        circuit_breaker = %self.config.name,
                        failure_count = state.failure_count,
                        threshold = self.config.failure_threshold,
                        "Circuit transitioning from closed to open"
                    );
                    state.state = CircuitState::Open;
                    state.opened_at = Some(Instant::now());
                    self.state_transitions.fetch_add(1, Ordering::Relaxed);
                }
            }
            CircuitState::HalfOpen => {
                state.half_open_in_flight = state.half_open_in_flight.saturating_sub(1);
                warn!(
                    circuit_breaker = %self.config.name,
                    "Failure in half-open state, transitioning back to open"
                );
                state.state = CircuitState::Open;
                state.success_count = 0;
                state.opened_at = Some(Instant::now());
                self.state_transitions.fetch_add(1, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Already open, update opened_at to extend timeout
                state.opened_at = Some(Instant::now());
            }
        }
    }

    /// Get metrics for this circuit breaker
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            name: self.config.name.clone(),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_rejections: self.total_rejections.load(Ordering::Relaxed),
            state_transitions: self.state_transitions.load(Ordering::Relaxed),
        }
    }
}

/// Metrics snapshot for a circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub name: String,
    pub total_requests: u64,
    pub total_failures: u64,
    pub total_rejections: u64,
    pub state_transitions: u32,
}

/// Registry of circuit breakers for external services.
/// Intended to be shared as application state (e.g., `web::Data`).
#[derive(Clone)]
pub struct CircuitBreakers {
    pub qdrant: Arc<CircuitBreaker>,
    pub s3: Arc<CircuitBreaker>,
}

impl std::fmt::Debug for CircuitBreakers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreakers")
            .field("qdrant", &self.qdrant.config.name)
            .field("s3", &self.s3.config.name)
            .finish()
    }
}

impl CircuitBreakers {
    pub fn from_env() -> Self {
        Self {
            qdrant: CircuitBreaker::new(CircuitBreakerConfig::from_env_with_prefix(
                "qdrant",
                "QDRANT_CB",
            )),
            s3: CircuitBreaker::new(CircuitBreakerConfig::from_env_with_prefix("s3", "S3_CB")),
        }
    }
}

/// Error returned when circuit breaker rejects a request
#[derive(Debug, Clone)]
pub struct CircuitOpenError {
    pub circuit_name: String,
}

impl std::fmt::Display for CircuitOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Circuit breaker '{}' is open, service unavailable",
            self.circuit_name
        )
    }
}

impl std::error::Error for CircuitOpenError {}

/// Classification of an error for circuit breaker purposes.
///
/// Only **server/service** errors (5xx, timeouts, connection refused) should trip
/// the circuit breaker.  Client errors (4xx, validation failures) represent caller
/// mistakes and should **not** count as service failures.
pub trait CircuitBreakerErrorClass {
    /// Return `true` if this error is a server/service-side fault (should trip circuit).
    /// Return `false` for client errors (4xx, invalid input, etc.).
    fn is_service_error(&self) -> bool;
}

/// Blanket implementation for `anyhow::Error`: treat all errors as service errors
/// unless they clearly look like client errors (the safe/conservative default).
impl CircuitBreakerErrorClass for anyhow::Error {
    fn is_service_error(&self) -> bool {
        let msg = self.to_string().to_lowercase();
        // Explicitly known client-error patterns — do NOT trip the circuit.
        let client_patterns = [
            "400",
            "401",
            "403",
            "404",
            "422",
            "invalid",
            "not found",
            "unauthorized",
            "forbidden",
            "bad request",
        ];
        !client_patterns.iter().any(|p| msg.contains(p))
    }
}

/// Execute an async operation with circuit breaker protection.
///
/// Only records a failure for server/service errors — errors that implement
/// [`CircuitBreakerErrorClass::is_service_error`].
/// Client errors (4xx, validation) do not count toward the failure threshold.
///
/// # Arguments
/// * `circuit` - The circuit breaker to use
/// * `operation` - The async operation to execute
///
/// # Example
/// ```ignore
/// let circuit = CircuitBreaker::new(CircuitBreakerConfig::new("qdrant"));
/// let result = with_circuit_breaker(&circuit, || async {
///     qdrant_client.upsert_points(...).await
/// }).await?;
/// ```
pub async fn with_circuit_breaker<F, Fut, T, E>(
    circuit: &CircuitBreaker,
    operation: F,
) -> Result<T, CircuitBreakerError<E>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: CircuitBreakerErrorClass,
{
    if !circuit.should_allow().await {
        return Err(CircuitBreakerError::CircuitOpen(CircuitOpenError {
            circuit_name: circuit.name().to_string(),
        }));
    }

    match operation().await {
        Ok(result) => {
            circuit.record_success().await;
            Ok(result)
        }
        Err(e) => {
            if e.is_service_error() {
                circuit.record_failure().await;
            }
            Err(CircuitBreakerError::OperationFailed(e))
        }
    }
}

/// Error type for circuit breaker protected operations
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// The circuit breaker rejected the request
    CircuitOpen(CircuitOpenError),
    /// The operation failed
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen(e) => write!(f, "{}", e),
            CircuitBreakerError::OperationFailed(e) => write!(f, "{}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::CircuitOpen(e) => Some(e),
            CircuitBreakerError::OperationFailed(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let config = CircuitBreakerConfig {
            name: "test".to_string(),
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(1),
            failure_window: Duration::from_secs(60),
        };
        let circuit = CircuitBreaker::new(config);

        // Should start closed
        assert_eq!(circuit.state().await, CircuitState::Closed);

        // Should allow requests
        assert!(circuit.should_allow().await);
        circuit.record_success().await;

        // Still closed
        assert_eq!(circuit.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            name: "test".to_string(),
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(1),
            failure_window: Duration::from_secs(60),
        };
        let circuit = CircuitBreaker::new(config);

        // Record failures
        for _ in 0..3 {
            circuit.should_allow().await;
            circuit.record_failure().await;
        }

        // Should be open now
        assert_eq!(circuit.state().await, CircuitState::Open);

        // Should reject requests
        assert!(!circuit.should_allow().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            name: "test".to_string(),
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(50),
            failure_window: Duration::from_secs(60),
        };
        let circuit = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..2 {
            circuit.should_allow().await;
            circuit.record_failure().await;
        }
        assert_eq!(circuit.state().await, CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Should transition to half-open
        assert!(circuit.should_allow().await);
        assert_eq!(circuit.state().await, CircuitState::HalfOpen);
    }
}
