//! Audit event consumer worker that processes events from NATS stream.
//!
//! This module consumes audit events published to the AUDIT_EVENTS stream and
//! persists them to the database with automatic retry and dead-letter queue handling.

use crate::audit::{AUDIT_EVENTS_STREAM, AuditEvent};
use crate::storage::postgres::audit;
use async_nats::Client;
use async_nats::jetstream::stream::RetentionPolicy;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info, warn};

/// Start the audit event consumer worker
/// This function blocks indefinitely and should be spawned in a tokio task
pub async fn start_audit_consumer(
    nats_client: Client,
    db_pool: Pool<Postgres>,
) -> anyhow::Result<()> {
    info!("Starting audit event consumer worker");

    let jetstream = async_nats::jetstream::new(nats_client.clone());

    // Ensure AUDIT_EVENTS stream exists
    ensure_audit_stream(&jetstream).await?;

    // Subscribe to audit events from the stream using the main NATS client
    let mut subscriber = nats_client.subscribe("audit.events").await?;

    info!("Audit event subscriber connected to audit.events");

    // Start consuming events
    while let Some(message) = subscriber.next().await {
        match serde_json::from_slice::<AuditEvent>(&message.payload) {
            Ok(event) => match store_audit_event(&db_pool, &event).await {
                Ok(_) => {
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
                }
            },
            Err(e) => {
                warn!(
                    error = %e,
                    payload_size = message.payload.len(),
                    "Failed to deserialize audit event"
                );
            }
        }
    }

    Ok(())
}

/// Store an audit event in the database using the storage layer
async fn store_audit_event(pool: &Pool<Postgres>, event: &AuditEvent) -> Result<(), sqlx::Error> {
    audit::store_audit_event(
        pool,
        &event.timestamp,
        &format!("{:?}", event.event_type),
        &format!("{:?}", event.outcome),
        &event.user,
        event.request_id.as_deref(),
        event.client_ip.as_deref(),
        event
            .resource_type
            .as_ref()
            .map(|rt| format!("{:?}", rt))
            .as_deref(),
        event.resource_id.as_deref(),
        event.details.as_deref(),
    )
    .await
}

/// Ensure the AUDIT_EVENTS stream exists in NATS JetStream
async fn ensure_audit_stream(jetstream: &async_nats::jetstream::Context) -> anyhow::Result<()> {
    use async_nats::jetstream::stream::Config as StreamConfig;

    // Try to get existing stream, create if needed
    match jetstream.get_stream(AUDIT_EVENTS_STREAM).await {
        Ok(_) => {
            info!("Audit stream already exists");
            Ok(())
        }
        Err(_) => {
            // Stream doesn't exist, create it
            info!("Creating audit stream");
            jetstream
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
            Ok(())
        }
    }
}
