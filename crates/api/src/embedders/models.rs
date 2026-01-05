use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateEmbedder {
    pub(crate) name: String,
    pub(crate) provider: String,
    pub(crate) base_url: String,
    pub(crate) api_key: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) config: serde_json::Value,
    #[serde(default = "default_batch_size")]
    pub(crate) batch_size: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) collection_name: Option<String>,
}

fn default_batch_size() -> i32 {
    50
}

#[derive(Serialize, ToSchema, FromRow, Clone)]
pub(crate) struct Embedder {
    pub(crate) embedder_id: i32,
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) provider: String,
    pub(crate) base_url: String,
    pub(crate) api_key: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) config: serde_json::Value,
    pub(crate) batch_size: i32,
    pub(crate) collection_name: Option<String>,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct UpdateEmbedder {
    pub(crate) name: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) config: Option<serde_json::Value>,
    pub(crate) batch_size: Option<i32>,
    pub(crate) collection_name: Option<String>,
}
