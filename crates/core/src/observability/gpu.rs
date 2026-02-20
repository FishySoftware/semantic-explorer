use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_gpu_metrics(
    device_index: u32,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    gpu_utilization: u32,
    memory_utilization: u32,
) {
    let metrics = get_metrics();
    let device_label = device_index.to_string();

    metrics.gpu_memory_used_bytes.record(
        memory_used_bytes as f64,
        &[KeyValue::new("device", device_label.clone())],
    );

    metrics.gpu_memory_total_bytes.record(
        memory_total_bytes as f64,
        &[KeyValue::new("device", device_label.clone())],
    );

    metrics.gpu_utilization_percent.record(
        gpu_utilization as f64,
        &[KeyValue::new("device", device_label.clone())],
    );

    metrics.gpu_memory_utilization_percent.record(
        memory_utilization as f64,
        &[KeyValue::new("device", device_label)],
    );
}

pub mod gpu_monitor {
    use nvml_wrapper::Nvml;
    use std::sync::OnceLock;
    use std::time::Duration;
    use tracing::{debug, info, warn};

    static NVML: OnceLock<Option<Nvml>> = OnceLock::new();

    pub fn init() -> bool {
        let nvml = NVML.get_or_init(|| match Nvml::init() {
            Ok(nvml) => {
                info!("NVML initialized successfully for GPU monitoring");
                Some(nvml)
            }
            Err(e) => {
                warn!(
                    "NVML initialization failed (no GPU or drivers not installed): {}",
                    e
                );
                None
            }
        });
        nvml.is_some()
    }

    pub fn get_memory_info(device_index: u32) -> Option<(u64, u64)> {
        let nvml = NVML.get()?.as_ref()?;
        let device = nvml.device_by_index(device_index).ok()?;
        let memory = device.memory_info().ok()?;
        Some((memory.used, memory.total))
    }

    pub fn get_utilization(device_index: u32) -> Option<(u32, u32)> {
        let nvml = NVML.get()?.as_ref()?;
        let device = nvml.device_by_index(device_index).ok()?;
        let utilization = device.utilization_rates().ok()?;
        Some((utilization.gpu, utilization.memory))
    }

    pub fn device_count() -> u32 {
        NVML.get()
            .and_then(|nvml| nvml.as_ref())
            .and_then(|nvml| nvml.device_count().ok())
            .unwrap_or(0)
    }

    pub fn is_memory_pressure_high(threshold_percent: f64) -> bool {
        let count = device_count();
        for i in 0..count {
            if let Some((used, total)) = get_memory_info(i) {
                let utilization = (used as f64 / total as f64) * 100.0;
                if utilization > threshold_percent {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_compute_pressure_high(threshold_percent: f64) -> bool {
        let count = device_count();
        for i in 0..count {
            if let Some((gpu_util, _mem_util)) = get_utilization(i)
                && gpu_util as f64 > threshold_percent
            {
                return true;
            }
        }
        false
    }

    pub fn is_gpu_under_pressure(threshold_percent: f64) -> bool {
        is_memory_pressure_high(threshold_percent) || is_compute_pressure_high(threshold_percent)
    }

    pub fn get_vram_per_device() -> Vec<(u32, u64)> {
        let count = device_count();
        (0..count)
            .filter_map(|i| get_memory_info(i).map(|(_used, total)| (i, total)))
            .collect()
    }

    pub fn get_min_device_vram() -> Option<u64> {
        let devices = get_vram_per_device();
        if devices.is_empty() {
            return None;
        }
        let min = devices.iter().min_by_key(|(_idx, vram)| *vram).unwrap();
        info!(
            device_count = devices.len(),
            min_vram_device = min.0,
            min_vram_mb = min.1 / (1024 * 1024),
            devices = ?devices.iter().map(|(i, v)| format!("GPU{}={}MB", i, v / (1024 * 1024))).collect::<Vec<_>>(),
            "Detected GPU VRAM across all visible devices"
        );
        Some(min.1)
    }

    pub fn get_memory_utilization_percent(device_index: u32) -> Option<f64> {
        let (used, total) = get_memory_info(device_index)?;
        Some((used as f64 / total as f64) * 100.0)
    }

    pub fn collect_metrics() {
        let count = device_count();
        for i in 0..count {
            if let (Some((used, total)), Some((gpu_util, mem_util))) =
                (get_memory_info(i), get_utilization(i))
            {
                super::record_gpu_metrics(i, used, total, gpu_util, mem_util);
                debug!(
                    device = i,
                    used_mb = used / 1024 / 1024,
                    total_mb = total / 1024 / 1024,
                    gpu_util = gpu_util,
                    mem_util = mem_util,
                    "GPU metrics collected"
                );
            }
        }
    }

    pub fn spawn_metrics_collector(interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if !init() {
                warn!("GPU monitoring disabled - NVML not available");
                return;
            }

            info!(
                interval_secs = interval.as_secs(),
                device_count = device_count(),
                "Starting GPU metrics collector"
            );

            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                collect_metrics();
            }
        })
    }
}
