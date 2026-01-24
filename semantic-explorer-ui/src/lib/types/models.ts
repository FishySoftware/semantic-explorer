export interface PaginatedResponse<T> {
	items: T[];
	total_count: number;
	limit: number;
	offset: number;
	has_more?: boolean;
	continuation_token?: string | null;
}

export interface ProcessedFile {
	id: number;
	transform_type: string;
	transform_id: number;
	file_key: string;
	processed_at: string;
	item_count: number;
	process_status: string;
	process_error: string | null;
	processing_duration_ms: number | null;
}

// --- Datasets ---

export interface Dataset {
	dataset_id: number;
	title: string;
	details: string | null;
	owner: string;
	tags: string[];
	is_public: boolean;
	item_count?: number;
	total_chunks?: number;
	created_at?: string;
	updated_at?: string;
}

export interface DatasetWithStats extends Dataset {
	item_count: number;
	total_chunks: number;
}

export interface DatasetItemSummary {
	item_id: number;
	title: string;
	chunk_count: number;
}

export interface DatasetItemChunks {
	chunks: Array<{
		content: string;
		metadata: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
	}>;
	metadata: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
}

export interface PaginatedItems {
	items: DatasetItemSummary[];
	total_count: number;
	has_more: boolean;
}

export interface DatasetTransform {
	dataset_transform_id: number;
	title: string;
	source_dataset_id: number;
	embedder_ids: number[];
	owner: string;
	is_enabled: boolean;
	job_config: any; // eslint-disable-line @typescript-eslint/no-explicit-any
	created_at: string;
	updated_at: string;
}

export interface DatasetTransformStats {
	dataset_transform_id: number;
	embedder_count: number;
	total_batches_processed: number;
	successful_batches: number;
	failed_batches: number;
	total_chunks_embedded: number;
	total_chunks_failed: number;
}

// --- Embedded Datasets ---

export interface EmbeddedDataset {
	embedded_dataset_id: number;
	title: string;
	dataset_transform_id: number;
	source_dataset_id: number;
	embedder_id: number;
	owner: string;
	collection_name: string;
	created_at: string;
	updated_at: string;
	// Optional fields (populated via joins or separate fetches)
	source_dataset_title?: string;
	embedder_name?: string;
	active_point_count?: number;
}

// Define the specific list response structure for Embedded Datasets as it uses a different field name
export interface PaginatedEmbeddedDatasetList {
	embedded_datasets: EmbeddedDataset[];
	total_count: number;
	limit: number;
	offset: number;
}

export interface EmbeddedDatasetStats {
	embedded_dataset_id: number;
	total_batches_processed: number;
	successful_batches: number;
	failed_batches: number;
	processing_batches: number;
	total_chunks_embedded: number;
	total_chunks_failed: number;
	total_chunks_processing: number;
	last_run_at?: string;
	first_processing_at?: string;
	avg_processing_duration_ms?: number;
}

export interface ProcessedBatch {
	id: number;
	embedded_dataset_id: number;
	file_key: string;
	processed_at: string;
	item_count: number;
	process_status: string;
	process_error: string | null;
	processing_duration_ms: number | null;
}

// --- Embedders ---

export interface Embedder {
	embedder_id: number;
	name: string;
	owner: string;
	provider: string;
	base_url: string;
	api_key: string | null;
	config: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
	batch_size?: number;
	dimensions?: number;
	collection_name: string;
	is_public: boolean;
	created_at: string;
	updated_at: string;
	// Optional fields
	title?: string;
}

export type ProviderDefaultConfig = {
	url: string;
	models: string[];
	inputTypes?: string[];
	embeddingTypes?: string[];
	truncate?: string[];
	config: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
};

// --- Collections ---

export interface Collection {
	collection_id: number;
	title: string;
	details: string | null;
	owner: string;
	bucket: string;
	tags: string[];
	created_at: string;
	updated_at: string;
	// Stats
	total_files?: number;
	file_count?: number; // Alias or alternative name
	total_size_bytes?: number;
	processed_files?: number;
}

