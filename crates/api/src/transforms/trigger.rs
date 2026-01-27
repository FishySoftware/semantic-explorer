//! Scanner Trigger System
//!
//! This module provides NATS-based coordination for transform scanners.
//! Instead of each API instance running its own timer, triggers are published
//! to NATS and consumed with `max_ack_pending: 1` to ensure only one instance
//! processes each scan at a time (HA active/standby pattern).
//!
//! Architecture:
//! - One API instance runs a lightweight timer that publishes triggers
//! - All API instances listen for triggers and process them
//! - NATS JetStream ensures exactly-once processing with failover

use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::{
    Client as NatsClient,
    jetstream::{self, Message, consumer::pull::Config as ConsumerConfig},
};
use aws_sdk_s3::Client as S3Client;
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::{QdrantConnectionConfig, ScanTrigger};
use semantic_explorer_core::nats::{ensure_consumer, extract_otel_context, inject_trace_context};
use semantic_explorer_core::observability::{
    record_scanner_scan_duration, record_scanner_trigger_processed,
    record_scanner_trigger_published,
};

use super::collection::scanner as collection_scanner;
use super::dataset::scanner as dataset_scanner;

/// Context required for processing scan triggers
#[derive(Clone)]
pub struct ScannerContext {
    pub pool: Pool<Postgres>,
    pub nats: NatsClient,
    pub s3: S3Client,
    pub s3_bucket_name: String,
    pub encryption: EncryptionService,
    pub qdrant_config: QdrantConnectionConfig,
}

/// Start the trigger publisher - a lightweight timer that publishes scan triggers.
/// Only one instance should run this (can use leader election or just accept redundancy).
pub fn start_trigger_publisher(nats: NatsClient) -> JoinHandle<()> {
    spawn(async move {
        // Stagger different scan types to avoid thundering herd
        let collection_interval = Duration::from_secs(5);
        let dataset_interval = Duration::from_secs(10);

        // Use separate tasks for each scan type
        let nats_clone = nats.clone();
        let collection_publisher = spawn(async move {
            let mut interval = interval(collection_interval);
            loop {
                interval.tick().await;
                if let Err(e) = publish_trigger(&nats_clone, "collection").await {
                    warn!("Failed to publish collection scan trigger: {}", e);
                }
            }
        });

        let nats_clone = nats.clone();
        let dataset_publisher = spawn(async move {
            let mut interval = interval(dataset_interval);
            loop {
                interval.tick().await;
                if let Err(e) = publish_trigger(&nats_clone, "dataset").await {
                    warn!("Failed to publish dataset scan trigger: {}", e);
                }
            }
        });

        // Wait for both (they run forever)
        let _ = tokio::join!(collection_publisher, dataset_publisher);
    })
}

/// Publish a periodic scan trigger to NATS (scans all active transforms of the type)
async fn publish_trigger(nats: &NatsClient, scan_type: &str) -> Result<()> {
    let trigger = ScanTrigger::periodic(scan_type);
    publish_trigger_internal(nats, &trigger).await
}

/// Publish a targeted scan trigger for a specific transform.
/// This is called when a new transform is created to immediately start processing.
pub async fn publish_targeted_trigger(
    nats: &NatsClient,
    scan_type: &str,
    transform_id: i32,
    owner_id: &str,
) -> Result<()> {
    let trigger = ScanTrigger::targeted(scan_type, transform_id, owner_id);
    publish_trigger_internal(nats, &trigger).await
}

/// Internal function to publish any trigger to NATS
async fn publish_trigger_internal(nats: &NatsClient, trigger: &ScanTrigger) -> Result<()> {
    let subject = trigger.subject();
    let payload = serde_json::to_vec(&trigger)?;

    let jetstream = jetstream::new(nats.clone());
    let mut headers = async_nats::HeaderMap::new();

    // Use trigger_id as message ID for deduplication
    let msg_id = format!("scan-{}-{}", trigger.scan_type, trigger.trigger_id);
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    inject_trace_context(&mut headers);

    jetstream
        .publish_with_headers(subject.clone(), headers, payload.into())
        .await?
        .await?;

    record_scanner_trigger_published(&trigger.scan_type);
    info!(
        "Published {} scan trigger: {} (targeted: {})",
        trigger.scan_type,
        trigger.trigger_id,
        trigger.transform_id.is_some()
    );
    Ok(())
}

