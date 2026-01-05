use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

fn non_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v: Vec<T> = Vec::deserialize(deserializer)?;
    if v.is_empty() {
        Err(serde::de::Error::custom("list must not be empty"))
    } else {
        Ok(v)
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateDataset {
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct Dataset {
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) owner: String,
    pub(crate) tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

#[derive(Serialize, Deserialize, ToSchema, FromRow)]
pub(crate) struct CreateDatasetItems {
    #[serde(deserialize_with = "non_empty")]
    pub(crate) items: Vec<CreateDatasetItem>,
}

#[derive(Serialize, Deserialize, ToSchema, FromRow)]
pub(crate) struct CreateDatasetItem {
    pub(crate) title: String,
    #[serde(deserialize_with = "non_empty")]
    pub(crate) chunks: Vec<String>,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreateDatasetItemsResponse {
    pub(crate) completed: Vec<String>,
    pub(crate) failed: Vec<String>,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct DatasetItem {
    pub(crate) item_id: i32,
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) chunks: Vec<String>,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PaginatedDatasetItems {
    pub(crate) items: Vec<DatasetItem>,
    pub(crate) page: i64,
    pub(crate) page_size: i64,
    pub(crate) total_count: i64,
    pub(crate) has_more: bool,
}

#[derive(Deserialize)]
pub(crate) struct PaginationParams {
    #[serde(default)]
    pub(crate) page: i64,
    #[serde(default = "default_page_size")]
    pub(crate) page_size: i64,
}

fn default_page_size() -> i64 {
    10
}
