use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{
    FromRow,
    postgres::PgRow,
    types::chrono::{DateTime, Utc},
};
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

fn non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Err(serde::de::Error::custom(
            "must not be empty or contain only whitespace",
        ))
    } else {
        Ok(trimmed.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ChunkWithMetadata {
    #[serde(deserialize_with = "non_empty_string")]
    pub(crate) content: String,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateDataset {
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) is_public: bool,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct Dataset {
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) owner_id: String,
    pub(crate) owner_display_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) is_public: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct DatasetWithStats {
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) owner_id: String,
    pub(crate) owner_display_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) is_public: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<DateTime<Utc>>,
    pub(crate) item_count: i64,
    pub(crate) total_chunks: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PaginatedDatasetList {
    pub(crate) items: Vec<DatasetWithStats>,
    pub(crate) total_count: i64,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

#[derive(Serialize, Deserialize, ToSchema, FromRow)]
pub(crate) struct CreateDatasetItems {
    #[serde(deserialize_with = "non_empty")]
    pub(crate) items: Vec<CreateDatasetItem>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateDatasetItem {
    #[serde(deserialize_with = "non_empty_string")]
    pub(crate) title: String,
    #[serde(deserialize_with = "non_empty")]
    pub(crate) chunks: Vec<ChunkWithMetadata>,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreateDatasetItemsResponse {
    pub(crate) completed: Vec<String>,
    pub(crate) failed: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct DatasetItem {
    pub(crate) item_id: i32,
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) chunks: Vec<ChunkWithMetadata>,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<DateTime<Utc>>,
}

impl sqlx::FromRow<'_, PgRow> for DatasetItem {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let chunks_json: serde_json::Value = row.try_get("chunks")?;
        let chunks: Vec<ChunkWithMetadata> =
            serde_json::from_value(chunks_json).map_err(|e| sqlx::Error::ColumnDecode {
                index: "chunks".to_string(),
                source: Box::new(e),
            })?;

        Ok(DatasetItem {
            item_id: row.try_get("item_id")?,
            dataset_id: row.try_get("dataset_id")?,
            title: row.try_get("title")?,
            chunks,
            metadata: row.try_get("metadata")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PaginatedDatasetItems {
    pub(crate) items: Vec<DatasetItem>,
    pub(crate) page: i64,
    pub(crate) page_size: i64,
    pub(crate) total_count: i64,
    pub(crate) has_more: bool,
}

/// Dataset item summary without chunks (for efficient listing)
#[derive(Serialize, ToSchema)]
pub(crate) struct DatasetItemSummary {
    pub(crate) item_id: i32,
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) chunk_count: i32,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<DateTime<Utc>>,
}

impl sqlx::FromRow<'_, PgRow> for DatasetItemSummary {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        Ok(DatasetItemSummary {
            item_id: row.try_get("item_id")?,
            dataset_id: row.try_get("dataset_id")?,
            title: row.try_get("title")?,
            chunk_count: row.try_get("chunk_count")?,
            metadata: row.try_get("metadata")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PaginatedDatasetItemSummaries {
    pub(crate) items: Vec<DatasetItemSummary>,
    pub(crate) page: i64,
    pub(crate) page_size: i64,
    pub(crate) total_count: i64,
    pub(crate) has_more: bool,
}

/// Response containing chunks for a single dataset item
#[derive(Serialize, ToSchema)]
pub(crate) struct DatasetItemChunks {
    pub(crate) item_id: i32,
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) chunks: Vec<ChunkWithMetadata>,
    #[schema(value_type = Object)]
    pub(crate) metadata: serde_json::Value,
}

#[derive(Deserialize)]
pub(crate) struct PaginationParams {
    #[serde(default)]
    pub(crate) page: i64,
    #[serde(default = "default_page_size")]
    pub(crate) page_size: i64,
    #[serde(default)]
    pub(crate) search: Option<String>,
}

fn default_page_size() -> i64 {
    10
}
