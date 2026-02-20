//! Adaptive concurrency control for NATS workers.
//!
//! Wraps a Tokio [`Semaphore`] with dynamic limit adjustment based on
//! downstream pressure signals (e.g., 503 responses from inference APIs).
//!
//! The controller starts at `max_concurrent_jobs` permits and scales down
//! when downstream services are overloaded, then scales back up when
//! pressure clears. This prevents workers from overwhelming inference APIs
//! while maximising throughput when capacity is available.
//!
//! ## How it works
//!
//! - **Scale-down**: When `record_downstream_pressure()` is called (e.g., on 503),
//!   the effective limit is halved (min 1). Excess permits are consumed to enforce
//!   the lower limit.
//! - **Scale-up**: A background task periodically checks if pressure has cleared
//!   and gradually increases the limit back toward the configured max.
//! - **Workers** acquire permits via `acquire()` — identical to bare Semaphore usage.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{info, warn};

/// Adaptive concurrency controller for worker message processing.
///
/// Drop the returned `Arc<AdaptiveConcurrency>` to stop the background scaler.
#[derive(Debug)]
pub struct AdaptiveConcurrency {
    /// The underlying semaphore — always created with `max_limit` permits.
    semaphore: Arc<Semaphore>,
    /// Maximum permits (configured at startup from MAX_CONCURRENT_JOBS).
    max_limit: usize,
    /// Current effective limit (may be < max_limit during backpressure).
    effective_limit: AtomicUsize,
    /// True when downstream services are signalling overload.
    downstream_pressure: AtomicBool,
    /// Number of permits we've "consumed" to enforce a lower effective limit.
    /// These are held as forgotten permits — they are released when we scale back up.
    held_permits: AtomicUsize,
}

impl AdaptiveConcurrency {
    /// Create a new adaptive concurrency controller.
    ///
    /// `max_concurrent_jobs` is the starting (and ceiling) number of permits.
    pub fn new(max_concurrent_jobs: usize) -> Arc<Self> {
        let max = max_concurrent_jobs.max(1);
        let ac = Arc::new(Self {
            semaphore: Arc::new(Semaphore::new(max)),
            max_limit: max,
            effective_limit: AtomicUsize::new(max),
            downstream_pressure: AtomicBool::new(false),
            held_permits: AtomicUsize::new(0),
        });

        // Start background scaler
        let ac_clone = Arc::clone(&ac);
        tokio::spawn(async move {
            ac_clone.run_scaler().await;
        });

        info!(
            max_concurrent_jobs = max,
            "Adaptive concurrency controller started"
        );
        ac
    }

