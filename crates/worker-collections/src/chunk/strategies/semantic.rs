use anyhow::{Result, anyhow};
use semantic_explorer_core::embedder::generate_batch_embeddings;
use semantic_explorer_core::models::EmbedderConfig;
use unicode_segmentation::UnicodeSegmentation;

use crate::chunk::config::ChunkingConfig;

pub async fn chunk_async(
    text: String,
    config: &ChunkingConfig,
    embedder_config: Option<&EmbedderConfig>,
) -> Result<Vec<String>> {
    let semantic_opts = config
        .options
        .semantic
        .as_ref()
        .ok_or_else(|| anyhow!("Semantic options not configured"))?;

    let embedder_config =
        embedder_config.ok_or_else(|| anyhow!("Embedder config required for semantic chunking"))?;

    let sentences: Vec<&str> = text
        .unicode_sentences()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if sentences.is_empty() {
        return Ok(vec![]);
    }

    if sentences.len() == 1 {
        return Ok(vec![sentences[0].to_string()]);
    }

    let embeddings = generate_batch_embeddings(
        embedder_config,
        sentences.clone(),
        Some(embedder_config.batch_size as usize),
    )
    .await?;

    merge_by_similarity(sentences, embeddings, semantic_opts, config.chunk_size)
}

fn merge_by_similarity(
    sentences: Vec<&str>,
    embeddings: Vec<Vec<f32>>,
    semantic_opts: &crate::chunk::config::SemanticOptions,
    _target_chunk_size: usize,
) -> Result<Vec<String>> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut current_embedding = embeddings[0].clone();
    let mut current_sentence_count = 1;

    for (idx, (sentence, embedding)) in sentences.iter().zip(embeddings.iter()).enumerate() {
        if idx == 0 {
            current_chunk = sentence.to_string();
            continue;
        }

        let similarity = cosine_similarity(&current_embedding, embedding);
        let potential_size = current_chunk.len() + sentence.len() + 1;
        let should_merge = similarity >= semantic_opts.similarity_threshold
            && potential_size <= semantic_opts.max_chunk_size;

        if should_merge {
            current_chunk.push(' ');
            current_chunk.push_str(sentence);
            current_sentence_count += 1;
            current_embedding =
                weighted_average_embedding(&current_embedding, embedding, current_sentence_count);
        } else {
            if current_chunk.len() >= semantic_opts.min_chunk_size {
                chunks.push(current_chunk.clone());
            } else if !chunks.is_empty() {
                let last = chunks.last_mut().unwrap();
                last.push(' ');
                last.push_str(&current_chunk);
            } else {
                chunks.push(current_chunk.clone());
            }

            current_chunk = sentence.to_string();
            current_embedding = embedding.clone();
            current_sentence_count = 1;
        }
    }

    if !current_chunk.is_empty() {
        if current_chunk.len() >= semantic_opts.min_chunk_size || chunks.is_empty() {
            chunks.push(current_chunk);
        } else {
            let last = chunks.last_mut().unwrap();
            last.push(' ');
            last.push_str(&current_chunk);
        }
    }

    Ok(chunks)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        0.0
    } else {
        dot / (mag_a * mag_b)
    }
}

fn weighted_average_embedding(current: &[f32], new: &[f32], count: usize) -> Vec<f32> {
    let weight_current = (count - 1) as f32 / count as f32;
    let weight_new = 1.0 / count as f32;

    current
        .iter()
        .zip(new.iter())
        .map(|(c, n)| c * weight_current + n * weight_new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::config::SemanticOptions;

    #[test]
    fn test_cosine_similarity_identical() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_merge_by_similarity() {
        let sentences = vec!["Sentence 1", "Sentence 2", "Sentence 3"];
        let embeddings = vec![vec![1.0, 0.0], vec![0.99, 0.01], vec![0.0, 1.0]];

        let opts = SemanticOptions {
            similarity_threshold: 0.9,
            max_chunk_size: 100,
            ..Default::default()
        };

        let result = merge_by_similarity(sentences, embeddings, &opts, 100);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("Sentence 1") && chunks[0].contains("Sentence 2"));
        assert_eq!(chunks[1], "Sentence 3");
    }

    #[test]
    fn test_merge_by_similarity_respects_max_size() {
        let sentences = vec!["Short", "A very long sentence that causes split"];
        let embeddings = vec![vec![1.0, 0.0], vec![0.99, 0.01]];

        let opts = SemanticOptions {
            similarity_threshold: 0.0,
            max_chunk_size: 10,
            ..Default::default()
        };

        let result = merge_by_similarity(sentences, embeddings, &opts, 10);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
    }
}
