//! Audit event consumer worker that processes events from NATS stream.
//!
//! This module consumes audit events published to the AUDIT_EVENTS stream and
//! persists them to the database with automatic retry and dead-letter queue handling.

use crate::audit::{AUDIT_EVENTS_STREAM, AuditEvent};
use crate::storage::postgres::audit;
use async_nats::Client;
use async_nats::jetstream::consumer::AckPolicy;
use async_nats::jetstream::consumer::pull::Config as ConsumerConfig;
use async_nats::jetstream::stream::RetentionPolicy;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info, warn};

/// Durable consumer name for audit events
const AUDIT_CONSUMER_NAME: &str = "audit-db-writer";

/// Start the audit event consumer worker
/// This function blocks indefinitely and should be spawned in a tokio task
pub async fn start_audit_consumer(
    nats_client: Client,
    db_pool: Pool<Postgres>,
) -> anyhow::Result<()> {
    info!("Starting audit event consumer worker");

    let jetstream = async_nats::jetstream::new(nats_client.clone());

    // Ensure AUDIT_EVENTS stream exists
    let stream = ensure_audit_stream(&jetstream).await?;

    // Create or get durable consumer for reliable message processing
    let consumer = stream
        .get_or_create_consumer(
            AUDIT_CONSUMER_NAME,
            ConsumerConfig {
                durable_name: Some(AUDIT_CONSUMER_NAME.to_string()),
                ack_policy: AckPolicy::Explicit,
                ack_wait: Duration::from_secs(30),
                max_deliver: 5,
                ..Default::default()
            },
        )
        .await?;

    info!(
        consumer = AUDIT_CONSUMER_NAME,
        "Audit event consumer connected to JetStream"
    );

    // Start consuming messages from JetStream
    let mut messages = consumer.messages().await?;

    while let Some(msg_result) = messages.next().await {
        match msg_result {
            Ok(message) => {
                match serde_json::from_slice::<AuditEvent>(&message.payload) {
                    Ok(event) => match store_audit_event(&db_pool, &event).await {
                        Ok(_) => {
                            // Acknowledge successful processing
                            if let Err(e) = message.ack().await {
                                warn!(
                                    error = %e,
                                    "Failed to acknowledge audit event message"
                                );
                            }
                            info!(
                                event_type = ?event.event_type,
                                user = %event.user,
                                "Audit event stored"
                            );
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                event = ?event,
                                "Failed to store audit event"
                            );
                            // NAK to retry later (JetStream will redeliver)
                            if let Err(nak_err) = message
                                .ack_with(async_nats::jetstream::AckKind::Nak(None))
                                .await
                            {
                                warn!(error = %nak_err, "Failed to NAK audit event message");
                            }
                        }
                    },
                    Err(e) => {
                        warn!(
                            error = %e,
                            payload_size = message.payload.len(),
                            "Failed to deserialize audit event, terminating message"
                        );
                        // Terminate message (don't redeliver malformed messages)
                        if let Err(term_err) =
                            message.ack_with(async_nats::jetstream::AckKind::Term).await
                        {
                            warn!(error = %term_err, "Failed to terminate audit event message");
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Error receiving message from JetStream");
            }
        }
    }

    Ok(())
}

/// Store an audit event in the database using the storage layer
async fn store_audit_event(pool: &Pool<Postgres>, event: &AuditEvent) -> Result<(), sqlx::Error> {
    audit::store_audit_event_simple(pool, event).await
}

/// Ensure the AUDIT_EVENTS stream exists in NATS JetStream and return it
async fn ensure_audit_stream(
    jetstream: &async_nats::jetstream::Context,
) -> anyhow::Result<async_nats::jetstream::stream::Stream> {
    use async_nats::jetstream::stream::Config as StreamConfig;

    // Try to get existing stream, create if needed
    match jetstream.get_stream(AUDIT_EVENTS_STREAM).await {
        Ok(stream) => {
            info!("Audit stream already exists");
            Ok(stream)
        }
        Err(_) => {
            // Stream doesn't exist, create it
            info!("Creating audit stream");
            let stream = jetstream
                .create_stream(StreamConfig {
                    name: AUDIT_EVENTS_STREAM.to_string(),
                    subjects: vec!["audit.events".to_string()],
                    retention: RetentionPolicy::Limits,
                    max_age: Duration::from_secs(30 * 24 * 60 * 60), // 30 days retention
                    num_replicas: 3,
                    ..Default::default()
                })
                .await?;

            info!("Audit stream created successfully");
            Ok(stream)
        }
    }
}
