use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateLLM {
    pub(crate) name: String,
    pub(crate) provider: String,
    pub(crate) base_url: String,
    pub(crate) api_key: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) config: serde_json::Value,
    #[serde(default)]
    pub(crate) is_public: bool,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, ToSchema, FromRow, Clone)]
pub(crate) struct LargeLanguageModel {
    pub(crate) llm_id: i32,
    pub(crate) name: String,
    pub(crate) owner_id: String,
    pub(crate) owner_display_name: String,
    pub(crate) provider: String,
    pub(crate) base_url: String,
    #[sqlx(rename = "api_key_encrypted")]
    pub(crate) api_key: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) config: serde_json::Value,
    pub(crate) is_public: bool,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct UpdateLargeLanguageModel {
    pub(crate) name: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) config: Option<serde_json::Value>,
    pub(crate) is_public: Option<bool>,
}
