use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct EmbeddedDatasetListQuery {
    pub search: Option<String>,
    #[schema(default = 20, minimum = 1, maximum = 1000)]
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[schema(default = 0, minimum = 0)]
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedEmbeddedDatasetList {
    pub embedded_datasets: Vec<EmbeddedDataset>,
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct EmbeddedDataset {
    pub embedded_dataset_id: i32,
    pub title: String,
    pub dataset_transform_id: i32, // Parent Dataset Transform (0 for standalone)
    pub source_dataset_id: i32,    // Source Dataset ID (0 for standalone)
    pub embedder_id: i32,          // Embedder ID (0 for standalone)
    pub owner_id: String,
    pub owner_display_name: String,
    pub collection_name: String, // Qdrant collection name
    /// Vector dimensions (required for standalone datasets, optional for transform-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i32>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_processed_at: Option<DateTime<Utc>>,
    /// Last processed item_id for composite (timestamp, item_id) watermark.
    /// Together with last_processed_at, enables progress through same-timestamp items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_processed_item_id: Option<i32>,
    /// Tracks source dataset version for efficient stats refresh
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub source_dataset_version: Option<DateTime<Utc>>,
}

/// Embedded Dataset with enriched information (joins)
/// For standalone datasets, source_dataset_title and embedder_name will be "N/A"
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
    /// Vector dimensions (always present for standalone, derived from embedder for transform-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i32>,
    /// Whether this is a standalone embedded dataset (true when dataset_transform_id == 0)
    pub is_standalone: bool,
    /// Collection ID if this embedded dataset was created from a collection transform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<i32>,
    /// Collection title if this embedded dataset was created from a collection transform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_title: Option<String>,
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

    /// Check if this is a standalone embedded dataset (not created via transform)
    /// Standalone datasets have sentinel value 0 for dataset_transform_id, source_dataset_id, and embedder_id
    pub fn is_standalone(&self) -> bool {
        self.dataset_transform_id == 0 && self.source_dataset_id == 0 && self.embedder_id == 0
    }
}

/// Request to create a standalone embedded dataset
/// Standalone datasets can receive vectors directly via push, without needing a transform/embedder
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateStandaloneEmbeddedDatasetRequest {
    /// Title for the embedded dataset
    pub title: String,
    /// Vector dimensions (must match the vectors you will push)
    #[schema(minimum = 1, maximum = 65536)]
    pub dimensions: i32,
}

/// A single vector point to be pushed to a standalone embedded dataset
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone)]
pub struct VectorPoint {
    /// Unique identifier for this point (UUID string or numeric string)
    pub id: String,
    /// The vector data (must match the dimensions of the dataset)
    pub vector: Vec<f32>,
    /// Metadata payload for this point
    pub payload: serde_json::Value,
}

/// Request to push vectors to a standalone embedded dataset
#[derive(Debug, Deserialize, ToSchema)]
pub struct PushVectorsRequest {
    /// Array of points to push (max 1000 per request)
    #[schema(max_items = 1000)]
    pub points: Vec<VectorPoint>,
}

/// Response after pushing vectors
#[derive(Debug, Serialize, ToSchema)]
pub struct PushVectorsResponse {
    /// Number of points successfully inserted
    pub points_inserted: usize,
    /// Collection name where points were inserted
    pub collection_name: String,
}
