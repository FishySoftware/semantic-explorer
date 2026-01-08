use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// Collection Transform: Processes files from a Collection into Dataset items
/// Handles extraction and chunking
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct CollectionTransform {
    pub collection_transform_id: i32,
    pub title: String,
    pub collection_id: i32,
    pub dataset_id: i32,
    pub owner: String,
    pub is_enabled: bool,
    pub chunk_size: i32,
    #[schema(value_type = Object)]
    pub job_config: serde_json::Value, // extraction + chunking config
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new Collection Transform
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct CreateCollectionTransform {
    pub title: String,
    pub collection_id: i32,
    pub dataset_id: i32,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: i32,
    #[serde(default)]
    pub job_config: serde_json::Value, // extraction + chunking config
}

/// Request to update an existing Collection Transform
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct UpdateCollectionTransform {
    pub title: Option<String>,
    pub is_enabled: Option<bool>,
    pub chunk_size: Option<i32>,
    pub job_config: Option<serde_json::Value>,
}

/// Request to trigger a Collection Transform manually
#[allow(dead_code)]
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct TriggerCollectionTransformRequest {
    pub collection_transform_id: i32,
}

/// Statistics for a Collection Transform
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct CollectionTransformStats {
    pub collection_transform_id: i32,
    pub total_files_processed: i64,
    pub successful_files: i64,
    pub failed_files: i64,
    pub total_items_created: i64,
}

/// Statistics with total file count from collection
#[allow(dead_code)]
#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct CollectionTransformStatsWithTotal {
    pub collection_transform_id: i32,
    pub total_files_in_collection: i64,
    pub total_files_processed: i64,
    pub successful_files: i64,
    pub failed_files: i64,
    pub total_items_created: i64,
}

/// Processed file record
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct ProcessedFile {
    pub id: i32,
    pub transform_type: String, // 'collection'
    pub transform_id: i32,      // collection_transform_id
    pub file_key: String,
    #[schema(value_type = String, format = DateTime)]
    pub processed_at: DateTime<Utc>,
    pub item_count: i32,
    pub process_status: String,
    pub process_error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

fn default_chunk_size() -> i32 {
    200
}
