use crate::chat::models::{RAGConfig, RetrievedDocument};
use crate::embedding::generate_embedding;
use crate::storage::postgres::{embedded_datasets, embedders};
use qdrant_client::qdrant::SearchPointsBuilder;
use qdrant_client::qdrant::value::Kind;
use qdrant_client::{Qdrant, qdrant::point_id::PointIdOptions};
use semantic_explorer_core::encryption::EncryptionService;
use sqlx::{Pool, Postgres};
use tracing::{debug, error, instrument, warn};

#[instrument(name = "retrieve_documents", skip(postgres_pool, qdrant_client, encryption), fields(embedded_dataset_id, query_len = query.len()))]
pub async fn retrieve_documents(
    postgres_pool: &Pool<Postgres>,
    qdrant_client: &Qdrant,
    embedded_dataset_id: i32,
    query: &str,
    config: &RAGConfig,
    encryption: &EncryptionService,
) -> Result<Vec<RetrievedDocument>, String> {
    // Fetch embedded dataset info (collection name and embedder ID)
    let dataset_info =
        embedded_datasets::get_embedded_dataset_info(postgres_pool, embedded_dataset_id)
            .await
            .map_err(|e| {
                error!(error = %e, "failed to get embedded dataset info");
                format!("failed to get embedded dataset info: {e}")
            })?;

    let collection_name = dataset_info.collection_name;
    let embedder_id = dataset_info.embedder_id;

    // Fetch embedder configuration (api_key is decrypted by storage layer)
    let embedder_config = embedders::get_embedder_config(postgres_pool, embedder_id, encryption)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to get embedder config");
            format!("failed to get embedder config: {e}")
        })?;

    let embedder_provider = embedder_config.provider;
    let embedder_base_url = embedder_config.base_url;
    let embedder_api_key = embedder_config.api_key_encrypted;
    let embedder_config_value = embedder_config.config;
    let _dimensions = embedder_config.dimensions;
    debug!(collection = %collection_name, embedder_id = embedder_id, "retrieving documents from Qdrant");

    let _collection_info = qdrant_client
        .collection_info(&collection_name)
        .await
        .map_err(|e| {
            error!(error = %e, collection = %collection_name, "failed to get collection info");
            format!("failed to access collection: {e}")
        })?;

    debug!(collection = %collection_name, "collection found");

    // Generate embedding using the actual embedder service
    let query_embedding = match generate_embedding(
        &embedder_provider,
        &embedder_base_url,
        embedder_api_key.as_deref(),
        &embedder_config_value,
        query,
    )
    .await
    {
        Ok(embedding) => embedding,
        Err(e) => {
            error!(error = %e, "failed to generate embedding");
            // Fallback to placeholder embedding
            warn!("falling back to placeholder embedding for query");
            generate_simple_embedding(query, _dimensions as usize)
        }
    };

    let search_builder = SearchPointsBuilder::new(
        &collection_name,
        query_embedding,
        config.max_context_documents as u64,
    )
    .score_threshold(config.min_similarity_score)
    .with_payload(true);

    let search_response = qdrant_client
        .search_points(search_builder)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to search Qdrant");
            format!("failed to search Qdrant: {e}")
        })?;

    debug!(
        result_count = search_response.result.len(),
        "retrieved documents from Qdrant"
    );

    let documents: Vec<RetrievedDocument> = search_response
        .result
        .into_iter()
        .filter_map(|point| {
            let document_id = point.id.as_ref().map(|id| match &id.point_id_options {
                Some(PointIdOptions::Uuid(u)) => u.clone(),
                Some(PointIdOptions::Num(n)) => n.to_string(),
                None => format!("{:?}", id),
            });

            let text = point
                .payload
                .get("text")
                .and_then(|v| {
                    if let Some(Kind::StringValue(s)) = &v.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let item_title = point.payload.get("item_title").and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            });

            if !text.is_empty() {
                Some(RetrievedDocument {
                    document_id,
                    text,
                    similarity_score: point.score,
                    item_title,
                })
            } else {
                None
            }
        })
        .collect();

    debug!(
        final_count = documents.len(),
        "final document count after filtering"
    );
    Ok(documents)
}

/// Generate a simple embedding from text
/// This is a placeholder implementation that creates a vector based on character/word frequencies
/// In production, this would call an actual embedding service
fn generate_simple_embedding(text: &str, dimension: usize) -> Vec<f32> {
    let mut embedding = vec![0.0; dimension];

    // Simple hash-based approach: distribute text characters across the vector
    let normalized_text = text.to_lowercase();
    for ch in normalized_text.chars() {
        if !embedding.is_empty() {
            let position = (ch as usize).wrapping_mul(31) % embedding.len();
            embedding[position] += 1.0;
        }
    }

    // Normalize the vector
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut embedding {
            *val /= norm;
        }
    }

    embedding
}

