use cuml_wrapper_rs::{identify_topic_clusters, reduce_dimensionality};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::vectors_output::VectorsOptions;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, ScrollPointsBuilder, UpsertPointsBuilder,
    VectorParams, VectorsConfig,
};
use semantic_explorer_core::llm::{TfidfTopicNamer, TopicNamer};
use semantic_explorer_core::models::{VisualizationTransformJob, VisualizationTransformResult};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info, warn};

/// L2-normalize vectors to unit length for cosine distance calculations
fn normalize_l2(vectors: &[f32], n_features: usize) -> Vec<f32> {
    let n_samples = vectors.len() / n_features;
    let mut normalized = Vec::with_capacity(vectors.len());

    for i in 0..n_samples {
        let start = i * n_features;
        let end = start + n_features;
        let sample = &vectors[start..end];

        // Calculate L2 norm
        let norm: f32 = sample.iter().map(|&x| x * x).sum::<f32>().sqrt();

        // Normalize each component
        if norm > 1e-10 {
            for &val in sample {
                normalized.push(val / norm);
            }
        } else {
            // Handle zero-length vectors by keeping them as-is
            normalized.extend_from_slice(sample);
        }
    }

    normalized
}

#[derive(Clone)]
struct DocumentData {
    text: String,
}

pub async fn process_visualization_job(
    job: VisualizationTransformJob,
    nats_client: &async_nats::Client,
) -> Result<(usize, i32, i64), Box<dyn std::error::Error>> {
    let start = Instant::now();

    info!(
        "Processing visualization job {} for transform {}",
        job.job_id, job.visualization_transform_id
    );
    debug!("  Source collection: {}", job.source_collection);
    debug!("  Output (reduced): {}", job.output_collection_reduced);
    debug!("  Output (topics): {}", job.output_collection_topics);

    let client = if let Some(api_key) = &job.vector_database_config.api_key {
        Qdrant::from_url(&job.vector_database_config.connection_url)
            .api_key(api_key.clone())
            .build()?
    } else {
        Qdrant::from_url(&job.vector_database_config.connection_url).build()?
    };

    let (document_vectors, documents) =
        fetch_documents_with_text(&client, &job.source_collection, 1000, None).await?;

    let n_features = get_vector_size(&client, &job.source_collection).await?;
    let n_samples = document_vectors.len() / n_features;

    info!(
        "Processing {} documents with {} features",
        n_samples, n_features
    );

    let dim_start = Instant::now();
    // L2-normalize input vectors for better cosine distance calculations
    let normalized_document_vectors = normalize_l2(&document_vectors, n_features);

    // spread: Controls the scale of the embedding - higher values spread points more in 3D
    // Using spread=2.0 to avoid flat "saucer" shaped embeddings
    let spread = 2.0_f32;
    
    // init: 0=random, 1=spectral
    // Random initialization tends to produce more 3D-like embeddings
    // Spectral initialization is faster but often produces flat embeddings
    let init = 0_i32; // Random initialization for better 3D distribution

    info!(
        "Running UMAP with n_neighbors={}, n_components={}, min_dist={}, spread={}, init={}, metric={}",
        job.visualization_config.n_neighbors,
        job.visualization_config.n_components,
        job.visualization_config.min_dist,
        spread,
        init,
        job.visualization_config.metric
    );

    let umap = reduce_dimensionality(
        &normalized_document_vectors,
        n_features,
        job.visualization_config.n_neighbors as usize,
        job.visualization_config.n_components as usize,
        job.visualization_config.min_dist,
        spread,
        init,
        "cosine",
    )?;

    info!(
        "Reduced dimensionality in {:.2}s (output shape: {}x{})",
        dim_start.elapsed().as_secs_f64(),
        umap.embedding.len() / job.visualization_config.n_components as usize,
        job.visualization_config.n_components
    );

    let cluster_start = Instant::now();
    // L2-normalize UMAP embeddings before clustering
    // Note: cuML's HDBSCAN only supports L2 distance, but L2 distance on L2-normalized vectors ≈ cosine distance
    let normalized_embeddings = normalize_l2(
        &umap.embedding,
        job.visualization_config.n_components as usize,
    );

    // Get min_samples from config, defaulting to min_cluster_size if not specified
    let min_samples = job
        .visualization_config
        .min_samples
        .unwrap_or(job.visualization_config.min_cluster_size) as usize;

    // Cluster selection method: 0 = EOM (Excess of Mass), 1 = LEAF (more fine-grained clusters)
    // LEAF tends to produce more, smaller clusters by selecting leaves of the condensed tree
    let cluster_selection_method = 1_i32; // LEAF
    
    // Cluster selection epsilon: Distance threshold for merging clusters
    // 0.0 = no merging (more clusters), higher values merge nearby clusters
    let cluster_selection_epsilon = 0.0_f32;

    info!(
        "Running HDBSCAN with min_cluster_size={}, min_samples={}, method=LEAF, epsilon={}, n_samples={}, n_components={}",
        job.visualization_config.min_cluster_size, min_samples, cluster_selection_epsilon, n_samples, job.visualization_config.n_components
    );

    let (hdbscan, topic_vectors) = identify_topic_clusters(
        &normalized_embeddings,
        job.visualization_config.n_components as usize,
        job.visualization_config.min_cluster_size as usize,
        min_samples,
        cluster_selection_method,
        cluster_selection_epsilon,
        "euclidean",
        &document_vectors,
        n_features,
    )?;

    let n_clusters = hdbscan.n_clusters;
    info!(
        "Identified {} clusters in {:.2}s (cluster labels range: {:?})",
        n_clusters,
        cluster_start.elapsed().as_secs_f64(),
        if hdbscan.labels.is_empty() {
            None
        } else {
            Some((
                *hdbscan.labels.iter().min().unwrap(),
                *hdbscan.labels.iter().max().unwrap(),
            ))
        }
    );

    let label_start = Instant::now();
    let topic_labels = generate_topic_labels(
        &hdbscan.labels,
        &documents,
        n_clusters as usize,
        &job.visualization_config,
    )
    .await;
    info!(
        "Generated topic labels in {:.2}s",
        label_start.elapsed().as_secs_f64()
    );

    // Initialize reduced points collection
    initialize_collection(
        &client,
        &job.output_collection_reduced,
        job.visualization_config.n_components as usize,
        true,
    )
    .await?;

    // Export reduced vectors with cluster assignments
    export_reduced_vectors(
        &client,
        &job.output_collection_reduced,
        &umap.embedding,
        job.visualization_config.n_components as usize,
        &hdbscan.labels,
        &topic_labels,
        &documents,
    )
    .await?;

    // Initialize topics collection
    initialize_collection(&client, &job.output_collection_topics, n_features, true).await?;

    // Export topic centroids
    export_topics(
        &client,
        &job.output_collection_topics,
        &topic_vectors,
        n_features,
        &hdbscan.labels,
        &topic_labels,
    )
    .await?;

    let duration_ms = start.elapsed().as_millis() as i64;
    info!(
        "Completed visualization job in {:.2}s",
        duration_ms as f64 / 1000.0
    );

    // Send result message to NATS
    send_result(
        nats_client,
        &job,
        Ok((n_samples, n_clusters)),
        Some(duration_ms),
    )
    .await?;

    Ok((n_samples, n_clusters, duration_ms))
}

