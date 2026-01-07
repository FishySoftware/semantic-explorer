use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// Visualization Transform: Creates 3D visualization from Embedded Dataset using UMAP + HDBSCAN
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct VisualizationTransform {
    pub visualization_transform_id: i32,
    pub title: String,
    pub embedded_dataset_id: i32,
    pub owner: String,
    pub is_enabled: bool,
    pub reduced_collection_name: Option<String>, // Qdrant collection for UMAP 3D points
    pub topics_collection_name: Option<String>,  // Qdrant collection for topic centroids
    #[schema(value_type = Object)]
    pub visualization_config: serde_json::Value, // UMAP + HDBSCAN parameters
    pub last_run_status: Option<String>,         // Status: pending, processing, completed, failed
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    #[schema(value_type = Object)]
    pub last_run_stats: Option<serde_json::Value>, // Stats: n_points, n_clusters, processing_duration_ms
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new Visualization Transform
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct CreateVisualizationTransform {
    pub title: String,
    pub embedded_dataset_id: i32,
    #[serde(default)]
    pub visualization_config: VisualizationConfig,
}

/// Request to update an existing Visualization Transform
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct UpdateVisualizationTransform {
    pub title: Option<String>,
    pub is_enabled: Option<bool>,
    pub visualization_config: Option<VisualizationConfig>,
}

/// Request to trigger a Visualization Transform manually
#[allow(dead_code)]
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct TriggerVisualizationTransformRequest {
    pub visualization_transform_id: i32,
}

/// Visualization configuration (UMAP + HDBSCAN parameters)
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct VisualizationConfig {
    // UMAP parameters
    #[serde(default = "default_n_neighbors")]
    pub n_neighbors: i32,
    #[serde(default = "default_n_components")]
    pub n_components: i32,
    #[serde(default = "default_min_dist")]
    pub min_dist: f32,
    #[serde(default = "default_metric")]
    pub metric: String,

    // HDBSCAN parameters
    #[serde(default = "default_min_cluster_size")]
    pub min_cluster_size: i32,
    #[serde(default = "default_min_samples")]
    pub min_samples: Option<i32>,

    // Topic naming configuration
    #[serde(default = "default_topic_naming_mode")]
    pub topic_naming_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_naming_llm_id: Option<i32>,
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            n_neighbors: default_n_neighbors(),
            n_components: default_n_components(),
            min_dist: default_min_dist(),
            metric: default_metric(),
            min_cluster_size: default_min_cluster_size(),
            min_samples: default_min_samples(),
            topic_naming_mode: default_topic_naming_mode(),
            topic_naming_llm_id: None,
        }
    }
}

/// Statistics for a Visualization Transform
#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct VisualizationTransformStats {
    pub visualization_transform_id: i32,
    pub total_points: i64,
    pub total_clusters: i64,
    pub noise_points: i64,
}

/// 3D point from visualization
#[allow(dead_code)]
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct VisualizationPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub cluster_id: i32,
    pub topic_label: Option<String>,
    pub text: String,
    pub item_id: i32,
}

/// Topic cluster information
#[allow(dead_code)]
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct VisualizationTopic {
    pub cluster_id: i32,
    pub label: String,
    pub point_count: i32,
    pub centroid_x: f32,
    pub centroid_y: f32,
    pub centroid_z: f32,
}

// Default values for UMAP parameters
fn default_n_neighbors() -> i32 {
    15
}

fn default_n_components() -> i32 {
    3
}

fn default_min_dist() -> f32 {
    0.1
}

fn default_metric() -> String {
    "cosine".to_string()
}

// Default values for HDBSCAN parameters
fn default_min_cluster_size() -> i32 {
    15
}

fn default_min_samples() -> Option<i32> {
    Some(5)
}

// Default values for topic naming
fn default_topic_naming_mode() -> String {
    "tfidf".to_string()
}

impl VisualizationTransform {
    /// Generate Qdrant collection names for this visualization
    pub fn generate_collection_names(
        visualization_transform_id: i32,
        owner: &str,
    ) -> (String, String) {
        let reduced = format!(
            "visualization-{}-{}-reduced",
            visualization_transform_id, owner
        );
        let topics = format!(
            "visualization-{}-{}-topics",
            visualization_transform_id, owner
        );
        (reduced, topics)
    }
}
