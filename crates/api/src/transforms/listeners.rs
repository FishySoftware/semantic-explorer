//! Transform result listeners - coordination module
//!
//! This module coordinates the startup of all transform result listeners.
//! Each transform type (collection, dataset, visualization) has its own
//! listener implementation in its respective submodule.

use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use tracing::{error, info, warn};

use crate::transforms::collection::listener as collection_listener;
use crate::transforms::dataset::listener as dataset_listener;
use crate::transforms::visualization::listener as visualization_listener;

/// Status update payload for SSE streams
#[derive(serde::Serialize)]
pub(crate) struct TransformStatusUpdate {
    /// The type of transform: "collection", "dataset", or "visualization"
    pub transform_type: String,
    /// The ID of the transform
    pub transform_id: i32,
    /// The ID of the related resource (collection_id, dataset_id, or embedded_dataset_id)
    pub resource_id: i32,
    /// Current status: "processing", "completed", "failed"
    pub status: String,
    /// Optional error message for failed status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Timestamp of the status update
    pub timestamp: String,
}

/// Publish a transform status update to NATS for SSE streaming.
/// Subject format: sse.transforms.{type}.status.{owner}.{resource_id}.{transform_id}
/// NOTE: Uses sse. prefix to avoid being captured by JetStream TRANSFORM_STATUS stream,
/// which is used for worker results (CollectionTransformResult, etc.)
pub(crate) async fn publish_transform_status(
    nats: &NatsClient,
    transform_type: &str,
    owner: &str,
    resource_id: i32,
    transform_id: i32,
    status: &str,
    error: Option<&str>,
) {
    let subject = format!(
        "sse.transforms.{}.status.{}.{}.{}",
        transform_type, owner, resource_id, transform_id
    );

    let update = TransformStatusUpdate {
        transform_type: transform_type.to_string(),
        transform_id,
        resource_id,
        status: status.to_string(),
        error: error.map(|e| e.to_string()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    match serde_json::to_vec(&update) {
        Ok(payload) => {
            if let Err(e) = nats.publish(subject.clone(), payload.into()).await {
                warn!("Failed to publish transform status to {}: {}", subject, e);
            } else {
                info!("Published transform status to {}", subject);
            }
        }
        Err(e) => {
            error!("Failed to serialize transform status update: {}", e);
        }
    }
}

/// Start all transform result listeners
pub(crate) async fn start_result_listeners(
    pool: Pool<Postgres>,
    s3_client: S3Client,
    s3_bucket_name: String,
    nats_client: NatsClient,
) -> Result<()> {
    // Start collection transform result listener
    collection_listener::start(collection_listener::CollectionListenerContext {
        pool: pool.clone(),
        s3_client: s3_client.clone(),
        s3_bucket_name: s3_bucket_name.clone(),
        nats_client: nats_client.clone(),
    });

    // Start dataset transform result listener
    dataset_listener::start_result_listener(dataset_listener::DatasetListenerContext {
        pool: pool.clone(),
        s3_client: s3_client.clone(),
        s3_bucket_name: s3_bucket_name.clone(),
        nats_client: nats_client.clone(),
    });

    // Start visualization transform result listener
    visualization_listener::start(visualization_listener::VisualizationListenerContext {
        pool: pool.clone(),
        nats_client: nats_client.clone(),
    });

    Ok(())
}
