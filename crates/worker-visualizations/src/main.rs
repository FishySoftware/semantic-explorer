/// Topic Modeling with cuML and Qdrant
///
/// This example replicates the Python workflow for topic modeling:
/// 1. Fetch document vectors from Qdrant
/// 2. Reduce dimensionality using UMAP with cosine KNN
/// 3. Identify topic clusters using HDBSCAN
/// 4. Export topic vectors back to Qdrant
///
/// Environment Variables:
/// - QDRANT_URL: Qdrant server URL (default: http://localhost:6334)
/// - QDRANT_API_KEY: Optional API key for Qdrant Cloud
/// - DOCUMENT_COLLECTION: Source collection name (default: documents)
/// - TOPIC_COLLECTION: Target collection name (default: topics)
/// - TOPIC_COLLECTION_RESET: Reset topic collection (default: false)
/// - BATCH_SIZE: Batch size for fetching vectors (default: 1000)
/// - N_NEIGHBORS: UMAP n_neighbors parameter (default: 15)
/// - N_COMPONENTS: UMAP n_components parameter (default: 5)
/// - MIN_CLUSTER_SIZE: HDBSCAN min_cluster_size (default: 15)
/// - LIMIT: Optional limit on number of documents to process
mod topic_naming;

use cuml_wrapper_rs::{identify_topic_clusters, reduce_dimensionality};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::vectors_output::VectorsOptions;
use qdrant_client::qdrant::{
    CreateCollection, Distance, PointStruct, ScrollPoints, UpsertPointsBuilder, Value,
    VectorParams, VectorsConfig, vectors_config::Config,
};
use std::collections::HashMap;
use std::env;
use std::time::Instant;
use topic_naming::TfidfTopicNamer;

#[derive(Clone)]
struct DocumentData {
    text: String,
}

async fn fetch_documents_with_text(
    client: &Qdrant,
    collection_name: &str,
    batch_size: u64,
    limit: Option<usize>,
) -> Result<(Vec<f32>, Vec<DocumentData>), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut all_vectors = Vec::new();
    let mut all_documents = Vec::new();
    let mut offset: Option<qdrant_client::qdrant::PointId> = None;

    loop {
        let scroll_result = client
            .scroll(ScrollPoints {
                collection_name: collection_name.to_string(),
                limit: Some(batch_size as u32),
                offset,
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            })
            .await?;

        let points = scroll_result.result;

        if points.is_empty() {
            break;
        }

        for point in &points {
            if let Some(vectors) = &point.vectors
                && let Some(vector_data) = vectors.vectors_options.as_ref()
                && let VectorsOptions::Vector(vec) = vector_data
            {
                #[allow(deprecated)]
                all_vectors.extend_from_slice(&vec.data);

                // Extract text from payload
                let text = point
                    .payload
                    .get("text")
                    .and_then(|v| {
                        if let Some(kind) = &v.kind {
                            match kind {
                                qdrant_client::qdrant::value::Kind::StringValue(s) => {
                                    Some(s.clone())
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                all_documents.push(DocumentData { text });
            }
        }

        if let Some(limit_val) = limit {
            let n_vectors = all_vectors.len() / get_vector_size(client, collection_name).await?;
            if n_vectors >= limit_val {
                break;
            }
        }

        if let Some(last_point) = points.last() {
            offset = last_point.id.clone();
        } else {
            break;
        }

        if (points.len() as u64) < batch_size {
            break;
        }
    }

    let vector_size = get_vector_size(client, collection_name).await?;
    let n_vectors = all_vectors.len() / vector_size;

    println!(
        "Fetched {} document vectors with text in {:.2} seconds",
        n_vectors,
        start.elapsed().as_secs_f64()
    );

    Ok((all_vectors, all_documents))
}

async fn get_vector_size(
    client: &Qdrant,
    collection_name: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let collection_info = client.collection_info(collection_name).await?;
    if let Some(config) = collection_info.result.and_then(|r| r.config)
        && let Some(vectors_config) = config.params.and_then(|p| p.vectors_config)
        && let Some(Config::Params(params)) = vectors_config.config
    {
        return Ok(params.size as usize);
    }
    Err("Could not determine vector size".into())
}

async fn initialize_topic_collection(
    client: &Qdrant,
    collection_name: &str,
    vector_size: usize,
    reset: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let exists = client.collection_exists(collection_name).await?;

    if exists && reset {
        println!("Deleting existing collection: {}", collection_name);
        client.delete_collection(collection_name).await?;
    }

    if !exists || reset {
        println!("Creating collection: {}", collection_name);
        client
            .create_collection(CreateCollection {
                collection_name: collection_name.to_string(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: vector_size as u64,
                        distance: Distance::Cosine.into(),
                        ..Default::default()
                    })),
                }),
                ..Default::default()
            })
            .await?;
    }
    Ok(())
}

