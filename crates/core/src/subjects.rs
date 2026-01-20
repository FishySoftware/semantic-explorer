//! NATS subject constants for worker communication.
//!
//! This module centralizes all NATS subject names used across workers
//! to prevent duplication and ensure consistency.
//!
//! Stream/Subject Architecture:
//! - Job streams use WorkQueue retention for reliable delivery
//! - Status streams use hierarchical subjects for SSE filtering

/// Job request subjects - API publishes to these (matched by JetStream streams)
pub mod jobs {
    /// Subject for collection file transformation jobs
    /// Stream: COLLECTION_TRANSFORMS
    pub const COLLECTION_TRANSFORM: &str = "workers.collection-transform";

    /// Subject for dataset embedding jobs
    /// Stream: DATASET_TRANSFORMS
    pub const DATASET_TRANSFORM: &str = "workers.dataset-transform";

    /// Subject for visualization transformation jobs
    /// Stream: VISUALIZATION_TRANSFORMS
    pub const VISUALIZATION_TRANSFORM: &str = "workers.visualization-transform";
}

/// Status update subjects - Workers publish to these for SSE real-time updates
/// Format: transforms.{type}.status.{owner}.{resource_id}.{transform_id}
/// Stream: TRANSFORM_STATUS
pub mod status {
    /// Prefix for collection transform status updates
    pub const COLLECTION_TRANSFORM_PREFIX: &str = "transforms.collection.status";

    /// Prefix for dataset transform status updates
    pub const DATASET_TRANSFORM_PREFIX: &str = "transforms.dataset.status";

    /// Prefix for visualization transform status updates
    pub const VISUALIZATION_TRANSFORM_PREFIX: &str = "transforms.visualization.status";

    /// Build a collection transform status subject
    /// Format: transforms.collection.status.{owner}.{collection_id}.{transform_id}
    pub fn collection_status_subject(owner: &str, collection_id: i32, transform_id: i32) -> String {
        format!(
            "{}.{}.{}.{}",
            COLLECTION_TRANSFORM_PREFIX, owner, collection_id, transform_id
        )
    }

    /// Build a dataset transform status subject
    /// Format: transforms.dataset.status.{owner}.{dataset_id}.{transform_id}
    pub fn dataset_status_subject(owner: &str, dataset_id: i32, transform_id: i32) -> String {
        format!(
            "{}.{}.{}.{}",
            DATASET_TRANSFORM_PREFIX, owner, dataset_id, transform_id
        )
    }

    /// Build a visualization transform status subject
    /// Format: transforms.visualization.status.{owner}.{embedded_dataset_id}.{transform_id}
    pub fn visualization_status_subject(
        owner: &str,
        embedded_dataset_id: i32,
        transform_id: i32,
    ) -> String {
        format!(
            "{}.{}.{}.{}",
            VISUALIZATION_TRANSFORM_PREFIX, owner, embedded_dataset_id, transform_id
        )
    }
}

/// Dead letter queue subjects
/// Stream: DLQ_TRANSFORMS
pub mod dlq {
    /// DLQ for failed collection transformation jobs
    pub const COLLECTION_TRANSFORM: &str = "dlq.collection-transforms";

    /// DLQ for failed dataset transformation jobs
    pub const DATASET_TRANSFORM: &str = "dlq.dataset-transforms";

    /// DLQ for failed visualization transformation jobs
    pub const VISUALIZATION_TRANSFORM: &str = "dlq.visualization-transforms";
}

/// Consumer/durable names for load balancing
pub mod consumers {
    /// Consumer group for collection file workers
    pub const COLLECTION_WORKERS: &str = "collection-transform-workers";

    /// Consumer group for dataset embedding workers
    pub const DATASET_WORKERS: &str = "dataset-transform-workers";

    /// Consumer group for visualization workers
    pub const VISUALIZATION_WORKERS: &str = "visualization-transform-workers";
}
