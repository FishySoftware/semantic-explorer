use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTransformJob {
    pub job_id: Uuid,
    pub source_file_key: String,
    pub bucket: String,
    pub collection_id: i32,
    pub collection_transform_id: i32,
    pub owner_id: String,
    pub extraction_config: serde_json::Value,
    pub chunking_config: serde_json::Value,
    /// Optional embedder config for semantic chunking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_config: Option<EmbedderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTransformResult {
    pub job_id: Uuid,
    pub collection_transform_id: i32,
    pub owner_id: String,
    pub source_file_key: String,
    pub bucket: String,
    pub chunks_file_key: String,
    pub chunk_count: usize,
    pub status: String,
    pub error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetTransformJob {
    pub job_id: Uuid,
    pub batch_file_key: String,
    pub bucket: String,
    pub dataset_id: i32,
    pub dataset_transform_id: i32,
    pub embedded_dataset_id: i32, // NEW: Identifies which embedded dataset this job is for
    pub owner_id: String,
    pub embedder_config: EmbedderConfig,
    pub vector_database_config: VectorDatabaseConfig,
    pub collection_name: String,
    #[serde(default)]
    pub wipe_collection: bool,
    #[serde(default)]
    pub batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetTransformScanJob {
    pub job_id: Uuid,
    pub dataset_transform_id: i32,
    pub owner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetTransformResult {
    pub job_id: Uuid,
    pub dataset_transform_id: i32,
    pub embedded_dataset_id: i32, // NEW: Identifies which embedded dataset this result is for
    pub owner_id: String,
    pub batch_file_key: String,
    pub chunk_count: usize,
    pub status: String,
    pub error: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub config: serde_json::Value,
    pub max_batch_size: i32,
    #[serde(default = "default_max_input_tokens")]
    pub max_input_tokens: i32,
}

fn default_max_input_tokens() -> i32 {
    8191 // OpenAI default for text-embedding-ada-002
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDatabaseConfig {
    pub database_type: String,
    pub connection_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub llm_id: i32,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationTransformJob {
    pub job_id: Uuid,
    pub visualization_transform_id: i32,
    pub visualization_id: i32,
    pub owner_id: String,
    pub embedded_dataset_id: i32,
    pub qdrant_collection_name: String,
    pub visualization_config: VisualizationConfig,
    pub vector_database_config: VectorDatabaseConfig,
    pub llm_config: Option<LLMConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    // UMAP parameters
    pub n_neighbors: i32,
    pub min_dist: f32,
    pub metric: String,
    // HDBSCAN parameters
    pub min_cluster_size: i32,
    pub min_samples: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_naming_llm_id: Option<i32>, // LLM database ID when mode = "llm"

    // LLM naming configuration
    #[serde(default = "default_llm_batch_size")]
    pub llm_batch_size: i32, // Number of clusters to process in parallel (1-100, default 10)
    #[serde(default = "default_samples_per_cluster")]
    pub samples_per_cluster: i32, // Number of sample texts to send to LLM per cluster (1-100, default 5)

    // Datamapplot create_interactive_plot parameters
    #[serde(default = "default_inline_data")]
    pub inline_data: bool,
    #[serde(default = "default_noise_label")]
    pub noise_label: String,
    #[serde(default = "default_noise_color")]
    pub noise_color: String,
    #[serde(default = "default_color_label_text")]
    pub color_label_text: bool,
    #[serde(default = "default_label_wrap_width")]
    pub label_wrap_width: i32,
    #[serde(default = "default_width")]
    pub width: String,
    #[serde(default = "default_height")]
    pub height: i32,
    #[serde(default = "default_darkmode")]
    pub darkmode: bool,
    #[serde(default = "default_palette_hue_shift")]
    pub palette_hue_shift: f32,
    #[serde(default = "default_palette_hue_radius_dependence")]
    pub palette_hue_radius_dependence: f32,
    #[serde(default = "default_palette_theta_range")]
    pub palette_theta_range: f32,
    #[serde(default = "default_use_medoids")]
    pub use_medoids: bool,
    #[serde(default = "default_cluster_boundary_polygons")]
    pub cluster_boundary_polygons: bool,
    #[serde(default = "default_polygon_alpha")]
    pub polygon_alpha: f32,
    #[serde(default = "default_cvd_safer")]
    pub cvd_safer: bool,
    #[serde(default = "default_enable_topic_tree")]
    pub enable_topic_tree: bool,

    // Datamapplot render_html parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_title: Option<String>,
    #[serde(default = "default_title_font_size")]
    pub title_font_size: i32,
    #[serde(default = "default_sub_title_font_size")]
    pub sub_title_font_size: i32,
    #[serde(default = "default_text_collision_size_scale")]
    pub text_collision_size_scale: f32,
    #[serde(default = "default_text_min_pixel_size")]
    pub text_min_pixel_size: f32,
    #[serde(default = "default_text_max_pixel_size")]
    pub text_max_pixel_size: f32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_weight")]
    pub font_weight: i32,
    #[serde(default = "default_tooltip_font_family")]
    pub tooltip_font_family: String,
    #[serde(default = "default_tooltip_font_weight")]
    pub tooltip_font_weight: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default = "default_logo_width")]
    pub logo_width: i32,
    #[serde(default = "default_line_spacing")]
    pub line_spacing: f32,
    #[serde(default = "default_min_fontsize")]
    pub min_fontsize: f32,
    #[serde(default = "default_max_fontsize")]
    pub max_fontsize: f32,
    #[serde(default = "default_text_outline_width")]
    pub text_outline_width: f32,
    #[serde(default = "default_text_outline_color")]
    pub text_outline_color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_size_scale: Option<f32>,
    #[serde(default = "default_point_hover_color")]
    pub point_hover_color: String,
    #[serde(default = "default_point_radius_min_pixels")]
    pub point_radius_min_pixels: f32,
    #[serde(default = "default_point_radius_max_pixels")]
    pub point_radius_max_pixels: f32,
    #[serde(default = "default_point_line_width_min_pixels")]
    pub point_line_width_min_pixels: f32,
    #[serde(default = "default_point_line_width_max_pixels")]
    pub point_line_width_max_pixels: f32,
    #[serde(default = "default_point_line_width")]
    pub point_line_width: f32,
    #[serde(default = "default_cluster_boundary_line_width")]
    pub cluster_boundary_line_width: f32,
    #[serde(default = "default_initial_zoom_fraction")]
    pub initial_zoom_fraction: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image: Option<String>,
}

// Default value functions for serde
fn default_inline_data() -> bool {
    true
}
fn default_noise_label() -> String {
    "Unlabelled".to_string()
}
fn default_noise_color() -> String {
    "#999999".to_string()
}
fn default_color_label_text() -> bool {
    true
}
fn default_label_wrap_width() -> i32 {
    16
}
fn default_llm_batch_size() -> i32 {
    10
}
fn default_samples_per_cluster() -> i32 {
    5
}
fn default_width() -> String {
    "100%".to_string()
}
fn default_height() -> i32 {
    800
}
fn default_darkmode() -> bool {
    false
}
fn default_palette_hue_shift() -> f32 {
    0.0
}
fn default_palette_hue_radius_dependence() -> f32 {
    1.0
}
fn default_palette_theta_range() -> f32 {
    0.196_349_55
} // π/16
fn default_use_medoids() -> bool {
    false
}
fn default_cluster_boundary_polygons() -> bool {
    false
}
fn default_polygon_alpha() -> f32 {
    0.1
}
fn default_cvd_safer() -> bool {
    false
}
fn default_enable_topic_tree() -> bool {
    false
}
fn default_title_font_size() -> i32 {
    36
}
fn default_sub_title_font_size() -> i32 {
    18
}
fn default_text_collision_size_scale() -> f32 {
    3.0
}
fn default_text_min_pixel_size() -> f32 {
    12.0
}
fn default_text_max_pixel_size() -> f32 {
    36.0
}
fn default_font_family() -> String {
    "Playfair Display SC".to_string()
}
fn default_font_weight() -> i32 {
    600
}
fn default_tooltip_font_family() -> String {
    "Playfair Display SC".to_string()
}
fn default_tooltip_font_weight() -> i32 {
    400
}
fn default_logo_width() -> i32 {
    256
}
fn default_line_spacing() -> f32 {
    0.95
}
fn default_min_fontsize() -> f32 {
    12.0
}
fn default_max_fontsize() -> f32 {
    24.0
}
fn default_text_outline_width() -> f32 {
    8.0
}
fn default_text_outline_color() -> String {
    "#eeeeeedd".to_string()
}
fn default_point_hover_color() -> String {
    "#aa0000bb".to_string()
}
fn default_point_radius_min_pixels() -> f32 {
    0.01
}
fn default_point_radius_max_pixels() -> f32 {
    24.0
}
fn default_point_line_width_min_pixels() -> f32 {
    0.001
}
fn default_point_line_width_max_pixels() -> f32 {
    3.0
}
fn default_point_line_width() -> f32 {
    0.001
}
fn default_cluster_boundary_line_width() -> f32 {
    1.0
}
fn default_initial_zoom_fraction() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizationTransformResult {
    pub job_id: Uuid,
    pub visualization_transform_id: i32,
    pub visualization_id: i32,
    pub owner_id: String,
    pub status: String,
    pub error_message: Option<String>,
    #[serde(rename = "htmlS3Key")]
    pub html_s3_key: Option<String>,
    pub point_count: Option<usize>,
    pub cluster_count: Option<i32>,
    pub processing_duration_ms: Option<i64>,
    pub stats_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T: ToSchema> {
    pub items: Vec<T>,
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
}
