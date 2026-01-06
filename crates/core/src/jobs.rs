use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformFileJob {
    pub job_id: Uuid,
    pub source_file_key: String,
    pub bucket: String,
    pub transform_id: i32,
    pub extraction_config: serde_json::Value,
    pub chunking_config: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_config: Option<EmbedderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransformResult {
    pub job_id: Uuid,
    pub transform_id: i32,
    pub source_file_key: String,
    pub bucket: String,
    pub chunks_file_key: String,
    pub chunk_count: usize,
    pub status: String,
    pub error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub config: serde_json::Value,
    pub max_batch_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDatabaseConfig {
    pub database_type: String,
    pub connection_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEmbedJob {
    pub job_id: Uuid,
    pub batch_file_key: String,
    pub bucket: String,
    pub transform_id: i32,
    pub embedder_config: EmbedderConfig,
    pub vector_database_config: VectorDatabaseConfig,
    pub collection_name: String,
    #[serde(default)]
    pub wipe_collection: bool,
    #[serde(default)]
    pub batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorBatchResult {
    pub job_id: Uuid,
    pub transform_id: i32,
    pub batch_file_key: String,
    pub chunk_count: usize,
    pub status: String,
    pub error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationTransformJob {
    pub job_id: Uuid,
    pub transform_id: i32,
    pub source_collection: String,
    pub output_collection_reduced: String,
    pub output_collection_topics: String,
    pub visualization_config: VisualizationConfig,
    pub vector_database_config: VectorDatabaseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    // UMAP parameters
    pub n_neighbors: i32,
    pub n_components: i32,
    pub min_dist: f32,
    pub metric: String,
    // HDBSCAN parameters
    pub min_cluster_size: i32,
    pub min_samples: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationResult {
    pub job_id: Uuid,
    pub transform_id: i32,
    pub status: String,
    pub error: Option<String>,
    pub processing_duration_ms: Option<i64>,
    pub n_points: usize,
    pub n_clusters: i32,
    pub output_collection_reduced: String,
    pub output_collection_topics: String,
}
