use anyhow::Result;

pub fn apply_overlap(chunks: Vec<String>, overlap_size: usize) -> Result<Vec<String>> {
    if chunks.is_empty() || overlap_size == 0 {
        return Ok(chunks);
    }

    let mut overlapped_chunks = Vec::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        if idx == 0 {
            overlapped_chunks.push(chunk.clone());
            continue;
        }

        let previous_chunk = &chunks[idx - 1];

        let overlap_prefix = get_last_chars(previous_chunk, overlap_size);

        let overlapped = if overlap_prefix.is_empty() {
            chunk.clone()
        } else {
            format!("{}{}", overlap_prefix, chunk)
        };
        overlapped_chunks.push(overlapped);
    }

    Ok(overlapped_chunks)
}

fn get_last_chars(text: &str, n: usize) -> String {
    text.chars()
        .rev()
        .take(n)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_overlap_basic() {
        let chunks = vec![
            "Chunk 1 content".to_string(),
            "Chunk 2 content".to_string(),
            "Chunk 3 content".to_string(),
        ];
        let overlap_size = 5;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 3);

        // First chunk should be unchanged
        assert_eq!(overlapped[0], "Chunk 1 content");

        // Second chunk should have overlap from first
        assert!(overlapped[1].starts_with("ntent"));
        assert!(overlapped[1].contains("Chunk 2"));

        // Third chunk should have overlap from second
        assert!(overlapped[2].starts_with("ntent"));
        assert!(overlapped[2].contains("Chunk 3"));
    }

    #[test]
    fn test_apply_overlap_zero() {
        let chunks = vec!["Chunk 1".to_string(), "Chunk 2".to_string()];
        let overlap_size = 0;

        let result = apply_overlap(chunks.clone(), overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped, chunks);
    }

    #[test]
    fn test_apply_overlap_empty() {
        let chunks: Vec<String> = vec![];
        let overlap_size = 5;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_apply_overlap_single_chunk() {
        let chunks = vec!["Only one chunk".to_string()];
        let overlap_size = 5;

        let result = apply_overlap(chunks.clone(), overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 1);
        assert_eq!(overlapped[0], "Only one chunk");
    }

    #[test]
    fn test_apply_overlap_larger_than_chunk() {
        let chunks = vec!["Short".to_string(), "Another".to_string()];
        let overlap_size = 100;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 2);
        // Should take all of previous chunk
        assert_eq!(overlapped[1], "ShortAnother");
    }

    #[test]
    fn test_apply_overlap_exact_chunk_size() {
        let chunks = vec!["12345".to_string(), "67890".to_string()];
        let overlap_size = 5;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped[0], "12345");
        assert_eq!(overlapped[1], "1234567890");
    }

    #[test]
    fn test_apply_overlap_unicode() {
        let chunks = vec!["Hello 世界".to_string(), "Testing 测试".to_string()];
        let overlap_size = 3;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 2);
        // Should handle unicode characters correctly
        // The last 3 characters of "Hello 世界" are "o 世界" but we want just 3 chars
        let first_chunk = &overlapped[0];
        let overlap_text = get_last_chars_helper(first_chunk, 3);
        assert!(overlapped[1].starts_with(&overlap_text));
    }

    // Helper function to test overlap logic
    fn get_last_chars_helper(text: &str, n: usize) -> String {
        let chars: Vec<char> = text.chars().collect();
        let start = if chars.len() > n { chars.len() - n } else { 0 };
        chars[start..].iter().collect()
    }

    #[test]
    fn test_apply_overlap_whitespace() {
        let chunks = vec!["Chunk   1   ".to_string(), "Chunk   2   ".to_string()];
        let overlap_size = 4;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 2);
        // Whitespace should be preserved
        assert!(overlapped[1].starts_with("1   "));
    }

    #[test]
    fn test_apply_overlap_newlines() {
        let chunks = vec!["Line 1\nLine 2".to_string(), "Line 3\nLine 4".to_string()];
        let overlap_size = 6;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 2);
        // Newlines should be preserved
        assert!(overlapped[1].contains("\n"));
    }

    #[test]
    fn test_apply_overlap_multiple_chunks() {
        let chunks = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
            "E".to_string(),
        ];
        let overlap_size = 1;

        let result = apply_overlap(chunks, overlap_size);
        assert!(result.is_ok());

        let overlapped = result.unwrap();
        assert_eq!(overlapped.len(), 5);
        assert_eq!(overlapped[0], "A");
        assert_eq!(overlapped[1], "AB");
        assert_eq!(overlapped[2], "BC");
        assert_eq!(overlapped[3], "CD");
        assert_eq!(overlapped[4], "DE");
    }

    #[test]
    fn test_apply_overlap_unicode_iterator_correctness() {
        // Test specifically for the iterator implementation of get_last_chars
        // "😊" is 4 bytes.
        let text = "Start 😊 End";
        let n = 5;

        let result = get_last_chars(text, n);

        // n is number of CHARS, not bytes.
        // "End" is 3 chars. " " is 1. "😊" is 1.
        // So last 5 chars are: "😊 End"

        assert_eq!(result, "😊 End");

        // Ensure no panic on split surrogates or large request
        let empty_result = get_last_chars("", 10);
        assert_eq!(empty_result, "");

        let overflow_result = get_last_chars("abc", 100);
        assert_eq!(overflow_result, "abc");
    }
}
