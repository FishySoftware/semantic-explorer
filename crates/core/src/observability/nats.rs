use opentelemetry::KeyValue;

use super::get_metrics;

pub fn update_nats_stream_stats(stream_name: &str, messages: u64, bytes: u64) {
    let metrics = get_metrics();
    metrics.nats_stream_messages.record(
        messages as f64,
        &[KeyValue::new("stream", stream_name.to_string())],
    );
    metrics.nats_stream_bytes.record(
        bytes as f64,
        &[KeyValue::new("stream", stream_name.to_string())],
    );
}

pub fn update_nats_consumer_stats(
    stream_name: &str,
    consumer_name: &str,
    pending: u64,
    ack_pending: u64,
) {
    let metrics = get_metrics();
    metrics.nats_consumer_pending.record(
        pending as f64,
        &[
            KeyValue::new("stream", stream_name.to_string()),
            KeyValue::new("consumer", consumer_name.to_string()),
        ],
    );
    metrics.nats_consumer_ack_pending.record(
        ack_pending as f64,
        &[
            KeyValue::new("stream", stream_name.to_string()),
            KeyValue::new("consumer", consumer_name.to_string()),
        ],
    );
}

pub fn record_dlq_message(transform_type: &str, reason: &str) {
    let metrics = get_metrics();
    metrics.dlq_messages_total.add(
        1,
        &[
            KeyValue::new("transform_type", transform_type.to_string()),
            KeyValue::new("reason", reason.to_string()),
        ],
    );
}
