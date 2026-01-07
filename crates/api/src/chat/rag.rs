use crate::chat::models::{RAGConfig, RetrievedDocument};
use qdrant_client::qdrant::SearchPointsBuilder;
use qdrant_client::qdrant::value::Kind;
use qdrant_client::{Qdrant, qdrant::point_id::PointIdOptions};
use sqlx::{Pool, Postgres};
use tracing::{debug, error, instrument};

#[instrument(name = "retrieve_documents", skip(postgres_pool, qdrant_client), fields(embedded_dataset_id, query_len = query.len()))]
pub async fn retrieve_documents(
    postgres_pool: &Pool<Postgres>,
    qdrant_client: &Qdrant,
    embedded_dataset_id: i32,
    query: &str,
    config: &RAGConfig,
) -> Result<Vec<RetrievedDocument>, String> {
    let row = sqlx::query_as::<_, (String, i32)>(
        "SELECT collection_name, embedder_id FROM embedded_datasets WHERE embedded_dataset_id = $1",
    )
    .bind(embedded_dataset_id)
    .fetch_optional(postgres_pool)
    .await
    .map_err(|e| {
        error!(error = %e, "failed to get collection info");
        format!("failed to get collection info: {e}")
    })?
    .ok_or_else(|| "embedded dataset not found".to_string())?;

    let (collection_name, embedder_id) = row;
    debug!(collection = %collection_name, embedder_id = embedder_id, "retrieving documents from Qdrant");

    let _collection_info = qdrant_client
        .collection_info(&collection_name)
        .await
        .map_err(|e| {
            error!(error = ?e, collection = %collection_name, "failed to get collection info");
            format!("failed to access collection: {e:?}")
        })?;

    debug!(collection = %collection_name, "collection found");

    let embedder = sqlx::query_as::<_, (String, String, String, i32)>(
        r#"SELECT provider, base_url, api_key, embedding_dimension FROM embedders WHERE embedder_id = $1"#
    )
    .bind(embedder_id)
    .fetch_optional(postgres_pool)
    .await
    .map_err(|e| {
        error!(error = %e, "failed to get embedder");
        format!("failed to get embedder: {e}")
    })?
    .ok_or_else(|| "embedder not found".to_string())?;

    let (_embedder_provider, _embedder_base_url, _embedder_api_key, embedding_dimension) = embedder;

    let query_embedding = generate_simple_embedding(query, embedding_dimension as usize);

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
            error!(error = ?e, "failed to search Qdrant");
            format!("failed to search Qdrant: {e:?}")
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

            let source = point
                .payload
                .get("source")
                .and_then(|v| {
                    if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .or_else(|| Some(collection_name.clone()));

            if !text.is_empty() {
                Some(RetrievedDocument {
                    document_id,
                    text,
                    similarity_score: point.score,
                    source,
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
    context.push_str("Retrieved context:\n\n");

    for (idx, doc) in documents.iter().enumerate() {
        let source = doc.source.as_deref().unwrap_or("unknown");
        // Use push_str with pre-formatted strings instead of format! to reduce allocations
        context.push('[');
        context.push_str(&(idx + 1).to_string());
        context.push_str("] (Score: ");
        context.push_str(&format!("{:.2}", doc.similarity_score));
        context.push_str(", Source: ");
        context.push_str(source);
        context.push_str(")\n");
        context.push_str(&doc.text);
        context.push_str("\n\n");
    }

    context
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
            source: Some("test_source".to_string()),
        }];

        let context = build_context(&docs);
        assert!(context.contains("Retrieved context:"));
        assert!(context.contains("Score: 0.95"));
        assert!(context.contains("test_source"));
        assert!(context.contains("This is a test document"));
    }

    #[test]
    fn test_rag_config_default() {
        let config = RAGConfig::default();
        assert_eq!(config.max_context_documents, 5);
        assert_eq!(config.min_similarity_score, 0.5);
        assert_eq!(config.max_tokens_context, 3000);
    }
}
