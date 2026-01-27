//! Visualization transform result listener
//!
//! Handles results from visualization transform workers (plot generation).

use async_nats::{Client as NatsClient, jetstream};
use futures_util::StreamExt;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info};

use crate::storage::postgres::visualization_transforms::{self, VisualizationUpdate};
use crate::transforms::visualization::models::VisualizationTransform;
use semantic_explorer_core::models::VisualizationTransformResult;

use super::super::listeners::publish_transform_status;

/// Context for visualization listener
#[derive(Clone)]
pub(crate) struct VisualizationListenerContext {
    pub pool: Pool<Postgres>,
    pub nats_client: NatsClient,
}

/// Start the visualization transform result listener
pub(crate) fn start(context: VisualizationListenerContext) {
    let nats_client = context.nats_client.clone();

    actix_web::rt::spawn(async move {
        // Use JetStream durable consumer for reliable message delivery
        // Subject format: transforms.visualization.status.{owner}.{dataset_id}.{transform_id}
        let subject = "transforms.visualization.status.>";
        let stream_name = "TRANSFORM_STATUS";
        let consumer_name = "visualization-status-listener";

        let jetstream = jetstream::new(nats_client.clone());

        let stream = match jetstream.get_stream(stream_name).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get stream {}: {}", stream_name, e);
                return;
            }
        };

        // Create or get durable consumer
        let consumer = match stream.get_consumer(consumer_name).await {
            Ok(c) => c,
            Err(_) => {
                let config = jetstream::consumer::pull::Config {
                    durable_name: Some(consumer_name.to_string()),
                    description: Some("Visualization transform status listener".to_string()),
                    filter_subject: subject.to_string(),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    ack_wait: Duration::from_secs(60),
                    max_deliver: 5,
                    ..Default::default()
                };
                match stream.create_consumer(config).await {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to create consumer {}: {}", consumer_name, e);
                        return;
                    }
                }
            }
        };

        info!(
            "Visualization result listener started with durable consumer: {}",
            consumer_name
        );

        let mut messages = match consumer.messages().await {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to get message stream: {}", e);
                return;
            }
        };

        while let Some(msg) = messages.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to receive message: {}", e);
                    continue;
                }
            };

            info!(
                "Received visualization result message on subject: {}",
                msg.subject
            );
            match serde_json::from_slice::<VisualizationTransformResult>(&msg.payload) {
                Ok(result) => {
                    handle_result(result, &context).await;
                    if let Err(e) = msg.ack().await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to deserialize visualization result: {}", e);
                    // Acknowledge bad messages to prevent reprocessing
                    if let Err(ack_err) = msg.ack().await {
                        error!("Failed to acknowledge bad message: {}", ack_err);
                    }
                }
            }
        }
    });
}

#[tracing::instrument(name = "handle_visualization_result", skip(ctx))]
async fn handle_result(result: VisualizationTransformResult, ctx: &VisualizationListenerContext) {
    info!(
        "Handling visualization result for transform_id={} (status: {})",
        result.visualization_transform_id, result.status
    );

    // Validate ownership by fetching the visualization transform
    let transform = match visualization_transforms::get_visualization_transform_by_id(
        &ctx.pool,
        result.visualization_transform_id,
    )
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            error!(
                "Visualization transform {} not found",
                result.visualization_transform_id
            );
            return;
        }
        Err(e) => {
            error!(
                "Failed to fetch visualization transform {}: {}",
                result.visualization_transform_id, e
            );
            return;
        }
    };

    match result.status.as_str() {
        "processing" => {
            handle_processing_status(&result, ctx).await;
        }
        "failed" => {
            handle_failed_status(&result, &transform, ctx).await;
        }
        "success" => {
            handle_success_status(&result, &transform, ctx).await;
        }
        _ => {
            error!(
                "Unknown status '{}' for visualization transform {}",
                result.status, result.visualization_transform_id
            );
        }
    }
}