async fn send_result(
    nats: &async_nats::Client,
    job: &VisualizationTransformJob,
    result: Result<(usize, i32), String>,
    processing_duration_ms: Option<i64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (n_points, n_clusters, status, error) = match result {
        Ok((points, clusters)) => (points, clusters, "completed".to_string(), None),
        Err(e) => (0, 0, "failed".to_string(), Some(e)),
    };

    let result_msg = VisualizationTransformResult {
        job_id: job.job_id,
        visualization_transform_id: job.visualization_transform_id,
        status,
        error,
        processing_duration_ms,
        n_points,
        n_clusters,
        output_collection_reduced: job.output_collection_reduced.clone(),
        output_collection_topics: job.output_collection_topics.clone(),
    };

    let payload = serde_json::to_vec(&result_msg)?;
    nats.publish("worker.result.visualization".to_string(), payload.into())
        .await?;
    Ok(())
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
        let mut scroll_builder = ScrollPointsBuilder::new(collection_name)
            .limit(batch_size as u32)
            .with_payload(true)
            .with_vectors(true);

        if let Some(ref offset_id) = offset {
            scroll_builder = scroll_builder.offset(offset_id.clone());
        }

        let response = client.scroll(scroll_builder).await?;
        let points = response.result;

        if points.is_empty() {
            break;
        }

        for point in &points {
            if let Some(vectors_output) = &point.vectors
                && let Some(vector_options) = &vectors_output.vectors_options
            {
                match vector_options {
                    VectorsOptions::Vector(v) => match v.clone().into_vector() {
                        qdrant_client::qdrant::vector_output::Vector::Dense(dense) => {
                            all_vectors.extend_from_slice(&dense.data);
                        }
                        _ => continue,
                    },
                    _ => continue,
                }
            }

            let text = point
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            all_documents.push(DocumentData { text });
        }

        let n_vectors = all_vectors.len();
        if let Some(limit_val) = limit
            && n_vectors >= limit_val
        {
            break;
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

    info!(
        "Fetched {} document vectors in {:.2}s",
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
        && let Some(qdrant_client::qdrant::vectors_config::Config::Params(params)) =
            vectors_config.config
    {
        return Ok(params.size as usize);
    }
    Err("Could not determine vector size".into())
}

async fn initialize_collection(
    client: &Qdrant,
    collection_name: &str,
    vector_size: usize,
    reset: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let exists = client.collection_exists(collection_name).await?;

    if exists && reset {
        info!("Deleting existing collection: {}", collection_name);
        client.delete_collection(collection_name).await?;
    }

    if !exists || reset {
        info!(
            "Creating collection: {} with vector size {}",
            collection_name, vector_size
        );
        let create_collection = CreateCollectionBuilder::new(collection_name)
            .vectors_config(VectorsConfig {
                config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                    VectorParams {
                        size: vector_size as u64,
                        distance: Distance::Cosine.into(),
                        ..Default::default()
                    },
                )),
            })
            .build();

        client.create_collection(create_collection).await?;
    }

    Ok(())
}

