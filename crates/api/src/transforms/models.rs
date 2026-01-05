use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScanCollectionJob {
    pub(crate) transform_id: i32,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(tag = "job_type", rename_all = "snake_case")]
pub(crate) enum CreateTransformConfig {
    CollectionToDataset {
        collection_id: i32,
        dataset_id: i32,
        #[serde(default = "default_chunk_size")]
        chunk_size: i32,
    },
    DatasetToVectorStorage {
        dataset_id: i32,
        embedder_ids: Vec<i32>,
        #[serde(default = "default_embedding_batch_size")]
        embedding_batch_size: Option<i32>,
        #[serde(default)]
        wipe_collection: bool,
    },
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateTransform {
    pub(crate) title: String,
    #[serde(flatten)]
    pub(crate) config: CreateTransformConfig,
}

fn default_chunk_size() -> i32 {
    200
}

fn default_embedding_batch_size() -> Option<i32> {
    None
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct Transform {
    pub(crate) transform_id: i32,
    pub(crate) title: String,
    pub(crate) collection_id: Option<i32>,
    pub(crate) dataset_id: i32,
    pub(crate) owner: String,
    pub(crate) is_enabled: bool,
    pub(crate) chunk_size: i32,
    pub(crate) job_type: String,
    pub(crate) source_dataset_id: Option<i32>,
    pub(crate) target_dataset_id: Option<i32>,
    pub(crate) embedder_ids: Option<Vec<i32>>,
    #[schema(value_type = Object)]
    pub(crate) job_config: serde_json::Value,
    #[schema(value_type = Object)]
    pub(crate) collection_mappings: serde_json::Value,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) updated_at: DateTime<Utc>,
}

impl Transform {
    pub(crate) fn get_collection_name(&self, embedder_id: i32) -> Option<String> {
        self.collection_mappings
            .get(embedder_id.to_string())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub(crate) fn generate_collection_name(
        dataset_id: i32,
        embedder_id: i32,
        transform_id: i32,
        owner: &str,
    ) -> String {
        format!(
            "dataset-{}-embedder-{}-transform-{}-{}",
            dataset_id, embedder_id, transform_id, owner
        )
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct UpdateTransform {
    pub(crate) title: Option<String>,
    pub(crate) is_enabled: Option<bool>,
    pub(crate) chunk_size: Option<i32>,
    pub(crate) embedder_ids: Option<Vec<i32>>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct TriggerTransformRequest {
    pub(crate) transform_id: i32,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct ProcessedFile {
    pub(crate) id: i32,
    pub(crate) transform_id: i32,
    pub(crate) file_key: String,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) processed_at: DateTime<Utc>,
    pub(crate) item_count: i32,
    pub(crate) process_status: String,
    pub(crate) process_error: Option<String>,
    pub(crate) processing_duration_ms: Option<i64>,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct TransformStats {
    pub(crate) transform_id: i32,
    pub(crate) total_files_processed: i64,
    pub(crate) successful_files: i64,
    pub(crate) failed_files: i64,
    pub(crate) total_items_created: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TransformStatsWithTotal {
    pub(crate) transform_id: i32,
    pub(crate) total_files_in_collection: i64,
    pub(crate) total_files_processed: i64,
    pub(crate) successful_files: i64,
    pub(crate) failed_files: i64,
    pub(crate) total_items_created: i64,
}

#[derive(Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub(crate) struct TransformStatsEnhanced {
    pub(crate) transform_id: i32,
    pub(crate) total_items_processed: i64,
    pub(crate) successful_items: i64,
    pub(crate) failed_items: i64,
    pub(crate) total_chunks_embedded: i64,
    pub(crate) total_chunks_failed: i64,
}
