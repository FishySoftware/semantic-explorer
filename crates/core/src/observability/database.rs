use super::get_metrics;

pub fn update_database_pool_stats(size: u64, idle: u64, max: u64) {
    let metrics = get_metrics();

    metrics
        .database_connection_pool_size
        .record(size as f64, &[]);

    metrics
        .database_connection_pool_idle
        .record(idle as f64, &[]);

    metrics.database_connection_pool_max.record(max as f64, &[]);
}

pub fn update_embedded_datasets_count(count: u64) {
    let metrics = get_metrics();
    metrics.embedded_datasets_active.record(count as f64, &[]);
}
