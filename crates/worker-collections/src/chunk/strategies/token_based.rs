use anyhow::Result;
use tiktoken_rs::{CoreBPE, cl100k_base};

use crate::chunk::config::ChunkingConfig;

/// Token-based chunking strategy that respects model token limits
/// Uses tiktoken tokenizer (cl100k_base, used by GPT-4 and text-embedding-ada-002)
pub fn chunk(text: String, config: &ChunkingConfig) -> Result<Vec<String>> {
    let bpe = cl100k_base()?;
    let options = config.options.token_based.as_ref();

    // Get token limit - use chunk_size as default
    let max_tokens = options.map(|o| o.max_tokens).unwrap_or(config.chunk_size);

    // Get overlap in tokens
    let overlap_tokens = options
        .map(|o| o.overlap_tokens)
        .unwrap_or(config.chunk_overlap);

    // Get separator preference
    let split_on_sentences = options.map(|o| o.split_on_sentences).unwrap_or(true);

    if text.is_empty() {
        return Ok(Vec::new());
    }

    if split_on_sentences {
        chunk_by_sentences(&text, &bpe, max_tokens, overlap_tokens)
    } else {
        chunk_by_tokens(&text, &bpe, max_tokens, overlap_tokens)
    }
}

/// Chunk text by grouping sentences while respecting token limits
fn chunk_by_sentences(
    text: &str,
    bpe: &CoreBPE,
    max_tokens: usize,
    overlap_tokens: usize,
) -> Result<Vec<String>> {
    use unicode_segmentation::UnicodeSegmentation;

    let sentences: Vec<&str> = text
        .unicode_sentences()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if sentences.is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut current_tokens = 0;

    for sentence in sentences {
        let sentence_tokens = count_tokens(bpe, sentence);

        // If single sentence exceeds max, split it by tokens
        if sentence_tokens > max_tokens {
            // First, flush current chunk if not empty
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
                current_tokens = 0;
            }

            // Split the long sentence into token-sized pieces
            let sub_chunks = split_by_tokens(sentence, bpe, max_tokens, overlap_tokens)?;
            chunks.extend(sub_chunks);
            continue;
        }

        // Check if adding this sentence would exceed limit
        if current_tokens + sentence_tokens + 1 > max_tokens && !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());

            // Apply overlap - take tokens from end of current chunk
            if overlap_tokens > 0 {
                current_chunk = get_overlap_text(&current_chunk, bpe, overlap_tokens);
                current_tokens = count_tokens(bpe, &current_chunk);
            } else {
                current_chunk = String::new();
                current_tokens = 0;
            }
        }

        // Add sentence to current chunk
        if !current_chunk.is_empty() {
            current_chunk.push(' ');
            current_tokens += 1; // Space token
        }
        current_chunk.push_str(sentence);
        current_tokens += sentence_tokens;
    }

    // Don't forget the last chunk
    if !current_chunk.is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    Ok(chunks)
}

/// Chunk text purely by token count (no sentence awareness)
fn chunk_by_tokens(
    text: &str,
    bpe: &CoreBPE,
    max_tokens: usize,
    overlap_tokens: usize,
) -> Result<Vec<String>> {
    let tokens = bpe.encode_with_special_tokens(text);

    if tokens.len() <= max_tokens {
        return Ok(vec![text.to_string()]);
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < tokens.len() {
        let end = (start + max_tokens).min(tokens.len());
        let chunk_tokens = &tokens[start..end];

        // Decode tokens back to text
        let chunk_text = bpe.decode(chunk_tokens.to_vec())?;
        chunks.push(chunk_text.trim().to_string());

        // Move start, accounting for overlap
        if overlap_tokens > 0 && end < tokens.len() {
            start = end.saturating_sub(overlap_tokens);
        } else {
            start = end;
        }
    }

    Ok(chunks)
}

/// Split a single piece of text by token count
fn split_by_tokens(
    text: &str,
    bpe: &CoreBPE,
    max_tokens: usize,
    overlap_tokens: usize,
) -> Result<Vec<String>> {
    chunk_by_tokens(text, bpe, max_tokens, overlap_tokens)
}

/// Get overlap text from the end of a chunk
fn get_overlap_text(text: &str, bpe: &CoreBPE, overlap_tokens: usize) -> String {
    let tokens = bpe.encode_with_special_tokens(text);
    if tokens.len() <= overlap_tokens {
        return text.to_string();
    }

    let overlap_start = tokens.len() - overlap_tokens;
    let overlap_tokens = &tokens[overlap_start..];

    bpe.decode(overlap_tokens.to_vec())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Count tokens in a piece of text
fn count_tokens(bpe: &CoreBPE, text: &str) -> usize {
    bpe.encode_with_special_tokens(text).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::config::{
        ChunkingConfig, ChunkingOptions, ChunkingStrategy, TokenBasedOptions,
    };

    fn create_token_config(
        max_tokens: usize,
        overlap: usize,
        split_sentences: bool,
    ) -> ChunkingConfig {
        ChunkingConfig {
            strategy: ChunkingStrategy::TokenBased,
            chunk_size: max_tokens,
            chunk_overlap: overlap,
            options: ChunkingOptions {
                token_based: Some(TokenBasedOptions {
                    max_tokens,
                    overlap_tokens: overlap,
                    split_on_sentences: split_sentences,
                    model: None,
                }),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_token_chunk_empty_text() {
        let config = create_token_config(100, 0, true);
        let result = chunk(String::new(), &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_token_chunk_short_text() {
        let config = create_token_config(100, 0, true);
        let text = "Hello, world! This is a short test.".to_string();
        let result = chunk(text.clone(), &config);
        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_token_chunk_respects_limit() {
        let config = create_token_config(20, 0, true);
        let text = "First sentence here. Second sentence follows. Third one comes next. Fourth is here too. Fifth sentence ends it.".to_string();
        let result = chunk(text, &config);
        assert!(result.is_ok());
        let chunks = result.unwrap();

        // Verify each chunk is within token limit
        let bpe = cl100k_base().unwrap();
        for chunk in &chunks {
            let token_count = bpe.encode_with_special_tokens(chunk).len();
            assert!(
                token_count <= 20,
                "Chunk has {} tokens, expected <= 20",
                token_count
            );
        }
    }

    #[test]
    fn test_token_chunk_with_overlap() {
        let config = create_token_config(20, 5, true);
        let text = "First sentence here. Second sentence follows. Third one comes next. Fourth is here too.".to_string();
        let result = chunk(text, &config);
        assert!(result.is_ok());
        let chunks = result.unwrap();

        // With overlap, later chunks should have some content from previous chunks
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_token_chunk_long_sentence() {
        let config = create_token_config(10, 0, true);
        // This sentence will exceed 10 tokens
        let text = "This is a very long sentence that contains many words and should definitely exceed the ten token limit we have set.".to_string();
        let result = chunk(text, &config);
        assert!(result.is_ok());
        let chunks = result.unwrap();

        // Should be split into multiple chunks
        assert!(chunks.len() > 1);

        // Each chunk should respect the limit (approximately)
        let bpe = cl100k_base().unwrap();
        for chunk in &chunks {
            let token_count = bpe.encode_with_special_tokens(chunk).len();
            assert!(
                token_count <= 12,
                "Chunk has {} tokens, expected <= ~10",
                token_count
            );
        }
    }

    #[test]
    fn test_pure_token_chunking() {
        let config = create_token_config(10, 0, false); // Disable sentence splitting
        let text =
            "This is text without sentence boundaries just flowing continuously through the chunk"
                .to_string();
        let result = chunk(text, &config);
        assert!(result.is_ok());
    }
}