/// Start the trigger listener - processes scan triggers from NATS.
/// Multiple instances can run this; NATS ensures only one processes each trigger.
pub fn start_trigger_listener(ctx: ScannerContext) -> JoinHandle<()> {
    spawn(async move {
        loop {
            if let Err(e) = run_trigger_listener(&ctx).await {
                error!("Trigger listener error: {}. Restarting in 5s...", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    })
}

/// Main trigger listener loop
async fn run_trigger_listener(ctx: &ScannerContext) -> Result<()> {
    let jetstream = jetstream::new(ctx.nats.clone());

    // Create consumer for scanner triggers
    let consumer_config = ConsumerConfig {
        durable_name: Some("scanner-workers".to_string()),
        description: Some("Consumer for scanner trigger messages".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(10 * 60), // 10 minutes to complete scan
        max_deliver: 3,
        max_ack_pending: 1, // Only one trigger processed at a time (HA)
        filter_subjects: vec!["scan.trigger.>".to_string()],
        ..Default::default()
    };

    let consumer = ensure_consumer(&jetstream, "SCANNER_TRIGGERS", consumer_config).await?;

    info!("Scanner trigger listener started, waiting for triggers...");

    // Process messages
    let mut messages = consumer
        .messages()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get message stream: {}", e))?;

    while let Some(msg_result) = messages.next().await {
        match msg_result {
            Ok(msg) => {
                if let Err(e) = process_trigger(ctx, &msg).await {
                    error!("Failed to process trigger: {}", e);
                    // NAK so it can be retried
                    if let Err(nak_err) = msg
                        .ack_with(async_nats::jetstream::AckKind::Nak(None))
                        .await
                    {
                        error!("Failed to NAK message: {}", nak_err);
                    }
                } else {
                    // ACK successful processing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to ACK message: {}", ack_err);
                    }
                }
            }
            Err(e) => {
                error!("Error receiving message: {}", e);
            }
        }
    }

    Ok(())
}

/// Process a single scan trigger
#[tracing::instrument(
    name = "process_scan_trigger",
    skip(ctx, msg),
    fields(otel.kind = "consumer")
)]
async fn process_trigger(ctx: &ScannerContext, msg: &Message) -> Result<()> {
    // Extract trace context from message
    let parent_context = extract_otel_context(msg.headers.as_ref());

    // Create child span using the parent context
    let span = tracing::info_span!(
        parent: tracing::Span::none(),
        "scan_trigger_handler",
        otel.kind = "consumer"
    );
    let _ = span.set_parent(parent_context);
    let _guard = span.enter();

    // Parse trigger
    let trigger: ScanTrigger = serde_json::from_slice(&msg.payload)?;

    let start = std::time::Instant::now();

    // Dispatch to appropriate scanner - use targeted scan if transform_id provided
    let result = match (trigger.scan_type.as_str(), trigger.transform_id) {
        // Targeted collection transform scan
        ("collection", Some(transform_id)) => {
            info!(
                "Processing targeted collection scan trigger {} for transform {}",
                trigger.trigger_id, transform_id
            );
            collection_scanner::scan_collection_transform(
                &ctx.pool,
                &ctx.nats,
                &ctx.s3,
                &ctx.s3_bucket_name,
                transform_id,
                &ctx.encryption,
            )
            .await
        }
        // Full collection scan
        ("collection", None) => {
            info!(
                "Processing full collection scan trigger: {}",
                trigger.trigger_id
            );
            collection_scanner::scan_active_collection_transforms(
                &ctx.pool,
                &ctx.nats,
                &ctx.s3,
                &ctx.s3_bucket_name,
                &ctx.encryption,
            )
            .await
        }
        // Targeted dataset transform scan
        ("dataset", Some(transform_id)) => {
            info!(
                "Processing targeted dataset scan trigger {} for transform {}",
                trigger.trigger_id, transform_id
            );
            dataset_scanner::scan_dataset_transform(
                &ctx.pool,
                &ctx.nats,
                &ctx.s3,
                transform_id,
                &ctx.encryption,
                &ctx.qdrant_config,
            )
            .await
        }
        // Full dataset scan
        ("dataset", None) => {
            info!(
                "Processing full dataset scan trigger: {}",
                trigger.trigger_id
            );
            dataset_scanner::scan_active_dataset_transforms(
                &ctx.pool,
                &ctx.nats,
                &ctx.s3,
                &ctx.encryption,
                &ctx.qdrant_config,
            )
            .await
        }
        ("visualization", _) => {
            // Visualization scans are triggered on-demand, not periodically
            info!(
                "Visualization scan trigger received, but visualization scans are on-demand only"
            );
            Ok(())
        }
        (scan_type, _) => {
            warn!("Unknown scan type: {}", scan_type);
            Ok(())
        }
    };

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();

    record_scanner_trigger_processed(&trigger.scan_type, success);
    record_scanner_scan_duration(&trigger.scan_type, duration);

    if success {
        info!(
            "Completed {} scan trigger: {} in {:.2}s",
            trigger.scan_type, trigger.trigger_id, duration
        );
    }

    result
}
