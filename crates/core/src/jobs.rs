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
