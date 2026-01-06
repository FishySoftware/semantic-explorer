use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(tag = "job_type", rename_all = "snake_case")]
pub(crate) enum CreateTransformConfig {
    CollectionToDataset {
        collection_id: i32,
        dataset_id: i32,
        #[serde(default = "default_chunk_size")]
        chunk_size: i32,
        #[serde(default)]
        job_config: serde_json::Value,
    },
    DatasetToVectorStorage {
        dataset_id: i32,
        embedder_ids: Vec<i32>,
        #[serde(default = "default_embedding_batch_size")]
        embedding_batch_size: Option<i32>,
        #[serde(default)]
        wipe_collection: bool,
    },
    DatasetVisualizationTransform {
        source_transform_id: i32,
        source_embedder_id: i32,
        dataset_id: i32,
        #[serde(default)]
        visualization_config: VisualizationConfig,
    },
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub(crate) struct VisualizationConfig {
    // UMAP parameters
    #[serde(default = "default_n_neighbors")]
    pub(crate) n_neighbors: i32,
    #[serde(default = "default_n_components")]
    pub(crate) n_components: i32,
    #[serde(default = "default_min_dist")]
    pub(crate) min_dist: f32,
    #[serde(default = "default_metric")]
    pub(crate) metric: String,

    // HDBSCAN parameters
    #[serde(default = "default_min_cluster_size")]
    pub(crate) min_cluster_size: i32,
    #[serde(default = "default_min_samples")]
    pub(crate) min_samples: Option<i32>,
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            n_neighbors: default_n_neighbors(),
            n_components: default_n_components(),
            min_dist: default_min_dist(),
            metric: default_metric(),
            min_cluster_size: default_min_cluster_size(),
            min_samples: default_min_samples(),
        }
    }
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

fn default_n_neighbors() -> i32 {
    15
}

fn default_n_components() -> i32 {
    3
}

fn default_min_dist() -> f32 {
    0.1
}

fn default_metric() -> String {
    "cosine".to_string()
}

fn default_min_cluster_size() -> i32 {
    15
}

fn default_min_samples() -> Option<i32> {
    Some(5)
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
    pub(crate) source_transform_id: Option<i32>,
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
        Self::generate_collection_name_with_suffix(
            dataset_id,
            embedder_id,
            transform_id,
            owner,
            None,
        )
    }

    pub(crate) fn generate_collection_name_with_suffix(
        dataset_id: i32,
        embedder_id: i32,
        transform_id: i32,
        owner: &str,
        suffix: Option<&str>,
    ) -> String {
        let base = format!(
            "dataset-{}-embedder-{}-transform-{}-{}",
            dataset_id, embedder_id, transform_id, owner
        );

        if let Some(suffix) = suffix {
            format!("{}-{}", base, suffix)
        } else {
            base
        }
    }

    pub(crate) fn collection_name_with_suffix(
        &self,
        embedder_id: i32,
        suffix: Option<&str>,
    ) -> String {
        Self::generate_collection_name_with_suffix(
            self.dataset_id,
            embedder_id,
            self.transform_id,
            &self.owner,
            suffix,
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

#[derive(Serialize, Deserialize, ToSchema)]
#[allow(dead_code)]
pub(crate) struct EmbeddedDatasetInfo {
    pub(crate) transform_id: i32,
    pub(crate) title: String,
    pub(crate) embedder_id: i32,
    pub(crate) embedder_name: String,
    pub(crate) collection_name: String,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) created_at: DateTime<Utc>,
}
