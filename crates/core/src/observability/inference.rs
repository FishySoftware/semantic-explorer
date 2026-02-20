use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_embed_request(model: &str, item_count: u64, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.inference_embed_requests_total.add(
        1,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.inference_embed_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if success {
        metrics
            .inference_embed_items_total
            .add(item_count, &[KeyValue::new("model", model.to_string())]);

        if item_count > 0 {
            let per_item_duration = duration_secs / item_count as f64;
            metrics.inference_embed_per_item_duration.record(
                per_item_duration,
                &[KeyValue::new("model", model.to_string())],
            );
        }
    }
}

pub fn record_rerank_request(model: &str, document_count: u64, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.inference_rerank_requests_total.add(
        1,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.inference_rerank_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if success {
        metrics
            .inference_rerank_documents_total
            .add(document_count, &[KeyValue::new("model", model.to_string())]);
    }
}

pub fn record_llm_request(model: &str, tokens_generated: u64, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.inference_llm_requests_total.add(
        1,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.inference_llm_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if success {
        metrics.inference_llm_tokens_generated.add(
            tokens_generated,
            &[KeyValue::new("model", model.to_string())],
        );

        if duration_secs > 0.0 {
            let tokens_per_sec = tokens_generated as f64 / duration_secs;
            metrics
                .inference_llm_tokens_per_second
                .record(tokens_per_sec, &[KeyValue::new("model", model.to_string())]);
        }
    }
}

pub fn record_embedding_batch(model: &str, duration_secs: f64, chunk_count: usize, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.embedding_per_chunk_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
            KeyValue::new("batch", "true"),
            KeyValue::new("chunk_count", chunk_count.to_string()),
        ],
    );
}

pub fn record_embedding_per_chunk(model: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.embedding_per_chunk_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_llm_response(model: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.llm_response_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_embedding_session_metrics(model_id: &str, request_count: u64, age_seconds: f64) {
    let metrics = get_metrics();

    metrics.embedding_session_request_count.record(
        request_count as f64,
        &[KeyValue::new("model", model_id.to_string())],
    );

    metrics
        .embedding_session_age_seconds
        .record(age_seconds, &[KeyValue::new("model", model_id.to_string())]);
}

pub fn record_embedding_session_reset(model_id: &str, reason: &str) {
    let metrics = get_metrics();

    metrics.embedding_session_resets_total.add(
        1,
        &[
            KeyValue::new("model", model_id.to_string()),
            KeyValue::new("reason", reason.to_string()),
        ],
    );
}

pub fn init_embedding_session_reset_metric(model_id: &str) {
    let metrics = get_metrics();

    metrics.embedding_session_resets_total.add(
        0,
        &[
            KeyValue::new("model", model_id.to_string()),
            KeyValue::new("reason", "none".to_string()),
        ],
    );
}
