use anyhow::Result;
use semantic_explorer_core::jobs::EmbedderConfig;
use serde::{Deserialize, Serialize};

use super::config::{ChunkingConfig, ChunkingStrategy};
use super::metadata::{ChunkMetadata, ChunkWithStructure as StrategyChunkWithStructure};
use super::strategies;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkWithMetadata {
    pub content: String,
    pub metadata: ChunkMetadata,
}

pub struct ChunkingService;

impl ChunkingService {
    pub async fn chunk_text(
        text: String,
        config: &ChunkingConfig,
        extraction_metadata: Option<serde_json::Value>,
        embedder_config: Option<&EmbedderConfig>,
    ) -> Result<Vec<ChunkWithMetadata>> {
        let structured_chunks = if matches!(config.strategy, ChunkingStrategy::MarkdownAware) {
            strategies::markdown_aware::chunk(text.clone(), config)
                .ok()
                .map(|chunks| {
                    chunks
                        .into_iter()
                        .map(|c| StrategyChunkWithStructure {
                            content: c.content,
                            structure_info: c.structure_info,
                        })
                        .collect::<Vec<_>>()
                })
        } else {
            None
        };

        let (chunks, structure_infos): (Vec<String>, Vec<Option<_>>) = if let Some(structured) =
            structured_chunks
        {
            structured
                .into_iter()
                .map(|c| (c.content, c.structure_info))
                .unzip()
        } else {
            // Fallback to regular chunking without structure
            let chunks = match config.strategy {
                ChunkingStrategy::Sentence => strategies::sentence::chunk(text.clone(), config)?,
                ChunkingStrategy::RecursiveCharacter => {
                    strategies::recursive_character::chunk(text.clone(), config)?
                }
                ChunkingStrategy::Semantic => {
                    strategies::semantic::chunk_async(text.clone(), config, embedder_config).await?
                }
                ChunkingStrategy::FixedSize => strategies::fixed_size::chunk(text.clone(), config)?,
                ChunkingStrategy::MarkdownAware => {
                    unreachable!("MarkdownAware is handled in the structured_chunks path above")
                }
            };
            let len = chunks.len();
            (chunks, vec![None; len])
        };

        let chunks_with_overlap = if config.chunk_overlap > 0 {
            strategies::overlap::apply_overlap(chunks, config.chunk_overlap)?
        } else {
            chunks
        };

        let total_chunks = chunks_with_overlap.len();
        let chunks_with_metadata = chunks_with_overlap
            .into_iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let structure_info = structure_infos.get(idx).and_then(|s| s.clone());
                let metadata = ChunkMetadata::new(
                    idx,
                    total_chunks,
                    chunk.len(),
                    extraction_metadata.clone(),
                    structure_info,
                );

                ChunkWithMetadata {
                    content: chunk,
                    metadata,
                }
            })
            .collect();

        Ok(chunks_with_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::config::{ChunkingOptions, MarkdownAwareOptions, RecursiveCharacterOptions};

    fn create_basic_config(strategy: ChunkingStrategy, chunk_size: usize) -> ChunkingConfig {
        ChunkingConfig {
            strategy,
            chunk_size,
            chunk_overlap: 0,
            options: ChunkingOptions::default(),
        }
    }

    #[tokio::test]
    async fn test_sentence_chunking() {
        let text = "First sentence. Second sentence. Third sentence.".to_string();
        let config = create_basic_config(ChunkingStrategy::Sentence, 30);

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());

        // Verify metadata
        for (idx, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.metadata.chunk_index, idx);
            assert_eq!(chunk.metadata.total_chunks, chunks.len());
            assert_eq!(chunk.metadata.chunk_size, chunk.content.len());
        }
    }

    #[tokio::test]
    async fn test_fixed_size_chunking() {
        let text = "a".repeat(100);
        let config = create_basic_config(ChunkingStrategy::FixedSize, 25);

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 4);

        for chunk in &chunks {
            assert_eq!(chunk.content.len(), 25);
        }
    }

    #[tokio::test]
    async fn test_recursive_character_chunking() {
        let text = "Paragraph 1\n\nParagraph 2\n\nParagraph 3".to_string();
        let mut config = create_basic_config(ChunkingStrategy::RecursiveCharacter, 30);
        config.options.recursive_character = Some(RecursiveCharacterOptions::default());

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[tokio::test]
    async fn test_markdown_aware_chunking() {
        let text = "# Header 1\nContent 1\n\n## Header 2\nContent 2".to_string();
        let mut config = create_basic_config(ChunkingStrategy::MarkdownAware, 100);
        config.options.markdown_aware = Some(MarkdownAwareOptions {
            split_on_headers: true,
            preserve_code_blocks: true,
        });

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[tokio::test]
    async fn test_chunking_with_overlap() {
        let text = "Chunk one content. Chunk two content. Chunk three content.".to_string();
        let mut config = create_basic_config(ChunkingStrategy::Sentence, 25);
        config.chunk_overlap = 5;

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(chunks.len() >= 2);

        // Second chunk should have overlap from first
        if chunks.len() > 1 {
            assert!(!chunks[0].content.is_empty());
            assert!(!chunks[1].content.is_empty());
        }
    }

    #[tokio::test]
    async fn test_empty_text() {
        let text = String::new();
        let config = create_basic_config(ChunkingStrategy::Sentence, 100);

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_chunking_with_extraction_metadata() {
        let text = "Test content for metadata.".to_string();
        let config = create_basic_config(ChunkingStrategy::Sentence, 100);
        let metadata = Some(serde_json::json!({
            "source": "test.txt",
            "page": 1
        }));

        let result = ChunkingService::chunk_text(text, &config, metadata.clone(), None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].metadata.extraction_metadata.is_some());

        let chunk_metadata = chunks[0].metadata.extraction_metadata.as_ref().unwrap();
        assert_eq!(chunk_metadata["source"], "test.txt");
        assert_eq!(chunk_metadata["page"], 1);
    }

    #[tokio::test]
    async fn test_single_sentence() {
        let text = "This is one sentence.".to_string();
        let config = create_basic_config(ChunkingStrategy::Sentence, 100);

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "This is one sentence.");
        assert_eq!(chunks[0].metadata.chunk_index, 0);
        assert_eq!(chunks[0].metadata.total_chunks, 1);
    }

    #[tokio::test]
    async fn test_unicode_content() {
        let text = "Hello 世界! Testing émojis 🚀 and ñoño.".to_string();
        let config = create_basic_config(ChunkingStrategy::Sentence, 100);

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("世界"));
        assert!(chunks[0].content.contains("🚀"));
    }

    #[tokio::test]
    async fn test_metadata_structure() {
        let text = "Test chunk.".to_string();
        let config = create_basic_config(ChunkingStrategy::Sentence, 100);

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);

        let chunk = &chunks[0];
        assert_eq!(chunk.content, "Test chunk.");
        assert_eq!(chunk.metadata.chunk_index, 0);
        assert_eq!(chunk.metadata.total_chunks, 1);
        assert_eq!(chunk.metadata.chunk_size, "Test chunk.".len());
        assert!(chunk.metadata.extraction_metadata.is_none());
        assert!(chunk.metadata.structure_info.is_none());
    }

    #[tokio::test]
    async fn test_markdown_with_structure_info() {
        let text = "# Introduction\nThis is the intro.\n\n## Section 1\nContent for section 1.\n\n## Section 2\nContent for section 2.".to_string();
        let mut config = create_basic_config(ChunkingStrategy::MarkdownAware, 100);
        config.options.markdown_aware = Some(MarkdownAwareOptions {
            split_on_headers: true,
            preserve_code_blocks: true,
        });

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());

        // Check that structure info is populated
        for chunk in &chunks {
            if chunk.content.contains("Section") {
                // Chunks with headers should have structure info
                assert!(
                    chunk.metadata.structure_info.is_some(),
                    "Chunk should have structure info: {}",
                    chunk.content
                );

                let structure = chunk.metadata.structure_info.as_ref().unwrap();
                assert!(structure.heading_hierarchy.is_some());
                assert!(structure.section_title.is_some());

                let hierarchy = structure.heading_hierarchy.as_ref().unwrap();
                assert!(!hierarchy.is_empty(), "Hierarchy should not be empty");
            }
        }
    }

    #[tokio::test]
    async fn test_markdown_heading_hierarchy() {
        let text = "# Main Title\n\n## Subsection A\nContent A\n\n### Deep Section\nDeep content\n\n## Subsection B\nContent B".to_string();
        let mut config = create_basic_config(ChunkingStrategy::MarkdownAware, 200);
        config.options.markdown_aware = Some(MarkdownAwareOptions {
            split_on_headers: true,
            preserve_code_blocks: true,
        });
        config.options.min_chunk_size = 10;

        let result = ChunkingService::chunk_text(text, &config, None, None).await;
        assert!(result.is_ok());

        let chunks = result.unwrap();

        // Verify that heading hierarchy is correctly tracked
        for chunk in &chunks {
            if let Some(ref structure) = chunk.metadata.structure_info {
                if let Some(ref hierarchy) = structure.heading_hierarchy {
                    // Check that hierarchy is in the correct format
                    assert!(!hierarchy.is_empty());

                    // If section title exists, it should be the last item in hierarchy
                    if let Some(ref title) = structure.section_title {
                        assert_eq!(hierarchy.last().unwrap(), title);
                    }
                }
            }
        }
    }
}
