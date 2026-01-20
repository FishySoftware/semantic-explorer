use anyhow::Result;

use crate::chunk::config::ChunkingConfig;

pub fn chunk(text: String, config: &ChunkingConfig) -> Result<Vec<String>> {
    let opts = config
        .options
        .recursive_character
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let chunks = split_text_recursive(
        &text,
        &opts.separators,
        config.chunk_size,
        opts.keep_separator,
    );

    Ok(chunks)
}

fn split_text_recursive(
    text: &str,
    separators: &[String],
    chunk_size: usize,
    keep_separator: bool,
) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }

    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }

    for (i, separator) in separators.iter().enumerate() {
        if separator.is_empty() {
            return split_by_characters(text, chunk_size);
        }

        if text.contains(separator.as_str()) {
            let splits: Vec<&str> = text.split(separator.as_str()).collect();
            let mut chunks = Vec::new();
            let mut current_chunk = String::new();

            for split in splits {
                let potential_len = if current_chunk.is_empty() {
                    split.len()
                } else {
                    current_chunk.len() + separator.len() + split.len()
                };

                if potential_len <= chunk_size {
                    if !current_chunk.is_empty() && keep_separator {
                        current_chunk.push_str(separator);
                    } else if !current_chunk.is_empty() {
                        current_chunk.push(' ');
                    }
                    current_chunk.push_str(split);
                } else {
                    if !current_chunk.is_empty() {
                        chunks.push(current_chunk.clone());
                        current_chunk.clear();
                    }

                    if split.len() > chunk_size {
                        let sub_chunks = split_text_recursive(
                            split,
                            &separators[i + 1..],
                            chunk_size,
                            keep_separator,
                        );
                        chunks.extend(sub_chunks);
                    } else {
                        current_chunk = split.to_string();
                    }
                }
            }

            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
            }

            return chunks;
        }
    }

    split_by_characters(text, chunk_size)
}

fn split_by_characters(text: &str, chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();

    for chunk in chars.chunks(chunk_size) {
        chunks.push(chunk.iter().collect());
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::config::{ChunkingConfig, ChunkingOptions, RecursiveCharacterOptions};

    fn create_config(chunk_size: usize, separators: Vec<String>) -> ChunkingConfig {
        ChunkingConfig {
            chunk_size,
            chunk_overlap: 0,
            options: ChunkingOptions {
                recursive_character: Some(RecursiveCharacterOptions {
                    separators,
                    keep_separator: true,
                }),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_recursive_basic() {
        let text = "Paragraph 1\n\nParagraph 2\n\nParagraph 3".to_string();
        let separators = vec!["\n\n".to_string(), "\n".to_string(), " ".to_string()];
        let config = create_config(50, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        // Should split on double newlines when possible
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_recursive_with_default_separators() {
        let text = "Paragraph 1\n\nParagraph 2. Sentence 2. Sentence 3".to_string();
        let config = ChunkingConfig {
            chunk_size: 30,
            chunk_overlap: 0,
            options: ChunkingOptions {
                recursive_character: Some(RecursiveCharacterOptions::default()),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_recursive_empty_text() {
        let text = String::new();
        let separators = vec!["\n\n".to_string(), "\n".to_string()];
        let config = create_config(100, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_recursive_no_separators() {
        let text = "ThisIsOneLongStringWithoutAnySeparators".to_string();
        let separators = vec![" ".to_string(), "".to_string()];
        let config = create_config(10, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should split by characters
        assert!(chunks.len() >= 3);
        for chunk in &chunks {
            assert!(chunk.len() <= 10);
        }
    }

    #[test]
    fn test_recursive_hierarchical_splitting() {
        let text = "Section 1\n\nParagraph 1. Sentence 1. Sentence 2.\n\nSection 2".to_string();
        let separators = vec!["\n\n".to_string(), ". ".to_string(), " ".to_string()];
        let config = create_config(30, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        // Should split on double newline first, then periods, then spaces
    }

    #[test]
    fn test_recursive_single_separator() {
        let text = "Word1 Word2 Word3 Word4 Word5".to_string();
        let separators = vec![" ".to_string()];
        let config = create_config(12, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 12);
        }
    }

    #[test]
    fn test_recursive_keep_separator_false() {
        let text = "Part1\n\nPart2\n\nPart3".to_string();
        let mut config = create_config(20, vec!["\n\n".to_string()]);
        if let Some(opts) = &mut config.options.recursive_character {
            opts.keep_separator = false;
        }

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_recursive_unicode() {
        let text = "部分1\n\n部分2\n\n部分3".to_string();
        let separators = vec!["\n\n".to_string(), "\n".to_string()];
        let config = create_config(20, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_recursive_text_smaller_than_chunk() {
        let text = "Short".to_string();
        let separators = vec![" ".to_string()];
        let config = create_config(100, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short");
    }

    #[test]
    fn test_recursive_long_word() {
        let text = "ThisIsAVeryLongWordThatExceedsTheChunkSize".to_string();
        let separators = vec![" ".to_string(), "".to_string()];
        let config = create_config(10, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should be split by characters
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 10);
        }
    }

    #[test]
    fn test_recursive_multiple_empty_separators() {
        let text = "A B C D E F G".to_string();
        let separators = vec!["\n\n".to_string(), " ".to_string()];
        let config = create_config(5, separators);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }
}
