pub mod models;

use anyhow::Result;
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, FieldCondition, Filter, Match as QdrantMatch, SearchParamsBuilder,
        SearchPointsBuilder, Value as QdrantValue, condition::ConditionOneOf,
        point_id::PointIdOptions, value::Kind,
    },
};

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use crate::search::models::{DocumentResult, SearchMatch, SearchMode, SearchRequest};
use qdrant_client::qdrant::r#match::MatchValue;

pub(crate) async fn search_collection(
    qdrant: &Qdrant,
    collection_name: &str,
    query_vector: &[f32],
    request: &SearchRequest,
) -> Result<Vec<SearchMatch>> {
    // In document mode, fetch chunks in batches until we have enough unique documents
    if matches!(request.search_mode, SearchMode::Documents) {
        const BATCH_SIZE: u64 = 200;
        let target_documents = request.limit as usize;
        let mut all_matches = Vec::new();
        let mut unique_item_ids = HashSet::new();
        let mut offset = 0;

        loop {
            // Fetch a batch
            let batch = search_batch(
                qdrant,
                collection_name,
                query_vector,
                request,
                BATCH_SIZE,
                offset,
            )
            .await?;

            if batch.is_empty() {
                break; // No more results
            }

            // Track unique documents in this batch
            for m in &batch {
                if let Some(item_id) = m.metadata.get("item_id").and_then(|v| v.as_i64()) {
                    unique_item_ids.insert(item_id);
                }
            }

            all_matches.extend(batch);

            // Check if we have enough unique documents
            if unique_item_ids.len() >= target_documents {
                break;
            }

            offset += BATCH_SIZE;

            // Safety check: don't fetch more than 50 batches (10000 chunks)
            if offset >= BATCH_SIZE * 50 {
                tracing::warn!(
                    "Reached maximum batch limit for collection '{}', stopping at {} chunks",
                    collection_name,
                    all_matches.len()
                );
                break;
            }
        }

        Ok(all_matches)
    } else {
        // Chunks mode: just fetch the requested limit
        search_batch(
            qdrant,
            collection_name,
            query_vector,
            request,
            request.limit,
            0,
        )
        .await
    }
}

async fn search_batch(
    qdrant: &Qdrant,
    collection_name: &str,
    query_vector: &[f32],
    request: &SearchRequest,
    limit: u64,
    offset: u64,
) -> Result<Vec<SearchMatch>> {
    let mut search_builder =
        SearchPointsBuilder::new(collection_name, query_vector.to_vec(), limit)
            .score_threshold(request.score_threshold)
            .with_payload(true)
            .offset(offset);

    if let Some(filters) = &request.filters
        && let Some(obj) = filters.as_object()
    {
        let mut conditions = Vec::new();

        for (key, value) in obj {
            let condition = if let Some(str_val) = value.as_str() {
                Condition {
                    condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                        key: format!("metadata.{}", key),
                        r#match: Some(QdrantMatch {
                            match_value: Some(MatchValue::Keyword(str_val.to_string())),
                        }),
                        ..Default::default()
                    })),
                }
            } else if let Some(num_val) = value.as_i64() {
                Condition {
                    condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                        key: format!("metadata.{}", key),
                        r#match: Some(QdrantMatch {
                            match_value: Some(MatchValue::Integer(num_val)),
                        }),
                        ..Default::default()
                    })),
                }
            } else {
                continue;
            };
            conditions.push(condition);
        }

        if !conditions.is_empty() {
            search_builder = search_builder.filter(Filter {
                must: conditions,
                ..Default::default()
            });
        }
    }

    if let Some(params) = &request.search_params
        && let Some(hnsw_ef) = params.hnsw_ef
    {
        search_builder =
            search_builder.params(SearchParamsBuilder::default().hnsw_ef(hnsw_ef).build());
    }

    let search_result = qdrant.search_points(search_builder).await?;

    let matches = search_result
        .result
        .into_iter()
        .map(|point| {
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

            let mut metadata_map = serde_json::Map::new();
            for (key, value) in &point.payload {
                if key != "text"
                    && let Some(json_val) = qdrant_value_to_json(value)
                {
                    metadata_map.insert(key.clone(), json_val);
                }
            }

            SearchMatch {
                id: point
                    .id
                    .map(|id| match id.point_id_options {
                        Some(PointIdOptions::Uuid(u)) => u,
                        Some(PointIdOptions::Num(n)) => n.to_string(),
                        None => format!("{:?}", id),
                    })
                    .unwrap_or_default(),
                score: point.score,
                text,
                metadata: serde_json::Value::Object(metadata_map),
            }
        })
        .collect();

    Ok(matches)
}

pub(crate) fn qdrant_value_to_json(value: &QdrantValue) -> Option<serde_json::Value> {
    if let Some(kind) = &value.kind {
        match kind {
            Kind::NullValue(_) => Some(serde_json::Value::Null),
            Kind::BoolValue(b) => Some(serde_json::json!(b)),
            Kind::IntegerValue(i) => Some(serde_json::json!(i)),
            Kind::DoubleValue(d) => Some(serde_json::json!(d)),
            Kind::StringValue(s) => Some(serde_json::json!(s)),
            Kind::ListValue(list) => {
                let values: Vec<serde_json::Value> = list
                    .values
                    .iter()
                    .filter_map(qdrant_value_to_json)
                    .collect();
                Some(serde_json::json!(values))
            }
            Kind::StructValue(strukt) => {
                let mut map = serde_json::Map::new();
                for (k, v) in &strukt.fields {
                    if let Some(json_val) = qdrant_value_to_json(v) {
                        map.insert(k.clone(), json_val);
                    }
                }
                Some(serde_json::Value::Object(map))
            }
        }
    } else {
        None
    }
}

/// Aggregate matches into unique documents based on item_id
/// Returns a list of documents sorted by best score (descending)
pub(crate) fn aggregate_matches_to_documents(matches: &[SearchMatch]) -> Vec<DocumentResult> {
    let mut document_map: HashMap<i32, (f32, Vec<&SearchMatch>)> = HashMap::new();

    for match_result in matches {
        // Extract item_id from metadata
        let item_id = match_result
            .metadata
            .get("item_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        if item_id == 0 {
            tracing::warn!(
                "Match missing item_id in metadata, skipping aggregation for match {}",
                match_result.id
            );
            continue;
        }

        let entry = document_map.entry(item_id).or_insert((0.0, Vec::new()));
        entry.1.push(match_result);

        // Track best score
        if match_result.score > entry.0 {
            entry.0 = match_result.score;
        }
    }

    let mut documents: Vec<DocumentResult> = document_map
        .into_iter()
        .map(|(item_id, (best_score, chunks))| {
            // Find the best chunk (highest score)
            let best_chunk = chunks
                .iter()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal))
                .unwrap();

            let item_title = best_chunk
                .metadata
                .get("item_title")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            DocumentResult {
                item_id,
                item_title,
                best_score,
                chunk_count: chunks.len() as i32,
                best_chunk: (*best_chunk).clone(),
            }
        })
        .collect();

    // Sort documents by score (descending)
    documents.sort_by(|a, b| {
        b.best_score
            .partial_cmp(&a.best_score)
            .unwrap_or(Ordering::Equal)
    });

    documents
}
