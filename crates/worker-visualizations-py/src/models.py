"""
Data Models for Visualization Worker

Pydantic models for NATS message deserialization and result publishing.
"""

from typing import Any, Dict, Optional
from uuid import UUID
from pydantic import BaseModel, Field


class QdrantConnectionConfig(BaseModel):
    url: str
    api_key: Optional[str] = None


class VisualizationConfig(BaseModel):
    """Visualization generation parameters."""

    # UMAP parameters
    n_neighbors: int = Field(default=15, description="UMAP n_neighbors parameter")
    min_dist: float = Field(default=0.1, description="UMAP min_dist parameter")
    metric: str = Field(default="cosine", description="UMAP distance metric")

    # HDBSCAN parameters
    min_cluster_size: int = Field(
        default=15, description="HDBSCAN minimum cluster size"
    )
    min_samples: Optional[int] = Field(
        default=5, description="HDBSCAN min_samples parameter"
    )

    # LLM naming configuration
    llm_batch_size: int = Field(
        default=10, description="Number of clusters to process in parallel (1-100)"
    )
    samples_per_cluster: int = Field(
        default=5,
        description="Number of sample texts to send to LLM per cluster (1-100)",
    )

    # Datamapplot create_interactive_plot parameters
    inline_data: bool = Field(
        default=True,
        description="Include data inline in HTML (compressed/base64) vs separate files",
    )
    noise_label: str = Field(
        default="", description="Label string for unlabelled/noise points"
    )
    noise_color: str = Field(
        default="#999999", description="Color for noise points (hex format)"
    )
    color_label_text: bool = Field(
        default=True, description="Use colors for text labels (vs black/white)"
    )
    label_wrap_width: int = Field(
        default=16, description="Character count before wrapping labels"
    )
    width: str = Field(
        default="100%", description="Plot width (HTML iframe width specification)"
    )
    height: int = Field(default=800, description="Plot height in pixels")
    darkmode: bool = Field(default=False, description="Use dark background theme")
    palette_hue_shift: float = Field(
        default=0.0, description="Hue shift in degrees for color palette"
    )
    palette_hue_radius_dependence: float = Field(
        default=1.0, description="Hue variation based on radius"
    )
    palette_theta_range: float = Field(
        default=0.19634954084936207, description="Radius mask restrictiveness (π/16)"
    )
    use_medoids: bool = Field(
        default=False,
        description="Use medoids instead of centroids for cluster positions",
    )
    cluster_boundary_polygons: bool = Field(
        default=False, description="Draw alpha-shape boundary lines around clusters"
    )
    polygon_alpha: float = Field(
        default=0.1, description="Transparency of cluster boundary polygons (0.0-1.0)"
    )
    cvd_safer: bool = Field(
        default=False, description="Use color vision deficiency safer palette"
    )
    enable_topic_tree: bool = Field(
        default=False, description="Build and display topic tree with label hierarchy"
    )

    # Datamapplot render_html parameters (passed through **render_html_kwds)
    title: Optional[str] = Field(default=None, description="Plot title")
    sub_title: Optional[str] = Field(default=None, description="Plot subtitle")
    title_font_size: int = Field(default=36, description="Title font size in points")
    sub_title_font_size: int = Field(
        default=18, description="Subtitle font size in points"
    )
    text_collision_size_scale: float = Field(
        default=3.0, description="Text label collision detection scale"
    )
    text_min_pixel_size: float = Field(
        default=12.0, description="Minimum pixel size of label text"
    )
    text_max_pixel_size: float = Field(
        default=36.0, description="Maximum pixel size of label text"
    )
    font_family: str = Field(
        default="Playfair Display SC", description="Font family for labels and titles"
    )
    font_weight: int = Field(
        default=600, description="Font weight for text labels (0-1000)"
    )
    tooltip_font_family: str = Field(
        default="Playfair Display SC", description="Font family for tooltips"
    )
    tooltip_font_weight: int = Field(
        default=400, description="Font weight for tooltips (0-1000)"
    )
    logo: Optional[str] = Field(
        default=None, description="Logo image URL (http/https/file)"
    )
    logo_width: int = Field(default=256, description="Logo width in pixels")
    line_spacing: float = Field(
        default=0.95, description="Line height spacing in label text"
    )
    min_fontsize: float = Field(
        default=12, description="Minimum font size for cluster labels in points"
    )
    max_fontsize: float = Field(
        default=24, description="Maximum font size for cluster labels in points"
    )
    text_outline_width: float = Field(
        default=8, description="Size of outline around label text"
    )
    text_outline_color: str = Field(
        default="#eeeeeedd", description="Color of outline around label text"
    )
    point_size_scale: Optional[float] = Field(
        default=None, description="Size scale of points (auto if None)"
    )
    point_hover_color: str = Field(
        default="#aa0000bb", description="Color of highlighted hover point"
    )
    point_radius_min_pixels: float = Field(
        default=0.01, description="Minimum point radius in pixels"
    )
    point_radius_max_pixels: float = Field(
        default=24, description="Maximum point radius in pixels"
    )
    point_line_width_min_pixels: float = Field(
        default=0.001, description="Minimum point outline width"
    )
    point_line_width_max_pixels: float = Field(
        default=3, description="Maximum point outline width"
    )
    point_line_width: float = Field(
        default=0.001, description="Absolute point outline width"
    )
    cluster_boundary_line_width: float = Field(
        default=1.0, description="Cluster boundary line width scale"
    )
    initial_zoom_fraction: float = Field(
        default=1.0, description="Fraction of data visible in initial zoom"
    )
    background_color: Optional[str] = Field(
        default=None, description="Background color (hex string, auto if None)"
    )
    background_image: Optional[str] = Field(
        default=None, description="Background image URL"
    )

    # Topic naming (only LLM is supported)
    # LLM naming is conditional: only applied if llm_config is provided in the job
    # If no LLM config, numeric cluster labels are used