async fn export_topics(
    client: &Qdrant,
    collection_name: &str,
    topic_vectors: &[f32],
    n_features: usize,
    cluster_labels: &[i32],
    documents: &[DocumentData],
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    let n_topics = topic_vectors.len() / n_features;

    let mut clusters: HashMap<i32, Vec<&str>> = HashMap::new();
    for (idx, &label) in cluster_labels.iter().enumerate() {
        if label >= 0 && idx < documents.len() {
            clusters
                .entry(label)
                .or_default()
                .push(&documents[idx].text);
        }
    }

    let mut all_clusters: Vec<Vec<&str>> = vec![Vec::new(); n_topics];
    for (label, docs) in &clusters {
        if (*label as usize) < n_topics {
            all_clusters[*label as usize] = docs.clone();
        }
    }

    let namer = TfidfTopicNamer::new(10, &all_clusters);

    let mut points = Vec::new();

    for i in 0..n_topics {
        let start_idx = i * n_features;
        let end_idx = start_idx + n_features;
        let vector = topic_vectors[start_idx..end_idx].to_vec();

        // Generate topic name using TF-IDF
        let topic_name = namer.generate_name_for_cluster(i);

        let mut payload: HashMap<String, Value> = HashMap::new();
        payload.insert("name".to_string(), Value::from(topic_name.clone()));
        payload.insert("topic_id".to_string(), Value::from(i as i64));

        // Add cluster statistics
        let doc_count = all_clusters.get(i).map(|v| v.len()).unwrap_or(0);
        payload.insert("document_count".to_string(), Value::from(doc_count as i64));

        println!("Topic {}: {} ({} docs)", i, topic_name, doc_count);

        points.push(PointStruct::new(i as u64, vector, payload));
    }

    let batch_size = 1000;
    for chunk in points.chunks(batch_size) {
        let upsert = UpsertPointsBuilder::new(collection_name, chunk.to_vec()).build();
        client.upsert_points(upsert).await?;
    }

    println!(
        "Exported {} topics with TF-IDF names in {:.2} seconds",
        n_topics,
        start.elapsed().as_secs_f64()
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let overall_start = Instant::now();

    let qdrant_url = env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let qdrant_api_key = env::var("QDRANT_API_KEY").ok();

    let document_collection = "dataset-6-embedder-1-transform-9-jpoisso";
    let topic_collection = "dataset-6-embedder-1-transform-9-jpoisso-topics";

    let topic_collection_reset = env::var("TOPIC_COLLECTION_RESET")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";
    let batch_size = env::var("BATCH_SIZE")
        .unwrap_or_else(|_| "1000".to_string())
        .parse::<u64>()?;
    let n_neighbors = env::var("N_NEIGHBORS")
        .unwrap_or_else(|_| "15".to_string())
        .parse::<usize>()?;
    let n_components = env::var("N_COMPONENTS")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<usize>()?;
    let min_cluster_size = env::var("MIN_CLUSTER_SIZE")
        .unwrap_or_else(|_| "15".to_string())
        .parse::<usize>()?;

    let min_distance = env::var("MIN_DISTANCE")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<f32>()?;
    let limit = env::var("LIMIT").ok().and_then(|s| s.parse::<usize>().ok());

    println!("Configuration:");
    println!("  Qdrant URL: {}", qdrant_url);
    println!("  Document collection: {}", document_collection);
    println!("  Topic collection: {}", topic_collection);
    println!("  Reset topic collection: {}", topic_collection_reset);
    println!("  Batch size: {}", batch_size);
    println!("  N neighbors: {}", n_neighbors);
    println!("  N components: {}", n_components);
    println!("  Min cluster size: {}", min_cluster_size);
    println!("  Limit: {:?}", limit);

    let client = if let Some(api_key) = qdrant_api_key {
        Qdrant::from_url(&qdrant_url).api_key(api_key).build()?
    } else {
        Qdrant::from_url(&qdrant_url).build()?
    };

    let (document_vectors, documents) =
        fetch_documents_with_text(&client, document_collection, batch_size, limit).await?;
    let n_features = get_vector_size(&client, document_collection).await?;
    let n_samples = document_vectors.len() / n_features;

    println!(
        "Processing {} documents with {} features",
        n_samples, n_features
    );

    println!("Reducing dimensionality...");
    let dim_start = Instant::now();
    let umap = reduce_dimensionality(
        &document_vectors,
        n_features,
        n_neighbors,
        n_components,
        min_distance,
    )?;
    println!(
        "Reduced dimensionality of {} vectors in {:.2} seconds",
        n_samples,
        dim_start.elapsed().as_secs_f64()
    );

    println!("Identifying topic clusters...");
    let cluster_start = Instant::now();
    let (hdbscan, topic_vectors) = identify_topic_clusters(
        &umap.embedding,
        n_components,
        min_cluster_size,
        &document_vectors,
        n_features,
    )?;

    let n_topics = topic_vectors.len() / n_features;

    println!(
        "Identified {} topic clusters in {:.2} seconds",
        n_topics,
        cluster_start.elapsed().as_secs_f64()
    );
    println!("  Total clusters: {}", hdbscan.n_clusters);

    println!(
        "  Noise points: {}",
        hdbscan.labels.iter().filter(|&&l| l == -1).count()
    );

    initialize_topic_collection(
        &client,
        topic_collection,
        n_features,
        topic_collection_reset,
    )
    .await?;

    export_topics(
        &client,
        topic_collection,
        &topic_vectors,
        n_features,
        &hdbscan.labels,
        &documents,
    )
    .await?;

    println!(
        "Generated and exported topic vectors in {:.2} seconds",
        overall_start.elapsed().as_secs_f64()
    );

    Ok(())
}