    /// Acquire a permit. Blocks until one is available (respects effective limit).
    ///
    /// This is the main entry point for workers — use exactly like `Semaphore::acquire`.
    pub async fn acquire(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::AcquireError> {
        self.semaphore.clone().acquire_owned().await
    }

    /// Get a clone of the inner semaphore (for use with `acquire_owned` + timeout).
    pub fn semaphore(&self) -> Arc<Semaphore> {
        Arc::clone(&self.semaphore)
    }

    /// Signal that downstream services are under pressure (e.g., received a 503).
    ///
    /// This will halve the effective concurrency limit on the next scaler tick.
    /// Safe to call from multiple tasks concurrently — coalesced into one scale-down.
    pub fn record_downstream_pressure(&self) {
        if !self.downstream_pressure.swap(true, Ordering::SeqCst) {
            let current = self.effective_limit.load(Ordering::SeqCst);
            warn!(
                effective_limit = current,
                "Downstream pressure detected, will scale down concurrency"
            );
        }
    }

    /// Signal that downstream is healthy again (e.g., a request succeeded).
    pub fn record_downstream_success(&self) {
        self.downstream_pressure.store(false, Ordering::SeqCst);
    }

    /// Check if downstream is currently under pressure.
    pub fn is_downstream_pressured(&self) -> bool {
        self.downstream_pressure.load(Ordering::SeqCst)
    }

    /// Current effective concurrency limit.
    pub fn effective_limit(&self) -> usize {
        self.effective_limit.load(Ordering::SeqCst)
    }

    /// Maximum configured concurrency limit.
    pub fn max_limit(&self) -> usize {
        self.max_limit
    }

    /// Available permits right now.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Background task that adjusts concurrency based on pressure signals.
    ///
    /// Runs every `ADAPTIVE_CONCURRENCY_SCALING_INTERVAL_SECS` seconds (default 5):
    /// - If pressure is active: halve the effective limit (min 1)
    /// - If pressure has cleared for 2 consecutive ticks: increase by 1 toward max
    async fn run_scaler(&self) {
        let scaling_interval_secs: u64 =
            std::env::var("ADAPTIVE_CONCURRENCY_SCALING_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5);
        let mut ticker = tokio::time::interval(Duration::from_secs(scaling_interval_secs));
        let mut ticks_without_pressure: u32 = 0;

        loop {
            ticker.tick().await;

            let pressure = self.downstream_pressure.load(Ordering::SeqCst);
            let current_limit = self.effective_limit.load(Ordering::SeqCst);

            if pressure {
                ticks_without_pressure = 0;

                // Scale down: halve the limit (min 1)
                let new_limit = (current_limit / 2).max(1);
                if new_limit < current_limit {
                    self.scale_down_to(new_limit).await;
                }
            } else {
                ticks_without_pressure += 1;

                // Only scale up after 2 consecutive ticks (10s) without pressure
                if ticks_without_pressure >= 2 && current_limit < self.max_limit {
                    self.scale_up_by_one().await;
                }
            }
        }
    }

    /// Reduce the effective limit by consuming excess permits from the semaphore.
    async fn scale_down_to(&self, new_limit: usize) {
        let current_limit = self.effective_limit.load(Ordering::SeqCst);
        if new_limit >= current_limit {
            return;
        }

        let permits_to_consume = current_limit - new_limit;
        let mut consumed = 0;

        for _ in 0..permits_to_consume {
            // Try to acquire a permit to "hold" it (non-blocking).
            // If we can't get one, the workers are using them — that's fine,
            // they'll naturally hit the lower limit as they finish.
            match self.semaphore.try_acquire() {
                Ok(permit) => {
                    // Forget the permit so it's not returned to the pool
                    permit.forget();
                    consumed += 1;
                }
                Err(_) => {
                    // All permits in use — workers will naturally be constrained
                    // as they complete and the available count stays low
                    break;
                }
            }
        }

        self.held_permits.fetch_add(consumed, Ordering::SeqCst);
        self.effective_limit.store(new_limit, Ordering::SeqCst);

        warn!(
            previous_limit = current_limit,
            new_limit = new_limit,
            permits_consumed = consumed,
            "Scaled down concurrency due to downstream pressure"
        );
    }

    /// Increase the effective limit by one, releasing a previously held permit.
    async fn scale_up_by_one(&self) {
        let current_limit = self.effective_limit.load(Ordering::SeqCst);
        if current_limit >= self.max_limit {
            return;
        }

        let held = self.held_permits.load(Ordering::SeqCst);
        if held == 0 {
            // No held permits to release — we may have been constrained without
            // actually consuming permits (workers were already using them all).
            // Just bump the limit up; the semaphore already has the right capacity.
            self.effective_limit
                .store(current_limit + 1, Ordering::SeqCst);
            return;
        }

        // Release one held permit back to the semaphore
        self.semaphore.add_permits(1);
        self.held_permits.fetch_sub(1, Ordering::SeqCst);

        let new_limit = current_limit + 1;
        self.effective_limit.store(new_limit, Ordering::SeqCst);

        info!(
            previous_limit = current_limit,
            new_limit = new_limit,
            remaining_held = held - 1,
            "Scaled up concurrency, downstream pressure cleared"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_state() {
        let ac = AdaptiveConcurrency::new(10);
        assert_eq!(ac.effective_limit(), 10);
        assert_eq!(ac.max_limit(), 10);
        assert_eq!(ac.available_permits(), 10);
        assert!(!ac.is_downstream_pressured());
    }

    #[tokio::test]
    async fn test_min_one_permit() {
        let ac = AdaptiveConcurrency::new(0);
        assert_eq!(ac.effective_limit(), 1);
        assert_eq!(ac.max_limit(), 1);
    }

    #[tokio::test]
    async fn test_pressure_flag() {
        let ac = AdaptiveConcurrency::new(10);
        assert!(!ac.is_downstream_pressured());

        ac.record_downstream_pressure();
        assert!(ac.is_downstream_pressured());

        ac.record_downstream_success();
        assert!(!ac.is_downstream_pressured());
    }

    #[tokio::test]
    async fn test_acquire_returns_permit() {
        let ac = AdaptiveConcurrency::new(2);
        assert_eq!(ac.available_permits(), 2);

        let _p1 = ac.acquire().await.unwrap();
        assert_eq!(ac.available_permits(), 1);

        let _p2 = ac.acquire().await.unwrap();
        assert_eq!(ac.available_permits(), 0);

        drop(_p1);
        assert_eq!(ac.available_permits(), 1);
    }
}