pub fn build_context(documents: &[RetrievedDocument]) -> String {
    if documents.is_empty() {
        return "No relevant documents found.".to_string();
    }

    // Pre-allocate with reasonable capacity to avoid reallocations
    let mut context = String::with_capacity(documents.len() * 200 + 50);
    context.push_str("Below are the relevant document chunks retrieved for this query. Reference them by their chunk number (e.g., \"According to Chunk 1\" or \"As stated in Chunk 2\") when answering.\n\n");

    for (idx, doc) in documents.iter().enumerate() {
        let chunk_num = idx + 1;
        let item_title = doc.item_title.as_deref().unwrap_or("unknown");
        // Format document chunk with injection protection delimiters
        let formatted_chunk = crate::chat::prompt_injection::format_document_chunk(
            chunk_num,
            item_title,
            doc.similarity_score,
            &doc.text,
        );
        context.push_str(&formatted_chunk);
        context.push_str("\n\n");
    }

    context
}

/// Replace "Chunk N" references in LLM response with actual document titles
/// This transforms the response from using generic chunk numbers to user-facing document titles
pub fn replace_chunk_references(content: &str, documents: &[RetrievedDocument]) -> String {
    use regex::Regex;

    // Create a mapping from chunk number to item title
    let chunk_to_title: std::collections::HashMap<usize, String> = documents
        .iter()
        .enumerate()
        .map(|(idx, doc)| {
            let chunk_num = idx + 1;
            let title = doc.item_title.as_deref().unwrap_or("Unknown Source");
            (chunk_num, title.to_string())
        })
        .collect();

    // Replace "Chunk N" with actual titles using regex
    // Matches patterns like "Chunk 1", "Chunk 2", etc.
    let re = Regex::new(r"Chunk (\d+)").unwrap();
    let result = re.replace_all(content, |caps: &regex::Captures| {
        let chunk_num_str = &caps[1];
        if let Ok(chunk_num) = chunk_num_str.parse::<usize>() {
            chunk_to_title
                .get(&chunk_num)
                .cloned()
                .unwrap_or_else(|| format!("Chunk {}", chunk_num))
        } else {
            caps[0].to_string()
        }
    });

    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context_empty() {
        let docs = vec![];
        let context = build_context(&docs);
        assert_eq!(context, "No relevant documents found.");
    }

    #[test]
    fn test_build_context_single() {
        let docs = vec![RetrievedDocument {
            document_id: Some("doc1".to_string()),
            text: "This is a test document".to_string(),
            similarity_score: 0.95,
            item_title: Some("test_item".to_string()),
        }];

        let context = build_context(&docs);
        assert!(context.contains("Chunk 1"));
        assert!(context.contains("Score: 0.95"));
        assert!(context.contains("test_item"));
        assert!(context.contains("This is a test document"));
    }

    #[test]
    fn test_replace_chunk_references() {
        let docs = vec![
            RetrievedDocument {
                document_id: Some("doc1".to_string()),
                text: "Content 1".to_string(),
                similarity_score: 0.95,
                item_title: Some("Document A".to_string()),
            },
            RetrievedDocument {
                document_id: Some("doc2".to_string()),
                text: "Content 2".to_string(),
                similarity_score: 0.85,
                item_title: Some("Document B".to_string()),
            },
        ];

        let content = "According to Chunk 1, this is true. Also, Chunk 2 confirms it.";
        let result = replace_chunk_references(content, &docs);

        assert_eq!(
            result,
            "According to Document A, this is true. Also, Document B confirms it."
        );
    }

    #[test]
    fn test_replace_chunk_references_no_title() {
        let docs = vec![RetrievedDocument {
            document_id: Some("doc1".to_string()),
            text: "Content".to_string(),
            similarity_score: 0.95,
            item_title: None,
        }];

        let content = "According to Chunk 1, this is true.";
        let result = replace_chunk_references(content, &docs);

        assert_eq!(result, "According to Unknown Source, this is true.");
    }

    #[test]
    fn test_rag_config_default() {
        let config = RAGConfig::default();
        assert_eq!(config.max_context_documents, 20);
        assert_eq!(config.min_similarity_score, 0.2);
        assert_eq!(config.max_tokens_context, 5000);
    }
}