class LLMConfig(BaseModel):
    """LLM configuration resolved from API database."""

    llm_id: int
    provider: str  # "cohere", "openai", "internal"
    model: str  # e.g., "command-a", "gpt-4", "mistral-7b-instruct"
    api_key: str
    config: Dict[str, Any] = Field(default_factory=dict)

    class Config:
        # Allow extra fields from API response
        extra = "allow"


class VisualizationTransformJob(BaseModel):
    """Job message from Rust API."""

    job_id: UUID
    visualization_transform_id: int
    visualization_id: int
    owner_id: str
    embedded_dataset_id: int
    qdrant_collection_name: str
    visualization_config: VisualizationConfig
    qdrant_config: QdrantConnectionConfig
    llm_config: Optional[LLMConfig] = None

    class Config:
        json_encoders = {UUID: str}


class VisualizationTransformResult(BaseModel):
    """Result message to Rust API."""

    job_id: UUID = Field(alias="jobId")
    visualization_transform_id: int = Field(alias="visualizationTransformId")
    visualization_id: int = Field(alias="visualizationId")
    owner_id: str = Field(alias="ownerId")
    status: str  # "processing", "success", "failed"
    html_s3_key: Optional[str] = Field(default=None, alias="htmlS3Key")
    point_count: Optional[int] = Field(default=None, alias="pointCount")
    cluster_count: Optional[int] = Field(default=None, alias="clusterCount")
    processing_duration_ms: Optional[int] = Field(
        default=None, alias="processingDurationMs"
    )
    error_message: Optional[str] = Field(default=None, alias="errorMessage")
    stats_json: Dict[str, Any] = Field(default_factory=dict, alias="statsJson")

    class Config:
        populate_by_name = True  # Allow both snake_case and camelCase fields

    def model_dump_json_safe(self, **kwargs) -> Dict[str, Any]:
        """Convert model to dict with proper JSON serialization for UUID and datetime."""
        data = self.model_dump(by_alias=True, exclude_none=True, **kwargs)
        # Ensure UUID is converted to string
        if isinstance(data.get("jobId"), UUID):
            data["jobId"] = str(data["jobId"])
        return data
