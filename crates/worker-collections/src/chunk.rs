use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn chunk_text(text: String, chunk_size: usize) -> Result<Vec<String>> {
    let sentences: Vec<&str> = text
        .unicode_sentences()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for sentence in sentences {
        if current_chunk.len() + sentence.len() + 1 > chunk_size && !current_chunk.is_empty() {
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
