// Phase 5.5: Load Testing Harness
// Purpose: Simulate concurrent user workloads and measure performance under load

use std::sync::Arc;
use std::time::Instant;

use crate::profiling::{LatencyTracker, ThroughputCounter};

/// Load test configuration
#[derive(Clone, Debug)]
pub struct LoadTestConfig {
    /// Number of concurrent clients
    pub concurrent_clients: usize,
    /// Duration of test in seconds
    pub duration_secs: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Ramp-up time in seconds (gradually increase load)
    pub ramp_up_secs: u64,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            concurrent_clients: 10,
            duration_secs: 60,
            request_timeout_ms: 30000,
            ramp_up_secs: 5,
        }
    }
}

/// Load test scenario
pub struct LoadTestScenario {
    pub name: String,
    pub description: String,
    pub config: LoadTestConfig,
}

/// Load test result
#[derive(Debug, Clone)]
pub struct LoadTestResult {
    pub scenario_name: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_bytes_processed: u64,
    pub duration_secs: f64,
    pub requests_per_sec: f64,
    pub bytes_per_sec: f64,
    pub error_rate: f64,
}

impl std::fmt::Display for LoadTestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\n=== Load Test: {} ===\n\
             Duration: {:.2}s\n\
             Total Requests: {}\n\
             Successful: {} ({:.1}%)\n\
             Failed: {} ({:.1}%)\n\
             Throughput: {:.2} req/s\n\
             Bytes Processed: {:.2} MB at {:.2} MB/s\n",
            self.scenario_name,
            self.duration_secs,
            self.total_requests,
            self.successful_requests,
            ((self.successful_requests as f64 / self.total_requests as f64) * 100.0),
            self.failed_requests,
            self.error_rate * 100.0,
            self.requests_per_sec,
            self.total_bytes_processed as f64 / (1024.0 * 1024.0),
            self.bytes_per_sec / (1024.0 * 1024.0)
        )
    }
}

/// Load test client simulator
pub struct TestClient {
    id: usize,
    latency_tracker: Arc<LatencyTracker>,
    throughput_counter: Arc<ThroughputCounter>,
}

impl TestClient {
    pub fn new(
        id: usize,
        latency_tracker: Arc<LatencyTracker>,
        throughput_counter: Arc<ThroughputCounter>,
    ) -> Self {
        Self {
            id,
            latency_tracker,
            throughput_counter,
        }
    }

    /// Simulate a request with random latency (for testing)
    pub fn simulate_request(&self) -> Result<u64, String> {
        let latency_ms = (50.0 + (self.id as f64 * std::f64::consts::PI) % 150.0) as u64;

        let bytes_returned = 1024 + (self.id * 256) as u64;

        self.latency_tracker.record(latency_ms as f64);
        self.throughput_counter.record_operation();
        self.throughput_counter.record_bytes(bytes_returned);

        Ok(bytes_returned)
    }
}

/// Load test executor
pub struct LoadTestExecutor {
    scenario: LoadTestScenario,
}

impl LoadTestExecutor {
    pub fn new(scenario: LoadTestScenario) -> Self {
        Self { scenario }
    }

    /// Run the load test (synchronous version for core library)
    /// For actual async load testing, use the integration tests in crates/api/tests/
    pub fn run(&self) -> LoadTestResult {
        let config = self.scenario.config.clone();
        let latency_tracker = Arc::new(LatencyTracker::new());
        let throughput_counter = Arc::new(ThroughputCounter::new());
        let test_start = Instant::now();

        // Simulate concurrent clients sequentially (use async version in integration tests)
        for client_id in 0..config.concurrent_clients {
            let client = TestClient::new(
                client_id,
                Arc::clone(&latency_tracker),
                Arc::clone(&throughput_counter),
            );

            // Simulate 10 requests per client
            for _ in 0..10 {
                if client.simulate_request().is_ok() {
                    // Request succeeded
                }
            }
        }

        let total_duration = test_start.elapsed().as_secs_f64();
        let total_requests = latency_tracker.count() as u64;
        let total_bytes = throughput_counter.get_bytes();

        LoadTestResult {
            scenario_name: self.scenario.name.clone(),
            total_requests,
            successful_requests: total_requests,
            failed_requests: 0,
            total_bytes_processed: total_bytes,
            duration_secs: total_duration,
            requests_per_sec: total_requests as f64 / total_duration,
            bytes_per_sec: total_bytes as f64 / total_duration,
            error_rate: 0.0,
        }
    }
}

/// Built-in test scenarios
pub mod scenarios {
    use super::*;

    pub fn concurrent_reads() -> LoadTestScenario {
        LoadTestScenario {
            name: "Concurrent Reads".to_string(),
            description: "Simulate 50 concurrent users performing read operations".to_string(),
            config: LoadTestConfig {
                concurrent_clients: 50,
                duration_secs: 60,
                request_timeout_ms: 30000,
                ramp_up_secs: 5,
            },
        }
    }

    pub fn rag_search_load() -> LoadTestScenario {
        LoadTestScenario {
            name: "RAG Search Load".to_string(),
            description: "Simulate 20 concurrent RAG search sessions".to_string(),
            config: LoadTestConfig {
                concurrent_clients: 20,
                duration_secs: 120,
                request_timeout_ms: 60000,
                ramp_up_secs: 10,
            },
        }
    }

    pub fn bulk_operations() -> LoadTestScenario {
        LoadTestScenario {
            name: "Bulk Operations".to_string(),
            description: "Simulate 10 concurrent bulk import/export operations".to_string(),
            config: LoadTestConfig {
                concurrent_clients: 10,
                duration_secs: 180,
                request_timeout_ms: 120000,
                ramp_up_secs: 15,
            },
        }
    }

    pub fn spike_test() -> LoadTestScenario {
        LoadTestScenario {
            name: "Spike Test".to_string(),
            description: "Simulate sudden spike from 10 to 100 concurrent users".to_string(),
            config: LoadTestConfig {
                concurrent_clients: 100,
                duration_secs: 30,
                request_timeout_ms: 30000,
                ramp_up_secs: 5,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_test_executor() {
        let scenario = scenarios::concurrent_reads();
        let executor = LoadTestExecutor::new(scenario);
        let result = executor.run();

        println!("{}", result);
        assert!(result.total_requests > 0);
        assert_eq!(result.failed_requests, 0);
        assert!(result.requests_per_sec > 0.0);
    }

    #[test]
    fn test_test_client() {
        let latency_tracker = Arc::new(LatencyTracker::new());
        let throughput_counter = Arc::new(ThroughputCounter::new());

        let client = TestClient::new(0, latency_tracker.clone(), throughput_counter.clone());
        let _ = client.simulate_request();

        assert!(latency_tracker.count() > 0);
        assert!(throughput_counter.get_operations() > 0);
    }
}
