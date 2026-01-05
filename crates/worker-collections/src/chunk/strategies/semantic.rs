use anyhow::{Result, anyhow};
use semantic_explorer_core::jobs::EmbedderConfig;
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

    let embedder =
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

    let embeddings = generate_batch_embeddings(embedder, &sentences).await?;

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

async fn generate_batch_embeddings(
    config: &EmbedderConfig,
    sentences: &[&str],
) -> Result<Vec<Vec<f32>>> {
    const BATCH_SIZE: usize = 100; // TODO: use embedder config for batch size. every embedder has different limits, 96 for cohere for example

    if sentences.is_empty() {
        return Ok(Vec::new());
    }

    let mut all_embeddings = Vec::new();

    for batch in sentences.chunks(BATCH_SIZE) {
        let embeddings = call_embedder_api(config, batch).await?;
        all_embeddings.extend(embeddings);
    }

    Ok(all_embeddings)
}

async fn call_embedder_api(config: &EmbedderConfig, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let (url, body) = match config.provider.as_str() {
        "openai" => {
            let model = config.model.as_deref().unwrap_or("text-embedding-ada-002");
            let body = serde_json::json!({
                "input": texts,
                "model": model,
            });
            let url = format!("{}/embeddings", config.base_url.trim_end_matches('/'));
            (url, body)
        }
        "cohere" => {
            let model = config.model.as_deref().unwrap_or("embed-english-v3.0");
            let input_type = config
                .config
                .get("input_type")
                .and_then(|v| v.as_str())
                .unwrap_or("clustering");

            let body = serde_json::json!({
                "texts": texts,
                "model": model,
                "input_type": input_type,
                "embedding_types": ["float"],
                "truncate": "NONE"
            });
            let base = config.base_url.trim_end_matches('/');
            let url = if base.ends_with("/embed") {
                base.to_string()
            } else {
                format!("{}/embed", base)
            };
            (url, body)
        }
        _ => {
            return Err(anyhow!(
                "Unsupported embedder provider: {}",
                config.provider
            ));
        }
    };

    let mut req = client.post(&url).json(&body);

    if let Some(key) = &config.api_key {
        req = req.bearer_auth(key);
    } else {
        return Err(anyhow!("API key required for {} provider", config.provider));
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Embedder API error ({}): {}", status, error_text));
    }

    let resp_json: serde_json::Value = resp.json().await?;

    let embeddings = match config.provider.as_str() {
        "openai" => {
            let data = resp_json["data"]
                .as_array()
                .ok_or_else(|| anyhow!("Invalid OpenAI response format"))?;

            data.iter()
                .map(|item| {
                    item["embedding"]
                        .as_array()
                        .ok_or_else(|| anyhow!("Missing embedding in response"))
                        .and_then(|arr| {
                            arr.iter()
                                .map(|v| {
                                    v.as_f64()
                                        .map(|f| f as f32)
                                        .ok_or_else(|| anyhow!("Invalid embedding value"))
                                })
                                .collect()
                        })
                })
                .collect::<Result<Vec<Vec<f32>>>>()?
        }
        "cohere" => {
            let embeddings_obj = resp_json["embeddings"]
                .as_object()
                .ok_or_else(|| anyhow!("Invalid Cohere response format"))?;

            let float_embeddings = embeddings_obj["float"]
                .as_array()
                .ok_or_else(|| anyhow!("Missing float embeddings in Cohere response"))?;

            float_embeddings
                .iter()
                .map(|arr| {
                    arr.as_array()
                        .ok_or_else(|| anyhow!("Invalid embedding format"))
                        .and_then(|a| {
                            a.iter()
                                .map(|v| {
                                    v.as_f64()
                                        .map(|f| f as f32)
                                        .ok_or_else(|| anyhow!("Invalid embedding value"))
                                })
                                .collect()
                        })
                })
                .collect::<Result<Vec<Vec<f32>>>>()?
        }
        _ => return Err(anyhow!("Unsupported provider")),
    };

    Ok(embeddings)
}
