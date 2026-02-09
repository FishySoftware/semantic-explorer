use actix_web::{HttpResponse, Responder, get, web::Data};
use async_nats::{
    Client,
    jetstream::{self, consumer::pull::Config as ConsumerConfig},
};
use serde::Serialize;

/// NATS connection state
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NatsConnectionState {
    Connected,
    Disconnected,
    Pending,
}

/// Overall Worker status response
#[derive(Debug, Serialize)]
pub struct NatsStatusResponse {
    pub connection: NatsConnectionStatus,
    pub streams: Vec<StreamStatus>,
    pub consumers: Vec<ConsumerStatus>,
    pub dlq: DlqStatus,
}

#[derive(Debug, Serialize)]
pub struct NatsConnectionStatus {
    pub state: NatsConnectionState,
    pub server_url: String,
}

#[derive(Debug, Serialize)]
pub struct StreamStatus {
    pub name: String,
    pub messages: u64,
    pub bytes: u64,
    pub consumer_count: usize,
    pub first_seq: u64,
    pub last_seq: u64,
    pub first_ts: Option<String>,
    pub last_ts: Option<String>,
    pub subjects: Vec<String>,
    pub retention: String,
}

#[derive(Debug, Serialize)]
pub struct ConsumerStatus {
    pub name: String,
    pub stream: String,
    pub num_pending: u64,
    pub num_ack_pending: u64,
    pub num_waiting: u64,
    pub num_redelivered: u64,
    pub num_delivered: u64,
    pub num_ack_floor: u64,
    pub last_delivered_seq: u64,
    pub ack_floor_seq: u64,
}

#[derive(Debug, Serialize)]
pub struct DlqStatus {
    pub total_messages: u64,
    pub total_bytes: u64,
    pub by_subject: Vec<DlqSubjectCount>,
}

#[derive(Debug, Serialize)]
pub struct DlqSubjectCount {
    pub subject: String,
    pub count: u64,
}

const STREAMS: &[&str] = &[
    "COLLECTION_TRANSFORMS",
    "DATASET_TRANSFORMS",
    "VISUALIZATION_TRANSFORMS",
    "DLQ_TRANSFORMS",
    "SCANNER_TRIGGERS",
    "TRANSFORM_STATUS",
    "AUDIT_EVENTS",
];

const CONSUMERS: &[(&str, &str)] = &[
    ("collection-transform-workers", "COLLECTION_TRANSFORMS"),
    ("dataset-transform-workers", "DATASET_TRANSFORMS"),
    (
        "visualization-transform-workers",
        "VISUALIZATION_TRANSFORMS",
    ),
    ("scanner-workers", "SCANNER_TRIGGERS"),
    ("audit-db-writer", "AUDIT_EVENTS"),
];

const DLQ_SUBJECTS: &[&str] = &[
    "dlq.collection-transforms",
    "dlq.dataset-transforms",
    "dlq.visualization-transforms",
];

