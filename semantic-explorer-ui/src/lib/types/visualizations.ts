/**
 * Shared type definitions for visualizations
 * These match the backend Rust structs in crates/core/src/models.rs
 */

export interface VisualizationConfig {
	n_neighbors: number;
	min_dist: number;
	metric: string;
	min_cluster_size: number;
	min_samples: number | null;
	topic_naming_llm_id: number | null;
	// Datamapplot create_interactive_plot parameters
	inline_data: boolean;
	noise_label: string;
	noise_color: string;
	color_label_text: boolean;
	label_wrap_width: number;
	width: string;
	height: number;
	darkmode: boolean;
	palette_hue_shift: number;
	palette_hue_radius_dependence: number;
	palette_theta_range: number;
	use_medoids: boolean;
	cluster_boundary_polygons: boolean;
	polygon_alpha: number;
	cvd_safer: boolean;
	enable_topic_tree: boolean;
	// Datamapplot render_html parameters
	title: string | null;
	sub_title: string | null;
	title_font_size: number;
	sub_title_font_size: number;
	text_collision_size_scale: number;
	text_min_pixel_size: number;
	text_max_pixel_size: number;
	font_family: string;
	font_weight: number;
	tooltip_font_family: string;
	tooltip_font_weight: number;
	logo: string | null;
	logo_width: number;
	line_spacing: number;
	min_fontsize: number;
	max_fontsize: number;
	text_outline_width: number;
	text_outline_color: string;
	point_size_scale: number | null;
	point_hover_color: string;
	point_radius_min_pixels: number;
	point_radius_max_pixels: number;
	point_line_width_min_pixels: number;
	point_line_width_max_pixels: number;
	point_line_width: number;
	cluster_boundary_line_width: number;
	initial_zoom_fraction: number;
	background_color: string | null;
	background_image: string | null;
}

export interface VisualizationTransform {
	visualization_transform_id: number;
	title: string;
	embedded_dataset_id: number;
	owner: string;
	is_enabled: boolean;
	reduced_collection_name: string | null;
	topics_collection_name: string | null;
	visualization_config: VisualizationConfig;
	last_run_status: string | null;
	last_run_at: string | null;
	last_error: string | null;
	last_run_stats: {
		n_points?: number;
		n_clusters?: number;
		processing_duration_ms?: number;
		point_count?: number;
		cluster_count?: number;
	} | null;
	created_at: string;
	updated_at: string;
}

export interface Visualization {
	visualization_id: number;
	visualization_transform_id: number;
	status: string;
	started_at: string | null;
	completed_at: string | null;
	html_s3_key: string | null;
	point_count: number | null;
	cluster_count: number | null;
	error_message: string | null;
	stats_json: Record<string, unknown> | null;
	created_at: string;
}
