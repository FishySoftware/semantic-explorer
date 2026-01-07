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

use std::{cmp::Ordering, collections::HashMap};

use crate::search::models::{EmbedderSearchResults, SearchMatch, SearchRequest, SourceAggregation};
use qdrant_client::qdrant::r#match::MatchValue;

pub(crate) async fn search_collection(
    qdrant: &Qdrant,
    collection_name: &str,
    query_vector: &[f32],
    request: &SearchRequest,
) -> Result<Vec<SearchMatch>> {
    let mut search_builder =
        SearchPointsBuilder::new(collection_name, query_vector.to_vec(), request.limit)
            .score_threshold(request.score_threshold)
            .with_payload(true);

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

pub(crate) fn aggregate_by_source(results: &[EmbedderSearchResults]) -> Vec<SourceAggregation> {
    let mut source_map: HashMap<String, SourceAggregation> = HashMap::new();

    for embedder_result in results {
        for match_result in &embedder_result.matches {
            // Extract source from metadata
            let source = match_result
                .metadata
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let entry = source_map
                .entry(source.clone())
                .or_insert(SourceAggregation {
                    source: source.clone(),
                    matches: Vec::new(),
                    best_score: 0.0,
                    embedder_ids: Vec::new(),
                });

            // Track embedder IDs
            if !entry.embedder_ids.contains(&embedder_result.embedder_id) {
                entry.embedder_ids.push(embedder_result.embedder_id);
            }

            // Update best score
            if match_result.score > entry.best_score {
                entry.best_score = match_result.score;
            }

            // Add match to this source
            entry.matches.push(match_result.clone());
        }
    }

    // Convert HashMap to Vec and sort by best_score descending
    let mut aggregated: Vec<SourceAggregation> = source_map.into_values().collect();
    aggregated.sort_by(|a, b| {
        b.best_score
            .partial_cmp(&a.best_score)
            .unwrap_or(Ordering::Equal)
    });

    aggregated
}
