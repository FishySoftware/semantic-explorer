use actix_multipart::form::{MultipartForm, tempfile::TempFile};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema, Clone)]
pub(crate) struct FailedUploadFile {
    pub(crate) name: String,
    pub(crate) error: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CollectionUploadResponse {
    pub(crate) completed: Vec<String>,
    pub(crate) failed: Vec<FailedUploadFile>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateCollection {
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) is_public: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct UpdateCollection {
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) is_public: bool,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct Collection {
    pub(crate) collection_id: i32,
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) owner_id: String,
    pub(crate) owner_display_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) is_public: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    #[sqlx(default)]
    pub(crate) file_count: i64,
    #[sqlx(default)]
    pub(crate) failed_file_count: i64,
    #[sqlx(default)]
    pub(crate) transform_count: i64,
}

impl Collection {
    pub(crate) fn s3_folder_key(&self) -> String {
        format!("collections/{}/", self.collection_id)
    }
}

#[derive(MultipartForm, ToSchema)]
pub(crate) struct CollectionUpload {
    #[multipart(rename = "files", limit = "1024MB")]
    #[schema(value_type = Vec<String>, format = Binary)]
    pub(crate) files: Vec<TempFile>,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct FileListQuery {
    #[serde(default = "default_page_size")]
    pub(crate) page_size: i32,
    pub(crate) continuation_token: Option<String>,
}

fn default_page_size() -> i32 {
    10
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct CollectionSearchQuery {
    #[serde(default)]
    pub(crate) q: Option<String>,
    #[serde(default = "default_collection_limit")]
    pub(crate) limit: i64,
    #[serde(default)]
    pub(crate) offset: i64,
}

fn default_collection_limit() -> i64 {
    10
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PaginatedCollections {
    pub(crate) collections: Vec<Collection>,
    pub(crate) total_count: i64,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}
