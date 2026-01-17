use aws_sdk_s3::primitives::ByteStream;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug)]
pub(crate) struct DocumentUpload {
    pub(crate) collection_id: String,
    pub(crate) name: String,
    pub(crate) content: ByteStream,
    pub(crate) mime_type: String,
    pub(crate) size: u64,
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

#[derive(Debug)]
pub(crate) struct S3FileList {
    pub(crate) files: Vec<CollectionFile>,
    pub(crate) continuation_token: Option<String>,
}
