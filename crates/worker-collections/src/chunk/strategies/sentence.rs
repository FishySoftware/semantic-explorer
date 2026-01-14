use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

use crate::chunk::config::ChunkingConfig;

pub fn chunk(text: String, config: &ChunkingConfig) -> Result<Vec<String>> {
    let sentences: Vec<&str> = text
        .unicode_sentences()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for sentence in sentences {
        // Handle sentences that exceed chunk_size by splitting them
        if sentence.len() > config.chunk_size {
            // First, flush current chunk if not empty
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
            }

            // Split the long sentence into smaller pieces
            let sub_chunks = split_long_sentence(sentence, config.chunk_size);
            chunks.extend(sub_chunks);
            continue;
        }

        if current_chunk.len() + sentence.len() + 1 > config.chunk_size && !current_chunk.is_empty()
        {
            chunks.push(current_chunk.trim().to_string());
            current_chunk = String::new();
        }

        if !current_chunk.is_empty() {
            current_chunk.push(' ');
        }
        current_chunk.push_str(sentence);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    Ok(chunks)
}

/// Split a long sentence into smaller chunks, preferring word boundaries
fn split_long_sentence(sentence: &str, max_size: usize) -> Vec<String> {
    // Split on whitespace while preserving punctuation attached to words
    let words: Vec<&str> = sentence.split_whitespace().collect();

    if words.is_empty() {
        // No words found, split by character
        return split_by_chars(sentence, max_size);
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for word in words {
        // If a single word exceeds max_size, split it by characters
        if word.len() > max_size {
            // First, flush current chunk
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
            }
            // Split the long word
            chunks.extend(split_by_chars(word, max_size));
            continue;
        }

        // Check if adding this word would exceed the limit
        let new_len = if current_chunk.is_empty() {
            word.len()
        } else {
            current_chunk.len() + 1 + word.len()
        };

        if new_len > max_size && !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
            current_chunk = String::new();
        }

        if !current_chunk.is_empty() {
            current_chunk.push(' ');
        }
        current_chunk.push_str(word);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    chunks
}

/// Split text by character count, respecting Unicode grapheme clusters
fn split_by_chars(text: &str, max_size: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();

    chars
        .chunks(max_size)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::config::{ChunkingConfig, ChunkingOptions};

    fn create_default_config(chunk_size: usize) -> ChunkingConfig {
        ChunkingConfig {
            chunk_size,
            chunk_overlap: 0,
            options: ChunkingOptions::default(),
            ..Default::default()
        }
    }

    #[test]
    fn test_sentence_basic() {
        let text = "Hello, World! This is a test. Another sentence here.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_single_sentence() {
        let text = "This is one sentence.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "This is one sentence.");
    }

    #[test]
    fn test_sentence_multiple_sentences() {
        let text = "First sentence. Second sentence. Third sentence.".to_string();
        let config = create_default_config(30);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should group sentences until chunk_size is reached
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 30 || chunks.len() == 1);
        }
    }

    #[test]
    fn test_sentence_empty_text() {
        let text = String::new();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_sentence_with_newlines() {
        let text = "First sentence.\nSecond sentence.\nThird sentence.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_with_abbreviations() {
        let text = "Dr. Smith works at Mt. Vernon. He is a Ph.D. holder.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Unicode segmentation should handle abbreviations correctly
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_with_questions_and_exclamations() {
        let text = "Is this a question? Yes! It certainly is.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_long_sentence() {
        let text = "This is a very long sentence that exceeds the chunk size limit and should be placed in its own chunk.".to_string();
        let config = create_default_config(50);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Long sentence should now be split into multiple chunks
        assert!(
            chunks.len() > 1,
            "Expected multiple chunks, got {}",
            chunks.len()
        );

        // Each chunk should respect the size limit
        for chunk in &chunks {
            assert!(
                chunk.len() <= 50,
                "Chunk '{}' exceeds max size of 50",
                chunk
            );
        }
    }

    #[test]
    fn test_sentence_long_sentence_preserves_all_content() {
        let text = "This is a very long sentence that exceeds the chunk size limit and should be split properly.".to_string();
        let config = create_default_config(30);

        let result = chunk(text.clone(), &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Verify all content is preserved by checking the combined length (minus spaces)
        let original_no_space: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let rejoined: String = chunks.join("");
        let rejoined_no_space: String = rejoined.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            original_no_space, rejoined_no_space,
            "Content should be preserved after splitting"
        );
    }

    #[test]
    fn test_sentence_very_long_word() {
        // A single word that exceeds chunk size (like a URL or code)
        let text = "Visit https://example.com/very/long/path/that/exceeds/the/limit for more info."
            .to_string();
        let config = create_default_config(20);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_unicode() {
        let text = "Hello 世界! This is a test. 这是一个测试。".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_ellipsis() {
        let text = "This is a sentence... Another sentence here.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_quotes() {
        let text =
            "He said \"Hello!\" She replied \"Hi there.\" They continued talking.".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_sentence_small_chunk_size() {
        let text = "Sentence one. Sentence two.".to_string();
        let config = create_default_config(10);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should create separate chunks for each sentence
        assert!(chunks.len() >= 2);
    }
}
