// Phase 5.5: Performance Profiling and Benchmarking Module
// Purpose: Provide utilities for memory profiling, latency measurement, and performance analysis

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Performance metric snapshot
#[derive(Debug, Clone)]
pub struct PerformanceMetric {
    pub name: String,
    pub duration_ms: f64,
    pub memory_bytes: Option<u64>,
    pub success: bool,
}

/// Latency percentile calculator
#[derive(Debug, Clone)]
pub struct LatencyTracker {
    measurements: Arc<std::sync::Mutex<Vec<f64>>>,
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self {
            measurements: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Record a latency measurement in milliseconds
    pub fn record(&self, duration_ms: f64) {
        if let Ok(mut measurements) = self.measurements.lock() {
            measurements.push(duration_ms);
        }
    }

    /// Get percentile value
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if let Ok(mut measurements) = self.measurements.lock() {
            if measurements.is_empty() {
                return None;
            }
            measurements.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let index = ((p / 100.0) * measurements.len() as f64).ceil() as usize;
            let index = std::cmp::min(index.saturating_sub(1), measurements.len() - 1);
            Some(measurements[index])
        } else {
            None
        }
    }

    /// Get statistics
    pub fn stats(&self) -> LatencyStats {
        if let Ok(measurements) = self.measurements.lock() {
            if measurements.is_empty() {
                return LatencyStats::default();
            }

            let min = measurements.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = measurements
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let avg = measurements.iter().sum::<f64>() / measurements.len() as f64;

            let mut sorted = measurements.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let p50 = sorted[sorted.len() / 2];
            let p95 = sorted[((95.0 / 100.0) * sorted.len() as f64) as usize];
            let p99 = sorted[((99.0 / 100.0) * sorted.len() as f64) as usize];

            LatencyStats {
                min,
                max,
                avg,
                p50,
                p95,
                p99,
                count: measurements.len(),
            }
        } else {
            LatencyStats::default()
        }
    }

    pub fn count(&self) -> usize {
        self.measurements.lock().map(|m| m.len()).unwrap_or(0)
    }
}

/// Latency statistics
#[derive(Debug, Clone, Default)]
pub struct LatencyStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub count: usize,
}

impl std::fmt::Display for LatencyStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Latency Stats (n={}): min={:.2}ms, avg={:.2}ms, p50={:.2}ms, p95={:.2}ms, p99={:.2}ms, max={:.2}ms",
            self.count, self.min, self.avg, self.p50, self.p95, self.p99, self.max
        )
    }
}

/// Simple timer for measuring operation duration
pub struct Timer {
    start: Instant,
    operation_name: String,
}

impl Timer {
    pub fn start(operation_name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            operation_name: operation_name.into(),
        }
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    pub fn stop(self) -> TimerResult {
        let duration_ms = self.elapsed_ms();
        TimerResult {
            operation_name: self.operation_name,
            duration_ms,
        }
    }
}

pub struct TimerResult {
    pub operation_name: String,
    pub duration_ms: f64,
}

impl std::fmt::Display for TimerResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:.2}ms", self.operation_name, self.duration_ms)
    }
}

/// Thread-safe counter for throughput measurement
pub struct ThroughputCounter {
    total_operations: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
}

impl ThroughputCounter {
    pub fn new() -> Self {
        Self {
            total_operations: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn record_operation(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_bytes(&self, bytes: u64) {
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn get_operations(&self) -> u64 {
        self.total_operations.load(Ordering::Relaxed)
    }

    pub fn get_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    pub fn get_throughput_ops_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs > 0.0 {
            self.get_operations() as f64 / duration_secs
        } else {
            0.0
        }
    }

    pub fn get_throughput_mb_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs > 0.0 {
            (self.get_bytes() as f64 / (1024.0 * 1024.0)) / duration_secs
        } else {
            0.0
        }
    }
}

impl Clone for ThroughputCounter {
    fn clone(&self) -> Self {
        Self {
            total_operations: Arc::clone(&self.total_operations),
            total_bytes: Arc::clone(&self.total_bytes),
        }
    }
}

impl Default for ThroughputCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_tracker() {
        let tracker = LatencyTracker::new();
        tracker.record(10.0);
        tracker.record(20.0);
        tracker.record(30.0);
        tracker.record(40.0);
        tracker.record(50.0);

        let stats = tracker.stats();
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 50.0);
        assert_eq!(stats.count, 5);
    }

    #[test]
    fn test_timer() {
        let timer = Timer::start("test_operation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let result = timer.stop();
        assert!(result.duration_ms >= 10.0);
    }

    #[test]
    fn test_throughput_counter() {
        let counter = ThroughputCounter::new();
        counter.record_operation();
        counter.record_operation();
        counter.record_bytes(1024);

        assert_eq!(counter.get_operations(), 2);
        assert_eq!(counter.get_bytes(), 1024);

        let ops_per_sec = counter.get_throughput_ops_per_sec(1.0);
        assert_eq!(ops_per_sec, 2.0);
    }
}