async fn export_reduced_vectors(
    client: &Qdrant,
    collection_name: &str,
    reduced_vectors: &[f32],
    n_components: usize,
    labels: &[i32],
    topic_labels: &HashMap<i32, String>,
    documents: &[DocumentData],
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let n_samples = reduced_vectors.len() / n_components;

    // Pre-allocate vector with exact capacity to avoid reallocations
    let mut points = Vec::with_capacity(n_samples);
    for i in 0..n_samples {
        let vector: Vec<f32> = reduced_vectors[i * n_components..(i + 1) * n_components].to_vec();
        let cluster_id = labels[i];

        let mut payload = HashMap::new();
        payload.insert("cluster_id".to_string(), Value::from(cluster_id as i64));

        if let Some(label) = topic_labels.get(&cluster_id) {
            payload.insert("topic_label".to_string(), Value::from(label.clone()));
        }

        if i < documents.len() {
            payload.insert("text".to_string(), Value::from(documents[i].text.clone()));
        }

        let point = PointStruct::new(i as u64, vector, payload);
        points.push(point);
    }

    let upsert_points = UpsertPointsBuilder::new(collection_name, points).build();
    client.upsert_points(upsert_points).await?;

    info!(
        "Exported {} reduced vectors to {} in {:.2}s",
        n_samples,
        collection_name,
        start.elapsed().as_secs_f64()
    );

    Ok(())
}

async fn export_topics(
    client: &Qdrant,
    collection_name: &str,
    topic_vectors: &[f32],
    n_features: usize,
    labels: &[i32],
    topic_labels: &HashMap<i32, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let n_topics = topic_vectors.len() / n_features;

    // Pre-allocate vector with exact capacity to avoid reallocations
    let mut points = Vec::with_capacity(n_topics);
    for i in 0..n_topics {
        let vector: Vec<f32> = topic_vectors[i * n_features..(i + 1) * n_features].to_vec();
        let cluster_id = i as i32;
        let label = topic_labels
            .get(&cluster_id)
            .cloned()
            .unwrap_or_else(|| format!("Topic {}", i));

        let cluster_size = labels.iter().filter(|&&l| l == cluster_id).count();

        let mut payload = HashMap::new();
        payload.insert("cluster_id".to_string(), Value::from(cluster_id as i64));
        payload.insert("label".to_string(), Value::from(label));
        payload.insert("size".to_string(), Value::from(cluster_size as i64));

        let point = PointStruct::new(i as u64, vector, payload);
        points.push(point);
    }

    let upsert_points = UpsertPointsBuilder::new(collection_name, points).build();
    client.upsert_points(upsert_points).await?;

    info!(
        "Exported {} topic centroids to {} in {:.2}s",
        n_topics,
        collection_name,
        start.elapsed().as_secs_f64()
    );

    Ok(())
}

async fn generate_topic_labels(
    labels: &[i32],
    documents: &[DocumentData],
    n_clusters: usize,
    config: &semantic_explorer_core::models::VisualizationConfig,
) -> HashMap<i32, String> {
    let mut cluster_texts: HashMap<i32, Vec<String>> = HashMap::new();

    for (i, &cluster_id) in labels.iter().enumerate() {
        if cluster_id >= 0 && i < documents.len() {
            cluster_texts
                .entry(cluster_id)
                .or_default()
                .push(documents[i].text.clone());
        }
    }

    let mut topic_labels = HashMap::new();

    // Select topic namer based on configuration
    let namer: Box<dyn TopicNamer> = match config.topic_naming_mode.to_lowercase().as_str() {
        "llm" => {
            if let Some(llm_id) = config.topic_naming_llm_id {
                // TODO: Fetch LLM configuration from database using llm_id
                // For now, fall back to TF-IDF
                warn!(
                    "LLM topic naming not yet fully implemented (LLM ID: {}), falling back to TF-IDF",
                    llm_id
                );
                Box::new(TfidfTopicNamer::new())
            } else {
                warn!("LLM topic naming mode selected but no LLM ID provided, using TF-IDF");
                Box::new(TfidfTopicNamer::new())
            }
        }
        _ => Box::new(TfidfTopicNamer::new()),
    };

    for cluster_id in 0..n_clusters as i32 {
        if let Some(texts) = cluster_texts.get(&cluster_id) {
            match namer.generate_topic_label(texts).await {
                Ok(label) => {
                    topic_labels.insert(cluster_id, label);
                }
                Err(e) => {
                    warn!(
                        "Failed to generate topic label for cluster {}: {}, using fallback",
                        cluster_id, e
                    );
                    topic_labels.insert(cluster_id, format!("Topic {}", cluster_id));
                }
            }
        } else {
            topic_labels.insert(cluster_id, format!("Topic {}", cluster_id));
        }
    }

    topic_labels
}
