use anyhow::Result;

use crate::chunk::config::ChunkingConfig;

pub fn chunk(text: String, config: &ChunkingConfig) -> Result<Vec<String>> {
    if text.is_empty() {
        return Ok(vec![]);
    }

    let chunk_size = config.chunk_size;
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();

    let mut start = 0;
    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();

        if config.options.trim_whitespace {
            let trimmed = chunk.trim();
            if !trimmed.is_empty() {
                chunks.push(trimmed.to_string());
            }
        } else {
            chunks.push(chunk);
        }

        start = end;
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
    fn test_fixed_size_basic() {
        let text = "Hello, World! This is a test.".to_string();
        let config = create_default_config(10);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        // Each chunk should be approximately chunk_size characters
        for chunk in &chunks {
            assert!(chunk.len() <= 10);
        }
    }

    #[test]
    fn test_fixed_size_exact_fit() {
        let text = "12345678901234567890".to_string(); // 20 characters
        let config = create_default_config(10);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "1234567890");
        assert_eq!(chunks[1], "1234567890");
    }

    #[test]
    fn test_fixed_size_empty_text() {
        let text = String::new();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_fixed_size_smaller_than_chunk() {
        let text = "Short".to_string();
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short");
    }

    #[test]
    fn test_fixed_size_with_whitespace_trim() {
        let text = "   Spaces   everywhere   ".to_string();
        let mut config = create_default_config(10);
        config.options.trim_whitespace = true;

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        for chunk in &chunks {
            // Should be trimmed
            assert_eq!(chunk.trim(), *chunk);
        }
    }

    #[test]
    fn test_fixed_size_without_whitespace_trim() {
        let text = "   Spaces   ".to_string();
        let mut config = create_default_config(10);
        config.options.trim_whitespace = false;

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_fixed_size_unicode_characters() {
        let text = "Hello 世界! Testing émojis 🚀".to_string();
        let config = create_default_config(10);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should handle multi-byte characters correctly
        for chunk in chunks {
            assert!(chunk.chars().count() <= 10);
        }
    }

    #[test]
    fn test_fixed_size_newlines() {
        let text = "Line 1\nLine 2\nLine 3".to_string();
        let config = create_default_config(10);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_fixed_size_single_char() {
        let text = "A".to_string();
        let config = create_default_config(1);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "A");
    }

    #[test]
    fn test_fixed_size_large_text() {
        let text = "a".repeat(1000);
        let config = create_default_config(100);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 10);
        for chunk in &chunks {
            assert_eq!(chunk.len(), 100);
        }
    }
}
