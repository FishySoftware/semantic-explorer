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
    pub owner: String,
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
    #[serde(default)]
    pub embedding_batch_size: Option<i32>,
    #[serde(default)]
    pub wipe_collection: bool,
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

/// Request to trigger a Dataset Transform manually
#[allow(dead_code)]
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct TriggerDatasetTransformRequest {
    pub dataset_transform_id: i32,
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
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_run_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub first_processing_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

impl DatasetTransformStats {
    /// Calculate overall status based on processing progress
    pub fn status(&self) -> &'static str {
        // If there are batches currently processing, we're active
        if self.processing_batches > 0 {
            return "processing";
        }

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

/// Processed batch record for dataset transforms
#[allow(dead_code)]
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct ProcessedBatch {
    pub id: i32,
    pub transform_type: String, // 'dataset'
    pub transform_id: i32,      // dataset_transform_id or embedded_dataset_id
    pub file_key: String,       // batch file key
    #[schema(value_type = String, format = DateTime)]
    pub processed_at: DateTime<Utc>,
    pub item_count: i32, // chunks embedded
    pub process_status: String,
    pub process_error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}
