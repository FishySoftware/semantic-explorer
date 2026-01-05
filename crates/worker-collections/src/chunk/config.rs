use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ChunkingStrategy {
    #[default]
    Sentence,
    RecursiveCharacter,
    Semantic,
    FixedSize,
    MarkdownAware,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_id: Option<i32>,

    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    #[serde(default = "default_min_chunk_size")]
    pub min_chunk_size: usize,

    #[serde(default = "default_max_chunk_size")]
    pub max_chunk_size: usize,

    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecursiveCharacterOptions {
    #[serde(default = "default_separators")]
    pub separators: Vec<String>,

    #[serde(default = "default_keep_separator")]
    pub keep_separator: bool,
}

impl Default for RecursiveCharacterOptions {
    fn default() -> Self {
        Self {
            separators: default_separators(),
            keep_separator: default_keep_separator(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarkdownAwareOptions {
    #[serde(default = "default_split_on_headers")]
    pub split_on_headers: bool,

    #[serde(default)]
    pub preserve_code_blocks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChunkingOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<SemanticOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursive_character: Option<RecursiveCharacterOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_aware: Option<MarkdownAwareOptions>,

    #[serde(default = "default_preserve_sentence_boundaries")]
    pub preserve_sentence_boundaries: bool,

    #[serde(default = "default_trim_whitespace")]
    pub trim_whitespace: bool,

    #[serde(default = "default_min_chunk_size")]
    pub min_chunk_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    #[serde(default)]
    pub strategy: ChunkingStrategy,

    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    #[serde(default)]
    pub chunk_overlap: usize,

    #[serde(default)]
    pub options: ChunkingOptions,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            strategy: ChunkingStrategy::Sentence,
            chunk_size: default_chunk_size(),
            chunk_overlap: 0,
            options: ChunkingOptions::default(),
        }
    }
}

fn default_similarity_threshold() -> f32 {
    0.7
}

fn default_min_chunk_size() -> usize {
    50
}

fn default_max_chunk_size() -> usize {
    500
}

fn default_buffer_size() -> usize {
    1
}

fn default_separators() -> Vec<String> {
    vec![
        "\n\n".to_string(),
        "\n".to_string(),
        ". ".to_string(),
        " ".to_string(),
        "".to_string(),
    ]
}

fn default_keep_separator() -> bool {
    true
}

fn default_split_on_headers() -> bool {
    true
}

fn default_preserve_sentence_boundaries() -> bool {
    true
}

fn default_trim_whitespace() -> bool {
    true
}

fn default_chunk_size() -> usize {
    200
}
