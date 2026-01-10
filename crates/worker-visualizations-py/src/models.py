"""
Data Models for Visualization Worker

Pydantic models for NATS message deserialization and result publishing.
"""

from datetime import datetime
from typing import Any, Dict, Optional
from uuid import UUID

from pydantic import BaseModel, Field


class VectorDatabaseConfig(BaseModel):
    """Vector database configuration."""

    database_type: str  # "qdrant"
    connection_url: str
    api_key: Optional[str] = None


class VisualizationConfig(BaseModel):
    """Visualization generation parameters."""

    # UMAP parameters
    n_neighbors: int = Field(default=15, description="UMAP n_neighbors parameter")
    n_components: int = Field(default=2, description="UMAP dimensionality (2 or 3)")
    min_dist: float = Field(default=0.1, description="UMAP min_dist parameter")
    metric: str = Field(default="cosine", description="UMAP distance metric")

    # HDBSCAN parameters
    min_cluster_size: int = Field(default=15, description="HDBSCAN minimum cluster size")
    min_samples: Optional[int] = Field(default=5, description="HDBSCAN min_samples parameter")

    # Datamapplot visualization parameters
    # Parameters passed to create_interactive_plot and render_html
    min_fontsize: float = Field(default=12, description="Minimum font size for labels in points")
    max_fontsize: float = Field(default=24, description="Maximum font size for labels in points")
    font_family: str = Field(default="Arial, sans-serif", description="Font family for labels")
    darkmode: bool = Field(default=False, description="Use dark background theme")
    noise_color: str = Field(default="#999999", description="Color for noise points (hex format)")
    label_wrap_width: int = Field(default=16, description="Character count before wrapping labels")
    use_medoids: bool = Field(default=False, description="Use medoids instead of centroids for cluster positions")
    cluster_boundary_polygons: bool = Field(default=True, description="Draw alpha-shape boundary lines around clusters")
    polygon_alpha: float = Field(default=0.3, description="Transparency of cluster boundary polygons (0.0-1.0)")

    # Topic naming (only LLM is supported)
    # LLM naming is conditional: only applied if llm_config is provided in the job
    # If no LLM config, numeric cluster labels are used


class LLMConfig(BaseModel):
    """LLM configuration resolved from API database."""

    llm_id: int
    provider: str  # "cohere", "openai"
    model: str  # e.g., "command-r-plus", "gpt-4"
    api_key: str
    config: Dict[str, Any] = Field(default_factory=dict)

    class Config:
        # Allow extra fields from API response
        extra = "allow"


class VisualizationTransformJob(BaseModel):
    """Job message from Rust API."""

    job_id: UUID
    visualization_transform_id: int
    run_id: int
    owner: str
    embedded_dataset_id: int
    qdrant_collection_name: str
    visualization_config: VisualizationConfig
    vector_database_config: VectorDatabaseConfig
    llm_config: Optional[LLMConfig] = None

    class Config:
        json_encoders = {UUID: str}


class VisualizationTransformResult(BaseModel):
    """Result message to Rust API."""

    job_id: UUID = Field(alias="jobId")
    visualization_transform_id: int = Field(alias="visualizationTransformId")
    run_id: int = Field(alias="runId")
    owner: str
    status: str  # "processing", "completed", "failed"
    started_at: datetime = Field(alias="startedAt")
    completed_at: Optional[datetime] = Field(default=None, alias="completedAt")
    html_s3_key: Optional[str] = Field(default=None, alias="htmlS3Key")
    point_count: Optional[int] = Field(default=None, alias="pointCount")
    cluster_count: Optional[int] = Field(default=None, alias="clusterCount")
    processing_duration_ms: Optional[int] = Field(default=None, alias="processingDurationMs")
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
        # Ensure datetime is ISO format with Z suffix
        if isinstance(data.get("startedAt"), datetime):
            data["startedAt"] = data["startedAt"].isoformat() + "Z"
        if isinstance(data.get("completedAt"), datetime):
            data["completedAt"] = data["completedAt"].isoformat() + "Z"
        return data
