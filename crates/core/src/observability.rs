use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_histogram_vec_with_registry, CounterVec, GaugeVec, HistogramVec, Opts, Registry,
};
use std::sync::OnceLock;

pub struct Metrics {
    pub database_query_total: CounterVec,
    pub database_query_duration: HistogramVec,
    pub database_connection_pool_size: GaugeVec,
    pub database_connection_pool_active: GaugeVec,
    pub database_connection_pool_idle: GaugeVec,

    pub storage_operations_total: CounterVec,
    pub storage_operation_duration: HistogramVec,
    pub storage_file_size_bytes: HistogramVec,

    pub worker_jobs_total: CounterVec,
    pub worker_job_duration: HistogramVec,
    pub worker_job_chunks: HistogramVec,
    pub worker_job_file_size: HistogramVec,
}

impl Metrics {
    fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let database_query_total = register_counter_vec_with_registry!(
            Opts::new("database_query_total", "Total number of database queries"),
            &["operation", "table", "status"],
            registry
        )?;

        let database_query_duration = register_histogram_vec_with_registry!(
            "database_query_duration_seconds",
            "Duration of database queries in seconds",
            &["operation", "table", "status"],
            registry
        )?;

        let database_connection_pool_size = register_gauge_vec_with_registry!(
            Opts::new(
                "database_connection_pool_size",
                "Total size of the database connection pool"
            ),
            &["_"],
            registry
        )?;

        let database_connection_pool_active = register_gauge_vec_with_registry!(
            Opts::new(
                "database_connection_pool_active",
                "Number of active database connections"
            ),
            &["_"],
            registry
        )?;

        let database_connection_pool_idle = register_gauge_vec_with_registry!(
            Opts::new(
                "database_connection_pool_idle",
                "Number of idle database connections"
            ),
            &["_"],
            registry
        )?;

        let storage_operations_total = register_counter_vec_with_registry!(
            Opts::new(
                "storage_operations_total",
                "Total number of storage operations"
            ),
            &["operation", "status"],
            registry
        )?;

        let storage_operation_duration = register_histogram_vec_with_registry!(
            "storage_operation_duration_seconds",
            "Duration of storage operations in seconds",
            &["operation", "status"],
            registry
        )?;

        let storage_file_size_bytes = register_histogram_vec_with_registry!(
            "storage_file_size_bytes",
            "Size of files in storage operations",
            &["operation"],
            vec![
                1024.0,
                10240.0,
                102400.0,
                1024000.0,
                10240000.0,
                102400000.0
            ],
            registry
        )?;

        let worker_jobs_total = register_counter_vec_with_registry!(
            Opts::new("worker_jobs_total", "Total number of worker jobs processed"),
            &["worker", "status"],
            registry
        )?;

        let worker_job_duration = register_histogram_vec_with_registry!(
            "worker_job_duration_seconds",
            "Duration of worker jobs in seconds",
            &["worker", "status"],
            registry
        )?;

        let worker_job_chunks = register_histogram_vec_with_registry!(
            "worker_job_chunks",
            "Number of chunks processed in worker jobs",
            &["worker", "status"],
            vec![1.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0],
            registry
        )?;

        let worker_job_file_size = register_histogram_vec_with_registry!(
            "worker_job_file_size_bytes",
            "Size of files processed in worker jobs",
            &["worker", "status"],
            vec![
                1024.0,
                10240.0,
                102400.0,
                1024000.0,
                10240000.0,
                102400000.0,
                1024000000.0
            ],
            registry
        )?;

        Ok(Self {
            database_query_total,
            database_query_duration,
            database_connection_pool_size,
            database_connection_pool_active,
            database_connection_pool_idle,
            storage_operations_total,
            storage_operation_duration,
            storage_file_size_bytes,
            worker_jobs_total,
            worker_job_duration,
            worker_job_chunks,
            worker_job_file_size,
        })
    }
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

pub fn init_metrics(registry: &Registry) -> Result<(), prometheus::Error> {
    let metrics = Metrics::new(registry)?;
    METRICS
        .set(metrics)
        .map_err(|_| prometheus::Error::Msg("Metrics already initialized".into()))?;
    Ok(())
}

pub fn get_metrics() -> &'static Metrics {
    METRICS.get().expect("Metrics not initialized")
}

pub fn record_database_query(operation: &str, table: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics
        .database_query_total
        .with_label_values(&[operation, table, status])
        .inc();

    metrics
        .database_query_duration
        .with_label_values(&[operation, table, status])
        .observe(duration_secs);
}

pub fn record_storage_operation(
    operation: &str,
    duration_secs: f64,
    file_size_bytes: Option<u64>,
    success: bool,
) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics
        .storage_operations_total
        .with_label_values(&[operation, status])
        .inc();

    metrics
        .storage_operation_duration
        .with_label_values(&[operation, status])
        .observe(duration_secs);

    if let Some(size) = file_size_bytes {
        metrics
            .storage_file_size_bytes
            .with_label_values(&[operation])
            .observe(size as f64);
    }
}

pub fn record_worker_job(worker: &str, duration_secs: f64, status: &str) {
    let metrics = get_metrics();

    metrics
        .worker_jobs_total
        .with_label_values(&[worker, status])
        .inc();

    metrics
        .worker_job_duration
        .with_label_values(&[worker, status])
        .observe(duration_secs);
}

pub fn record_worker_job_with_metrics(
    worker: &str,
    duration_secs: f64,
    status: &str,
    chunk_count: Option<usize>,
    file_size_bytes: Option<u64>,
) {
    let metrics = get_metrics();

    metrics
        .worker_jobs_total
        .with_label_values(&[worker, status])
        .inc();

    metrics
        .worker_job_duration
        .with_label_values(&[worker, status])
        .observe(duration_secs);

    if let Some(chunks) = chunk_count {
        metrics
            .worker_job_chunks
            .with_label_values(&[worker, status])
            .observe(chunks as f64);
    }

    if let Some(size) = file_size_bytes {
        metrics
            .worker_job_file_size
            .with_label_values(&[worker, status])
            .observe(size as f64);
    }
}

pub fn update_database_pool_stats(size: u64, active: u64, idle: u64) {
    let metrics = get_metrics();

    metrics
        .database_connection_pool_size
        .with_label_values(&[""])
        .set(size as f64);

    metrics
        .database_connection_pool_active
        .with_label_values(&[""])
        .set(active as f64);

    metrics
        .database_connection_pool_idle
        .with_label_values(&[""])
        .set(idle as f64);
}
