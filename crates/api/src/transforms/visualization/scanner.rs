use anyhow::Result;
use async_nats::Client as NatsClient;
use sqlx::{Pool, Postgres};
use tracing::{debug, info};
use uuid::Uuid;

use semantic_explorer_core::models::{
    LLMConfig, VectorDatabaseConfig, VisualizationConfig, VisualizationTransformJob,
};

use crate::storage::postgres::{embedded_datasets, llms, visualization_transforms};

/// Trigger a visualization transform scan manually
pub async fn trigger_visualization_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    visualization_transform_id: i32,
    owner: &str,
) -> Result<()> {
    info!(
        "Manually triggering visualization transform scan for ID: {}",
        visualization_transform_id
    );

    // Get transform
    let transform = match visualization_transforms::get_visualization_transform_by_id(
        pool,
        visualization_transform_id,
    )
    .await?
    {
        Some(t) => t,
        None => {
            return Err(anyhow::anyhow!(
                "Visualization transform {} not found",
                visualization_transform_id
            ));
        }
    };

    // Verify ownership
    if transform.owner != owner {
        return Err(anyhow::anyhow!("Access denied"));
    }

    // Get embedded dataset
    let embedded_dataset =
        embedded_datasets::get_embedded_dataset(pool, owner, transform.embedded_dataset_id).await?;

    // Create run record
    let run = visualization_transforms::create_visualization_run(pool, visualization_transform_id)
        .await?;
    info!(
        "Created visualization run {} for transform {}",
        run.run_id, visualization_transform_id
    );

    // Parse visualization config - deserialize directly from JSON
    let visualization_config: VisualizationConfig = serde_json::from_value(transform.visualization_config.clone())
        .unwrap_or_else(|e| {
            debug!("Failed to deserialize full config: {}. Using partial config with defaults.", e);
            // Fallback: extract just the required fields manually
            let viz_config = &transform.visualization_config;
            VisualizationConfig {
                n_neighbors: viz_config.get("n_neighbors").and_then(|v| v.as_i64()).unwrap_or(15) as i32,
                min_dist: viz_config.get("min_dist").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
                metric: viz_config.get("metric").and_then(|v| v.as_str()).unwrap_or("cosine").to_string(),
                min_cluster_size: viz_config.get("min_cluster_size").and_then(|v| v.as_i64()).unwrap_or(10) as i32,
                min_samples: viz_config.get("min_samples").and_then(|v| v.as_i64()).map(|v| v as i32),
                topic_naming_llm_id: viz_config.get("topic_naming_llm_id").and_then(|v| v.as_i64()).map(|v| v as i32),
                // Use defaults for all datamapplot parameters
                inline_data: true,
                noise_label: "Unlabelled".to_string(),
                noise_color: "#999999".to_string(),
                color_label_text: true,
                label_wrap_width: 16,
                width: "100%".to_string(),
                height: 800,
                darkmode: false,
                palette_hue_shift: 0.0,
                palette_hue_radius_dependence: 1.0,
                palette_theta_range: 0.19634954,
                use_medoids: false,
                cluster_boundary_polygons: false,
                polygon_alpha: 0.1,
                cvd_safer: false,
                enable_topic_tree: false,
                title: None,
                sub_title: None,
                title_font_size: 36,
                sub_title_font_size: 18,
                text_collision_size_scale: 3.0,
                text_min_pixel_size: 12.0,
                text_max_pixel_size: 36.0,
                font_family: "Roboto".to_string(),
                font_weight: 600,
                tooltip_font_family: "Roboto".to_string(),
                tooltip_font_weight: 400,
                logo: None,
                logo_width: 256,
                line_spacing: 0.95,
                min_fontsize: 12.0,
                max_fontsize: 24.0,
                text_outline_width: 8.0,
                text_outline_color: "#eeeeeedd".to_string(),
                point_size_scale: None,
                point_hover_color: "#aa0000bb".to_string(),
                point_radius_min_pixels: 0.01,
                point_radius_max_pixels: 24.0,
                point_line_width_min_pixels: 0.001,
                point_line_width_max_pixels: 3.0,
                point_line_width: 0.001,
                cluster_boundary_line_width: 1.0,
                initial_zoom_fraction: 1.0,
                background_color: None,
                background_image: None,
            }
        });

    let topic_naming_llm_id = visualization_config.topic_naming_llm_id;

    // Get LLM config if specified
    let llm_config = if let Some(llm_id) = topic_naming_llm_id {
        let llm = llms::get_llm(pool, owner, llm_id).await?;
        
        // Log LLM config details for debugging
        let has_api_key = llm.api_key.is_some();
        debug!(
            "LLM config for visualization: llm_id={}, provider={}, has_api_key={}",
            llm_id, llm.provider, has_api_key
        );    
        Some(LLMConfig {
            llm_id,
            provider: llm.provider.clone(),
            model: llm
                .config
                .get("model")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            api_key: llm.api_key.clone().unwrap_or_default(),
            config: llm.config.clone(),
        })
    } else {
        None
    };

    // Get vector database config
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let vector_db_config = VectorDatabaseConfig {
        database_type: "qdrant".to_string(),
        connection_url: qdrant_url,
        api_key: std::env::var("QDRANT_API_KEY").ok(),
    };

    // Build job
    let job = VisualizationTransformJob {
        job_id: Uuid::new_v4(),
        visualization_transform_id,
        run_id: run.run_id,
        owner: owner.to_string(),
        embedded_dataset_id: transform.embedded_dataset_id,
        qdrant_collection_name: embedded_dataset.collection_name.clone(),
        visualization_config,
        vector_database_config: vector_db_config,
        llm_config,
    };

    // Publish to NATS
    let js = async_nats::jetstream::new(nats.clone());
    let message_id = format!("vt-{}-{}", visualization_transform_id, run.run_id);

    let mut headers = async_nats::HeaderMap::new();
    headers.insert("Nats-Msg-Id", message_id.as_str());

    js.publish_with_headers(
        "workers.visualization-transform".to_string(),
        headers,
        serde_json::to_vec(&job)?.into(),
    )
    .await?
    .await?;

    info!(
        "Published visualization job {} for transform {} (run {})",
        job.job_id, visualization_transform_id, run.run_id
    );

    Ok(())
}
