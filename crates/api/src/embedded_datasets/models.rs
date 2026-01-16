use serde::Serialize;
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// Embedded Dataset: Result entity created by Dataset Transform
/// One Embedded Dataset per embedder in the Dataset Transform
/// Contains vector embeddings stored in Qdrant
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct EmbeddedDataset {
    pub embedded_dataset_id: i32,
    pub title: String,
    pub dataset_transform_id: i32, // Parent Dataset Transform
    pub source_dataset_id: i32,
    pub embedder_id: i32, // Single embedder
    pub owner_id: String,
    pub owner_display_name: String,
    pub collection_name: String, // Qdrant collection name
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_processed_at: Option<DateTime<Utc>>,
}

/// Embedded Dataset with enriched information (joins)
#[derive(Serialize, ToSchema, Debug, Clone, FromRow)]
pub struct EmbeddedDatasetWithDetails {
    pub embedded_dataset_id: i32,
    pub title: String,
    pub dataset_transform_id: i32,
    pub source_dataset_id: i32,
    pub source_dataset_title: String,
    pub embedder_id: i32,
    pub embedder_name: String,
    pub owner_id: String,
    pub owner_display_name: String,
    pub collection_name: String,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

/// Statistics for a specific Embedded Dataset
#[derive(Serialize, ToSchema, Debug, Clone, FromRow)]
pub struct EmbeddedDatasetStats {
    pub embedded_dataset_id: i32,
    pub total_batches_processed: i64,
    pub successful_batches: i64,
    pub failed_batches: i64,
    pub processing_batches: i64,
    pub total_chunks_embedded: i64,
    pub total_chunks_failed: i64,
    pub total_chunks_processing: i64,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_run_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub first_processing_at: Option<DateTime<Utc>>,
    pub avg_processing_duration_ms: Option<i64>,
}

/// Processed batch for this embedded dataset
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct EmbeddedDatasetProcessedBatch {
    pub id: i32,
    pub embedded_dataset_id: i32,
    pub file_key: String,
    #[schema(value_type = String, format = DateTime)]
    pub processed_at: DateTime<Utc>,
    pub item_count: i32,
    pub process_status: String,
    pub process_error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

impl EmbeddedDataset {
    /// Generate Qdrant collection name for this embedded dataset
    pub fn generate_collection_name(embedded_dataset_id: i32, owner: &str) -> String {
        format!("embedded-dataset-{}-{}", embedded_dataset_id, owner)
    }
}
