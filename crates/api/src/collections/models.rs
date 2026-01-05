use actix_multipart::form::{MultipartForm, tempfile::TempFile};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(crate) struct CollectionUploadResponse {
    pub(crate) completed: Vec<String>,
    pub(crate) failed: Vec<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct CreateCollection {
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Serialize, ToSchema, FromRow)]
pub(crate) struct Collection {
    pub(crate) collection_id: i32,
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) owner: String,
    pub(crate) bucket: String,
    pub(crate) tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) created_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) updated_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

#[derive(MultipartForm, ToSchema)]
pub(crate) struct CollectionUpload {
    #[multipart(limit = "1024MB")]
    #[schema(value_type = String, format = Binary, content_media_type = "application/octet-stream")]
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
