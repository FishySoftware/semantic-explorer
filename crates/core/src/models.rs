use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTransformJob {
    pub job_id: Uuid,
    pub source_file_key: String,
    pub bucket: String,
    pub collection_transform_id: i32,
    pub owner: String,
    pub extraction_config: serde_json::Value,
    pub chunking_config: serde_json::Value,
    /// Optional embedder config for semantic chunking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_config: Option<EmbedderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTransformResult {
    pub job_id: Uuid,
    pub collection_transform_id: i32,
    pub owner: String,
    pub source_file_key: String,
    pub bucket: String,
    pub chunks_file_key: String,
    pub chunk_count: usize,
    pub status: String,
    pub error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetTransformJob {
    pub job_id: Uuid,
    pub batch_file_key: String,
    pub bucket: String,
    pub dataset_transform_id: i32,
    pub embedded_dataset_id: i32, // NEW: Identifies which embedded dataset this job is for
    pub owner: String,
    pub embedder_config: EmbedderConfig,
    pub vector_database_config: VectorDatabaseConfig,
    pub collection_name: String,
    #[serde(default)]
    pub wipe_collection: bool,
    #[serde(default)]
    pub batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetTransformResult {
    pub job_id: Uuid,
    pub dataset_transform_id: i32,
    pub embedded_dataset_id: i32, // NEW: Identifies which embedded dataset this result is for
    pub owner: String,
    pub batch_file_key: String,
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
    #[serde(default = "default_max_input_tokens")]
    pub max_input_tokens: i32,
}

fn default_max_input_tokens() -> i32 {
    8191 // OpenAI default for text-embedding-ada-002
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDatabaseConfig {
    pub database_type: String,
    pub connection_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub llm_id: i32,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationTransformJob {
    pub job_id: Uuid,
    pub visualization_transform_id: i32,
    pub run_id: i32,
    pub owner: String,
    pub embedded_dataset_id: i32,
    pub qdrant_collection_name: String,
    pub visualization_config: VisualizationConfig,
    pub vector_database_config: VectorDatabaseConfig,
    pub llm_config: Option<LLMConfig>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_naming_llm_id: Option<i32>, // LLM database ID when mode = "llm"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationTransformResult {
    pub job_id: Uuid,
    pub visualization_transform_id: i32,
    pub run_id: i32,
    pub owner: String,
    pub status: String,
    pub error_message: Option<String>,
    pub html_s3_key: Option<String>,
    pub point_count: Option<usize>,
    pub cluster_count: Option<i32>,
    pub processing_duration_ms: Option<i64>,
    pub stats_json: Option<serde_json::Value>,
}

#[deprecated(note = "Use VisualizationTransformResult instead")]
pub type VisualizationResult = VisualizationTransformResult;
