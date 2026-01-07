use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct SearchRequest {
    pub query: String,
    pub dataset_id: i32,
    pub embeddings: std::collections::HashMap<i32, Vec<f32>>,
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
    Sources,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
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
    pub results: Vec<EmbedderSearchResults>,
    pub query: String,
    pub search_mode: SearchMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregated_sources: Option<Vec<SourceAggregation>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct SourceAggregation {
    pub source: String,
    pub matches: Vec<SearchMatch>,
    pub best_score: f32,
    pub embedder_ids: Vec<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EmbedderSearchResults {
    pub embedder_id: i32,
    pub embedder_name: String,
    pub collection_name: String,
    pub matches: Vec<SearchMatch>,
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
