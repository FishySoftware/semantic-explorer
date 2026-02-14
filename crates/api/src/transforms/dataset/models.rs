use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// Dataset Transform: Processes a Dataset with 1-N embedders to create N Embedded Datasets
/// One Dataset Transform can create multiple Embedded Datasets (one per embedder)
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct DatasetTransform {
    pub dataset_transform_id: i32,
    pub title: String,
    pub source_dataset_id: i32,
    pub embedder_ids: Vec<i32>, // Array of embedder IDs (1-N)
    pub owner_id: String,
    pub owner_display_name: String,
    pub is_enabled: bool,
    #[schema(value_type = Object)]
    pub job_config: serde_json::Value, // batch size, wipe settings
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new Dataset Transform
/// Creating a Dataset Transform with N embedders automatically creates N Embedded Datasets
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct CreateDatasetTransform {
    pub title: String,
    pub source_dataset_id: i32,
    pub embedder_ids: Vec<i32>, // Must have at least 1 embedder
}

/// Request to update an existing Dataset Transform
/// Updating embedder_ids will add/remove Embedded Datasets accordingly
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct UpdateDatasetTransform {
    pub title: Option<String>,
    pub is_enabled: Option<bool>,
    pub embedder_ids: Option<Vec<i32>>, // Add/remove embedders (updates embedded datasets)
    pub job_config: Option<serde_json::Value>,
}

/// Aggregate statistics for a Dataset Transform (across all embedded datasets)
#[derive(Serialize, ToSchema, Debug, Clone, FromRow)]
pub struct DatasetTransformStats {
    pub dataset_transform_id: i32,
    pub embedder_count: i32,
    pub total_batches_processed: i64,
    pub successful_batches: i64,
    pub failed_batches: i64,
    pub processing_batches: i64,
    pub total_chunks_embedded: i64,
    pub total_chunks_processing: i64,
    pub total_chunks_failed: i64,
    pub total_chunks_to_process: i64,
    /// Total batches dispatched to workers (for completion tracking)
    #[serde(default)]
    pub total_batches_dispatched: i64,
    /// Total chunks dispatched to workers
    #[serde(default)]
    pub total_chunks_dispatched: i64,
    /// Current run ID for tracking which run batches belong to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_run_id: Option<String>,
    /// When the current run started
    #[schema(value_type = Option<String>, format = DateTime)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_run_started_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_run_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub first_processing_at: Option<DateTime<Utc>>,
}

impl DatasetTransformStats {
    /// Calculate overall status based on processing progress
    ///
    /// Status logic considers both batch-level and chunk-level tracking for accuracy:
    /// - Uses total_batches_dispatched vs (successful + failed) for batch completion
    /// - Uses total_chunks_dispatched for chunk-level accuracy when available
    /// - Falls back to total_chunks_to_process for backward compatibility
    pub fn status(&self) -> &'static str {
        // If there are batches currently processing, we're active
        if self.processing_batches > 0 {
            return "processing";
        }

        // Use dispatched counts if available (more accurate)
        let use_dispatched_tracking = self.total_batches_dispatched > 0;

        if use_dispatched_tracking {
            // New accurate tracking based on what was actually dispatched
            let completed_batches = self.successful_batches + self.failed_batches;

            if completed_batches >= self.total_batches_dispatched {
                // All dispatched batches have been processed
                if self.failed_batches > 0 && self.successful_batches == 0 {
                    return "failed";
                }
                if self.failed_batches > 0 {
                    return "completed_with_errors";
                }
                return "completed";
            }

            // Some batches still pending
            if self.successful_batches > 0 || self.failed_batches > 0 {
                return "processing"; // Has some activity
            }

            return "pending"; // Dispatched but not yet started
        }

        // Fallback: Legacy behavior using total_chunks_to_process
        if self.total_chunks_to_process == 0 {
            // No items to process, check if we have any activity at all
            if self.total_batches_processed > 0 {
                return "completed"; // Had activity but now idle
            }
            return "idle"; // Never processed anything
        }

        // Check if all chunks are processed
        let total_processed = self.total_chunks_embedded + self.total_chunks_failed;
        if total_processed >= self.total_chunks_to_process {
            if self.total_chunks_failed > 0 && self.total_chunks_embedded == 0 {
                return "failed";
            }
            if self.total_chunks_failed > 0 {
                return "completed_with_errors";
            }
            return "completed";
        }

        // Have items to process but not done - could be waiting for scanner
        if self.total_batches_processed == 0 {
            return "pending";
        }

        // Has some activity but not done
        "processing"
    }

    /// Check if transform is currently processing
    pub fn is_processing(&self) -> bool {
        self.processing_batches > 0
    }
}
