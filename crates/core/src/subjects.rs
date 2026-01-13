//! NATS subject constants for worker communication.
//!
//! This module centralizes all NATS subject names used across workers
//! to prevent duplication and ensure consistency.

/// Job request subjects - API publishes to these
pub mod jobs {
    /// Subject for collection file transformation jobs
    pub const COLLECTION_TRANSFORM: &str = "worker.job.file";

    /// Subject for dataset embedding jobs
    pub const DATASET_TRANSFORM: &str = "worker.job.vector";

    /// Subject for visualization transformation jobs
    pub const VISUALIZATION_TRANSFORM: &str = "worker.job.visualization";
}

/// Job result subjects - Workers publish to these
pub mod results {
    /// Subject for collection file transformation results
    pub const COLLECTION_TRANSFORM: &str = "worker.result.file";

    /// Subject for dataset embedding results
    pub const DATASET_TRANSFORM: &str = "worker.result.vector";

    /// Subject for visualization transformation results
    pub const VISUALIZATION_TRANSFORM: &str = "worker.result.visualization";

    /// Subject for progress updates (used by dataset transforms)
    pub const PROGRESS_UPDATE: &str = "worker.result.vector";
}

/// Dead letter queue subjects
pub mod dlq {
    /// DLQ for failed collection transformation jobs
    pub const COLLECTION_TRANSFORM: &str = "worker.dlq.file";

    /// DLQ for failed dataset transformation jobs
    pub const DATASET_TRANSFORM: &str = "worker.dlq.vector";

    /// DLQ for failed visualization transformation jobs
    pub const VISUALIZATION_TRANSFORM: &str = "worker.dlq.visualization";
}

/// Consumer group names for load balancing
pub mod consumers {
    /// Consumer group for collection file workers
    pub const COLLECTION_WORKERS: &str = "collection-workers";

    /// Consumer group for dataset embedding workers
    pub const DATASET_WORKERS: &str = "dataset-workers";

    /// Consumer group for visualization workers
    pub const VISUALIZATION_WORKERS: &str = "visualization-workers";
}
