use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub(crate) struct SearchRequest {
    pub query: String,
    pub embedded_dataset_ids: Vec<i32>,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub score_threshold: f32,
    #[serde(default)]
    pub filters: Option<serde_json::Value>,
    #[serde(default)]
    pub search_params: Option<SearchParams>,
    #[serde(default)]
    pub search_mode: SearchMode,
}

#[derive(Debug, Deserialize, Serialize, ToSchema, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchMode {
    #[default]
    Documents,
    Chunks,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SearchParams {
    #[serde(default)]
    pub exact: bool,
    pub hnsw_ef: Option<u64>,
}

fn default_limit() -> u64 {
    10
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct SearchResponse {
    pub results: Vec<EmbeddedDatasetSearchResults>,
    pub query: String,
    pub search_mode: SearchMode,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct DocumentResult {
    pub item_id: i32,
    pub item_title: String,
    pub best_score: f32,
    pub chunk_count: i32,
    pub best_chunk: SearchMatch,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EmbeddedDatasetSearchResults {
    pub embedded_dataset_id: i32,
    pub embedded_dataset_title: String,
    pub source_dataset_id: i32,
    pub source_dataset_title: String,
    pub embedder_id: i32,
    pub embedder_name: String,
    pub collection_name: String,
    pub matches: Vec<SearchMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<DocumentResult>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct SearchMatch {
    pub id: String,
    pub score: f32,
    pub text: String,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
}
