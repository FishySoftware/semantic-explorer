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
        // Long sentence should still be in one chunk
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
