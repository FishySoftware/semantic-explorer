use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone)]
pub(crate) struct DocumentUpload {
    pub(crate) collection_id: String,
    pub(crate) name: String,
    pub(crate) content: Vec<u8>,
    pub(crate) mime_type: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct CollectionFile {
    pub(crate) key: String,
    pub(crate) size: i64,
    pub(crate) last_modified: Option<String>,
    pub(crate) content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct PaginatedFiles {
    pub(crate) files: Vec<CollectionFile>,
    pub(crate) page: i32,
    pub(crate) page_size: i32,
    pub(crate) has_more: bool,
    pub(crate) continuation_token: Option<String>,
    pub(crate) total_count: Option<i64>,
}
