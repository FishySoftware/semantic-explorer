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
    TokenBased,
    CodeAware,
    TableAware,
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

/// Options for token-based chunking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBasedOptions {
    /// Maximum number of tokens per chunk (default: 512)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Number of tokens to overlap between chunks (default: 0)
    #[serde(default)]
    pub overlap_tokens: usize,

    /// Whether to split on sentence boundaries when possible (default: true)
    #[serde(default = "default_split_on_sentences")]
    pub split_on_sentences: bool,

    /// Optional model name for tokenizer selection (default: cl100k_base for GPT-4/ada-002)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for TokenBasedOptions {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            overlap_tokens: 0,
            split_on_sentences: true,
            model: None,
        }
    }
}

/// Options for code-aware chunking using tree-sitter
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeAwareOptions {
    /// Programming language (file extension or mime type). Auto-detected if None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Maximum chunk size in bytes (default: uses chunk_size from config)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_chunk_size: Option<usize>,

    /// Minimum chunk size before merging with neighbors (default: 50)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_chunk_size: Option<usize>,

    /// Whether to include import statements as context (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_imports: Option<bool>,

    /// Whether to preserve comments with their associated code (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_comments: Option<bool>,
}

/// Options for table-aware chunking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableAwareOptions {
    /// Maximum rows per chunk (default: 50)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rows_per_chunk: Option<usize>,

    /// Whether to include column headers with each chunk (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_headers: Option<bool>,

    /// Maximum chunk size in bytes (default: uses chunk_size from config)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_chunk_size: Option<usize>,

    /// Minimum rows before creating a separate table chunk (default: 3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_rows_for_table: Option<usize>,
}

/// Options for streaming chunking (memory-efficient large document processing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingOptions {
    /// Maximum chunk size in bytes (default: 2000)
    #[serde(default = "default_streaming_chunk_size")]
    pub max_chunk_size: usize,

    /// Read buffer size in bytes (default: 64KB)
    #[serde(default = "default_streaming_buffer_size")]
    pub buffer_size: usize,

    /// Overlap between chunks in bytes (default: 0)
    #[serde(default)]
    pub overlap_bytes: usize,

    /// Whether to split only on line boundaries (default: true)
    #[serde(default = "default_true")]
    pub line_boundary_only: bool,

    /// Progress callback interval in bytes (default: None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_interval: Option<usize>,
}

impl Default for StreamingOptions {
    fn default() -> Self {
        Self {
            max_chunk_size: default_streaming_chunk_size(),
            buffer_size: default_streaming_buffer_size(),
            overlap_bytes: 0,
            line_boundary_only: true,
            progress_interval: None,
        }
    }
}

fn default_streaming_chunk_size() -> usize {
    2000
}

fn default_streaming_buffer_size() -> usize {
    64 * 1024 // 64KB
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<SemanticOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursive_character: Option<RecursiveCharacterOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_aware: Option<MarkdownAwareOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_based: Option<TokenBasedOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_aware: Option<CodeAwareOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_aware: Option<TableAwareOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<StreamingOptions>,

    #[serde(default = "default_preserve_sentence_boundaries")]
    pub preserve_sentence_boundaries: bool,

    #[serde(default = "default_trim_whitespace")]
    pub trim_whitespace: bool,

    #[serde(default = "default_min_chunk_size")]
    pub min_chunk_size: usize,
}

impl Default for ChunkingOptions {
    fn default() -> Self {
        Self {
            semantic: None,
            recursive_character: None,
            markdown_aware: None,
            token_based: None,
            code_aware: None,
            table_aware: None,
            streaming: None,
            preserve_sentence_boundaries: default_preserve_sentence_boundaries(),
            trim_whitespace: default_trim_whitespace(),
            min_chunk_size: default_min_chunk_size(),
        }
    }
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

fn default_max_tokens() -> usize {
    512
}

fn default_split_on_sentences() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_chunking_config_defaults() {
        let config = ChunkingConfig::default();
        assert!(matches!(config.strategy, ChunkingStrategy::Sentence));
        assert_eq!(config.chunk_size, 200);
        assert_eq!(config.chunk_overlap, 0);
        // Test nested default options
        assert!(config.options.preserve_sentence_boundaries);
    }

    #[test]
    fn test_deserialize_minimal_config() {
        let json = json!({
            "strategy": "fixed_size",
            "chunk_size": 500
        });

        let config: ChunkingConfig = serde_json::from_value(json).unwrap();
        assert!(matches!(config.strategy, ChunkingStrategy::FixedSize));
        assert_eq!(config.chunk_size, 500);
        // options should get defaults
        assert_eq!(config.chunk_overlap, 0);
    }

    #[test]
    fn test_deserialize_full_recursive_config() {
        let json = json!({
            "strategy": "recursive_character",
            "chunk_size": 1000,
            "chunk_overlap": 100,
            "options": {
                "recursive_character": {
                    "separators": ["|", ";"],
                    "keep_separator": false
                }
            }
        });

        let config: ChunkingConfig = serde_json::from_value(json).unwrap();
        assert!(matches!(
            config.strategy,
            ChunkingStrategy::RecursiveCharacter
        ));
        assert_eq!(config.chunk_size, 1000);

        let opts = config.options.recursive_character.unwrap();
        assert_eq!(opts.separators, vec!["|", ";"]);
        assert!(!opts.keep_separator);
    }

    #[test]
    fn test_deserialize_token_based_options() {
        let json = json!({
            "strategy": "token_based",
            "options": {
                "token_based": {
                    "max_tokens": 1024,
                    "split_on_sentences": false,
                    "model": "gpt-4"
                }
            }
        });

        let config: ChunkingConfig = serde_json::from_value(json).unwrap();
        let opts = config.options.token_based.unwrap();
        assert_eq!(opts.max_tokens, 1024);
        assert!(!opts.split_on_sentences);
        assert_eq!(opts.model, Some("gpt-4".to_string()));
    }
}