export interface PaginatedCollectionList {
	collections: Collection[];
	total_count: number;
	limit: number;
	offset: number;
}

export interface CollectionFile {
	key: string;
	size: number;
	last_modified: string | null;
	content_type: string | null;
}

export interface PaginatedFiles {
	files: CollectionFile[];
	page: number;
	page_size: number;
	has_more: boolean;
	continuation_token: string | null;
	total_count: number | null;
}

export interface CollectionTransform {
	collection_transform_id: number;
	dataset_id: number;
	collection_id: number;
	owner_id: string;
	extraction_policy: string; // 'all' | 'extensions' | 'metadata'
	extraction_config: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
	chunking_policy: string; // 'recursive' | 'token' | 'semantic' | 'none'
	chunking_config: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
	embedder_config?: {
		embedder_id: number;
		model: string;
		api_key?: string;
	} | null;
	cron_schedule: string | null;
	is_enabled: boolean;
	last_run_at: string | null;
	next_run_at: string | null;
	created_at: string;
	updated_at: string;
	// Joined fields
	collection_title?: string;
	dataset_title?: string;
}

export interface CollectionTransformStats {
	collection_transform_id: number;
	total_files_processed: number;
	total_chunks_created: number;
	total_errors: number;
	last_processed_file: string | null;
	last_activity_at: string | null;
}

// --- Visualizations ---

export interface VisualizationStats {
	visualization_transform_id: number;
	total_points: number;
	total_clusters: number;
	noise_points: number;
}

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

export interface DatabaseVisualization {
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

// --- Search ---

export interface SearchMatch {
	id: string;
	score: number;
	text: string;
	metadata: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
}

export interface DocumentResult {
	item_id: number;
	item_title: string;
	best_score: number;
	chunk_count: number;
	best_chunk: SearchMatch;
}

export interface EmbeddedDatasetSearchResults {
	embedded_dataset_id: number;
	embedded_dataset_title: string;
	source_dataset_id: number;
	source_dataset_title: string;
	embedder_id: number;
	embedder_name: string;
	collection_name: string;
	matches: SearchMatch[];
	documents?: DocumentResult[];
	error?: string;
}

export interface SearchResponse {
	results: EmbeddedDatasetSearchResults[];
	query: string;
	search_mode: 'documents' | 'chunks';
}

// --- Chat & LLMs ---

export interface ModelInfo {
	id: string;
	name: string;
	description: string;
	size: string;
	capabilities: string[];
}

export interface ModelsResponse {
	models: ModelInfo[];
}

export interface LLM {
	llm_id: number;
	name: string;
	owner_id: string;
	owner_display_name: string;
	provider: string;
	base_url: string;
	api_key: string | null;
	config: Record<string, any> | any; // eslint-disable-line @typescript-eslint/no-explicit-any
	is_public: boolean;
	created_at: string;
	updated_at: string;
}

export interface PaginatedLLMList {
	items: LLM[];
	total_count: number;
	limit: number;
	offset: number;
}

export interface ChatSession {
	session_id: string;
	embedded_dataset_id: number;
	llm_id: number;
	title: string | null;
	created_at: string;
	updated_at: string;
}

export interface ChatMessage {
	message_id: number;
	role: string;
	content: string;
	created_at: string;
	tokens_used: number | null;
	metadata: Record<string, any> | null; // eslint-disable-line @typescript-eslint/no-explicit-any
	// Frontend/Optional props
	documents_retrieved?: number | null;
	status?: string; // 'complete', 'incomplete', 'error'
	retrieved_documents?: RetrievedDocument[];
	embedded_dataset_id?: number;
}

export interface RetrievedDocument {
	document_id: string | null;
	text: string;
	similarity_score: number;
	item_title: string | null;
}
