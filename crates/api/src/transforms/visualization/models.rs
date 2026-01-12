use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// Visualization Transform: Creates interactive visualizations from Embedded Datasets
/// Generates UMAP reductions + HDBSCAN clustering + datamapplot visualizations
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct VisualizationTransform {
    pub visualization_transform_id: i32,
    pub title: String,
    pub embedded_dataset_id: i32,
    pub owner: String,
    pub is_enabled: bool,
    pub reduced_collection_name: Option<String>,
    pub topics_collection_name: Option<String>,
    #[schema(value_type = Object)]
    pub visualization_config: serde_json::Value,
    pub last_run_status: Option<String>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub last_run_stats: Option<serde_json::Value>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

/// Visualization: Individual execution of a visualization transform
#[derive(Serialize, ToSchema, FromRow, Debug, Clone)]
pub struct Visualization {
    pub visualization_id: i32,
    pub visualization_transform_id: i32,
    pub status: String,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub started_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub completed_at: Option<DateTime<Utc>>,
    pub html_s3_key: Option<String>,
    pub point_count: Option<i32>,
    pub cluster_count: Option<i32>,
    pub error_message: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub stats_json: Option<serde_json::Value>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
}

/// Request to create a new Visualization Transform
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct CreateVisualizationTransform {
    pub title: String,
    pub embedded_dataset_id: i32,
    #[serde(default)]
    pub llm_id: Option<i32>, // Optional LLM for topic naming
    // UMAP parameters
    #[serde(default = "default_n_neighbors")]
    pub n_neighbors: i32,
    #[serde(default = "default_min_dist")]
    pub min_dist: f32,
    #[serde(default = "default_metric")]
    pub metric: String,
    // HDBSCAN parameters
    #[serde(default = "default_min_cluster_size")]
    pub min_cluster_size: i32,
    #[serde(default)]
    pub min_samples: Option<i32>,
    // LLM naming configuration
    #[serde(default = "default_llm_batch_size")]
    pub llm_batch_size: i32,
    #[serde(default = "default_samples_per_cluster")]
    pub samples_per_cluster: i32,
    // Datamapplot visualization parameters
    #[serde(default = "default_min_fontsize")]
    pub min_fontsize: f32,
    #[serde(default = "default_max_fontsize")]
    pub max_fontsize: f32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_darkmode")]
    pub darkmode: bool,
    #[serde(default = "default_noise_color")]
    pub noise_color: String,
    #[serde(default = "default_label_wrap_width")]
    pub label_wrap_width: i32,
    #[serde(default = "default_use_medoids")]
    pub use_medoids: bool,
    #[serde(default = "default_cluster_boundary_polygons")]
    pub cluster_boundary_polygons: bool,
    #[serde(default = "default_polygon_alpha")]
    pub polygon_alpha: f32,
}

/// Request to update an existing Visualization Transform
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct UpdateVisualizationTransform {
    pub title: Option<String>,
    pub is_enabled: Option<bool>,
    pub visualization_config: Option<serde_json::Value>,
}

/// Statistics for a Visualization Transform
#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct VisualizationTransformStats {
    pub visualization_transform_id: i32,
    pub latest_visualization: Option<Visualization>,
    pub total_runs: i64,
    pub successful_runs: i64,
    pub failed_runs: i64,
}

// Default values for visualization parameters
fn default_n_neighbors() -> i32 {
    15
}

fn default_min_dist() -> f32 {
    0.1
}

fn default_metric() -> String {
    "cosine".to_string()
}

fn default_min_cluster_size() -> i32 {
    10
}

fn default_llm_batch_size() -> i32 {
    10
}

fn default_samples_per_cluster() -> i32 {
    5
}

fn default_min_fontsize() -> f32 {
    12.0
}

fn default_max_fontsize() -> f32 {
    24.0
}

fn default_font_family() -> String {
    "Arial, sans-serif".to_string()
}

fn default_darkmode() -> bool {
    false
}

fn default_noise_color() -> String {
    "#999999".to_string()
}

fn default_label_wrap_width() -> i32 {
    16
}

fn default_use_medoids() -> bool {
    false
}

fn default_cluster_boundary_polygons() -> bool {
    true
}

fn default_polygon_alpha() -> f32 {
    0.3
}
