use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub chunk_size: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_metadata: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub structure_info: Option<StructureInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_hierarchy: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ChunkWithStructure {
    pub content: String,
    pub structure_info: Option<StructureInfo>,
}

impl ChunkMetadata {
    pub fn new(
        chunk_index: usize,
        total_chunks: usize,
        chunk_size: usize,
        extraction_metadata: Option<serde_json::Value>,
        structure_info: Option<StructureInfo>,
    ) -> Self {
        Self {
            chunk_index,
            total_chunks,
            chunk_size,
            extraction_metadata,
            structure_info,
        }
    }
}