async fn handle_processing_status(
    result: &VisualizationTransformResult,
    ctx: &VisualizationListenerContext,
) {
    info!(
        "Visualization transform {} is processing (visualization_id: {})",
        result.visualization_transform_id, result.visualization_id
    );

    // Check current status - don't overwrite if already completed or failed
    // This prevents race conditions when processing/success messages arrive out of order
    if let Ok(viz) = visualization_transforms::get_visualization(&ctx.pool, result.visualization_id).await {
        if viz.status == "completed" || viz.status == "failed" {
            info!(
                "Skipping processing update for visualization {} - already {}",
                result.visualization_id, viz.status
            );
            return;
        }
    }

    // Update the visualization transform status
    if let Err(e) = visualization_transforms::update_visualization_transform_status(
        &ctx.pool,
        result.visualization_transform_id,
        Some("processing"),
        Some(chrono::Utc::now()),
        None,
        None,
    )
    .await
    {
        error!(
            "Failed to update visualization transform status to processing: {}",
            e
        );
    }

    // Update the individual visualization record
    let update = VisualizationUpdate::new()
        .status("processing")
        .started_at(chrono::Utc::now());

    if let Err(e) =
        visualization_transforms::update_visualization(&ctx.pool, result.visualization_id, &update)
            .await
    {
        error!(
            "Failed to update visualization {} to processing: {}",
            result.visualization_id, e
        );
    }
}

async fn handle_failed_status(
    result: &VisualizationTransformResult,
    transform: &VisualizationTransform,
    ctx: &VisualizationListenerContext,
) {
    error!(
        "Visualization transform {} failed: {:?}",
        result.visualization_transform_id, result.error_message
    );

    let error_msg = result
        .error_message
        .clone()
        .unwrap_or_else(|| "Unknown error".to_string());

    if let Err(e) = visualization_transforms::update_visualization_transform_status(
        &ctx.pool,
        result.visualization_transform_id,
        Some("failed"),
        Some(chrono::Utc::now()),
        Some(&error_msg),
        result.stats_json.as_ref(),
    )
    .await
    {
        error!(
            "Failed to update visualization transform status to failed: {}",
            e
        );
        return;
    }

    // Update the individual visualization record
    let mut update = VisualizationUpdate::new()
        .status("failed")
        .completed_at(chrono::Utc::now())
        .error_message(&error_msg);

    if let Some(ref stats) = result.stats_json {
        update = update.stats_json(stats.clone());
    }

    if let Err(e) =
        visualization_transforms::update_visualization(&ctx.pool, result.visualization_id, &update)
            .await
    {
        error!(
            "Failed to update visualization {} to failed: {}",
            result.visualization_id, e
        );
    }

    // Publish failed status for SSE streaming
    publish_transform_status(
        &ctx.nats_client,
        "visualization",
        &result.owner_id,
        transform.embedded_dataset_id,
        result.visualization_transform_id,
        "failed",
        Some(&error_msg),
    )
    .await;
}

async fn handle_success_status(
    result: &VisualizationTransformResult,
    transform: &VisualizationTransform,
    ctx: &VisualizationListenerContext,
) {
    info!(
        "Visualization transform {} completed successfully (visualization_id: {}, output_key: {:?}, duration_ms: {:?})",
        result.visualization_transform_id,
        result.visualization_id,
        result.html_s3_key,
        result.processing_duration_ms
    );

    // Update the transform status
    if let Err(e) = visualization_transforms::update_visualization_transform_status(
        &ctx.pool,
        result.visualization_transform_id,
        Some("completed"),
        Some(chrono::Utc::now()),
        None,
        result.stats_json.as_ref(),
    )
    .await
    {
        error!(
            "Failed to update visualization transform status to completed: {}",
            e
        );
        return;
    }

    // Update the individual visualization record with all result data
    let mut update = VisualizationUpdate::new()
        .status("completed")
        .completed_at(chrono::Utc::now());

    if let Some(ref s3_key) = result.html_s3_key {
        update = update.html_s3_key(s3_key);
    }

    if let Some(point_count) = result.point_count {
        update = update.point_count(point_count as i32);
    }

    if let Some(cluster_count) = result.cluster_count {
        update = update.cluster_count(cluster_count);
    }

    if let Some(ref stats) = result.stats_json {
        update = update.stats_json(stats.clone());
    }

    if let Err(e) =
        visualization_transforms::update_visualization(&ctx.pool, result.visualization_id, &update)
            .await
    {
        error!(
            "Failed to update visualization {} to completed: {}",
            result.visualization_id, e
        );
    }

    // Publish completed status for SSE streaming
    publish_transform_status(
        &ctx.nats_client,
        "visualization",
        &result.owner_id,
        transform.embedded_dataset_id,
        result.visualization_transform_id,
        "completed",
        None,
    )
    .await;

    info!(
        "Successfully completed visualization transform {} (visualization_id: {})",
        result.visualization_transform_id, result.visualization_id
    );
}