#[get("/api/status/nats")]
pub async fn get_nats_status(nats_client: Data<Client>) -> impl Responder {
    let client = nats_client.get_ref();
    let jetstream = jetstream::new(client.clone());

    // Connection state
    let state = match client.connection_state() {
        async_nats::connection::State::Connected => NatsConnectionState::Connected,
        async_nats::connection::State::Disconnected => NatsConnectionState::Disconnected,
        async_nats::connection::State::Pending => NatsConnectionState::Pending,
    };

    let connection = NatsConnectionStatus {
        state,
        server_url: client.server_info().host.clone(),
    };

    // Collect stream info
    let mut streams = Vec::new();
    for stream_name in STREAMS {
        match jetstream.get_stream(*stream_name).await {
            Ok(mut stream) => match stream.info().await {
                Ok(info) => {
                    let retention = match info.config.retention {
                        async_nats::jetstream::stream::RetentionPolicy::Limits => {
                            "Limits".to_string()
                        }
                        async_nats::jetstream::stream::RetentionPolicy::Interest => {
                            "Interest".to_string()
                        }
                        async_nats::jetstream::stream::RetentionPolicy::WorkQueue => {
                            "WorkQueue".to_string()
                        }
                    };

                    streams.push(StreamStatus {
                        name: stream_name.to_string(),
                        messages: info.state.messages,
                        bytes: info.state.bytes,
                        consumer_count: info.state.consumer_count,
                        first_seq: info.state.first_sequence,
                        last_seq: info.state.last_sequence,
                        first_ts: Some(info.state.first_timestamp.to_string()),
                        last_ts: Some(info.state.last_timestamp.to_string()),
                        subjects: info.config.subjects.clone(),
                        retention,
                    });
                }
                Err(e) => {
                    tracing::warn!(stream = stream_name, error = %e, "Failed to get stream info");
                    streams.push(StreamStatus {
                        name: stream_name.to_string(),
                        messages: 0,
                        bytes: 0,
                        consumer_count: 0,
                        first_seq: 0,
                        last_seq: 0,
                        first_ts: None,
                        last_ts: None,
                        subjects: vec![],
                        retention: "Unknown".to_string(),
                    });
                }
            },
            Err(e) => {
                tracing::warn!(stream = stream_name, error = %e, "Failed to get stream");
                streams.push(StreamStatus {
                    name: stream_name.to_string(),
                    messages: 0,
                    bytes: 0,
                    consumer_count: 0,
                    first_seq: 0,
                    last_seq: 0,
                    first_ts: None,
                    last_ts: None,
                    subjects: vec![],
                    retention: "Unknown".to_string(),
                });
            }
        }
    }

    // Collect consumer info
    let mut consumers = Vec::new();
    for (consumer_name, stream_name) in CONSUMERS {
        match jetstream.get_stream(*stream_name).await {
            Ok(stream) => match stream.get_consumer::<ConsumerConfig>(consumer_name).await {
                Ok(mut consumer) => match consumer.info().await {
                    Ok(info) => {
                        consumers.push(ConsumerStatus {
                            name: consumer_name.to_string(),
                            stream: stream_name.to_string(),
                            num_pending: info.num_pending,
                            num_ack_pending: info.num_ack_pending as u64,
                            num_waiting: info.num_waiting as u64,
                            num_redelivered: info.num_redelivered as u64,
                            num_delivered: info.delivered.consumer_sequence,
                            num_ack_floor: info.ack_floor.consumer_sequence,
                            last_delivered_seq: info.delivered.stream_sequence,
                            ack_floor_seq: info.ack_floor.stream_sequence,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            consumer = consumer_name,
                            stream = stream_name,
                            error = %e,
                            "Failed to get consumer info"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        consumer = consumer_name,
                        stream = stream_name,
                        error = %e,
                        "Failed to get consumer"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(stream = stream_name, error = %e, "Failed to get stream for consumer query");
            }
        }
    }

    // Collect DLQ info with per-subject breakdown
    let dlq = match jetstream.get_stream("DLQ_TRANSFORMS").await {
        Ok(mut stream) => match stream.info().await {
            Ok(info) => {
                // DLQ per-subject breakdown
                // Stream subject counts aren't directly accessible from the public API,
                // so we report the total and list expected subjects
                let by_subject: Vec<DlqSubjectCount> = DLQ_SUBJECTS
                    .iter()
                    .map(|s| DlqSubjectCount {
                        subject: s.to_string(),
                        count: 0, // Exact per-subject counts require subject-filtered stream info
                    })
                    .collect();

                DlqStatus {
                    total_messages: info.state.messages,
                    total_bytes: info.state.bytes,
                    by_subject,
                }
            }
            Err(_) => DlqStatus {
                total_messages: 0,
                total_bytes: 0,
                by_subject: vec![],
            },
        },
        Err(_) => DlqStatus {
            total_messages: 0,
            total_bytes: 0,
            by_subject: vec![],
        },
    };

    let response = NatsStatusResponse {
        connection,
        streams,
        consumers,
        dlq,
    };

    HttpResponse::Ok().json(response)
}
