"""
Visualization Processing Pipeline

Handles UMAP dimensionality reduction, HDBSCAN clustering, and datamapplot
interactive visualization generation.
"""

import logging
import time
from typing import Any, Dict, Optional

import numpy as np
from qdrant_client import QdrantClient
from qdrant_client.models import PointStruct
from umap import UMAP
from hdbscan import HDBSCAN
import datamapplot

try:
    # Try relative imports (for package execution)
    from .models import VisualizationTransformJob, LLMConfig, VisualizationConfig
    from .llm_namer import LLMProvider
except ImportError:
    # Fallback to absolute imports (for direct script execution)
    from models import VisualizationTransformJob, LLMConfig, VisualizationConfig
    from llm_namer import LLMProvider

logger = logging.getLogger(__name__)


class VisualizationProcessor:
    """Processes vectors and generates interactive visualizations."""

    def __init__(self, qdrant_url: str):
        """Initialize processor with Qdrant connection using gRPC."""
        init_start = time.time()
        # Convert HTTP URL to gRPC URL
        grpc_url = self._convert_to_grpc_url(qdrant_url)
        logger.debug(f"Converting Qdrant URL: {qdrant_url} -> {grpc_url}")
        self.qdrant = QdrantClient(url=grpc_url, prefer_grpc=True)
        init_elapsed = time.time() - init_start
        logger.info(f"Initialized Qdrant client in {init_elapsed:.3f}s: {grpc_url}")

    @staticmethod
    def _convert_to_grpc_url(url: str) -> str:
        """Convert HTTP Qdrant URL to gRPC URL."""
        if url.startswith("http://"):
            host_port = url.replace("http://", "")
            # Default gRPC port is 6334
            if ":" in host_port:
                host = host_port.split(":")[0]
                return f"{host}:6334"
            else:
                return f"{host_port}:6334"
        elif url.startswith("https://"):
            host_port = url.replace("https://", "")
            if ":" in host_port:
                host = host_port.split(":")[0]
                return f"{host}:6334"
            else:
                return f"{host_port}:6334"
        return url

    async def process_job(
        self, job: VisualizationTransformJob, llm_provider: Optional[LLMProvider] = None
    ) -> Dict[str, Any]:
        """
        Process visualization job end-to-end.

        Args:
            job: Visualization transform job
            llm_provider: Optional LLM provider for topic naming

        Returns:
            Dictionary with keys:
            - html: Generated interactive HTML
            - point_count: Number of points
            - cluster_count: Number of clusters
            - stats: Processing statistics

        Raises:
            Exception: If processing fails
        """
        logger.info(
            f"Starting visualization processing for transform {job.visualization_transform_id}"
        )

        # Fetch vectors from Qdrant
        logger.info(f"Fetching vectors from Qdrant collection: {job.qdrant_collection_name}")
        vectors, ids, texts = await self._fetch_vectors_from_qdrant(
            job.qdrant_collection_name, job.owner
        )

        logger.info(f"Fetched {len(vectors)} vectors")

        # Apply UMAP dimensionality reduction
        logger.info(
            f"Applying UMAP: n_neighbors={job.visualization_config.n_neighbors}, "
            f"n_components={job.visualization_config.n_components}, "
            f"min_dist={job.visualization_config.min_dist}, "
            f"metric={job.visualization_config.metric}"
        )
        umap_vectors = await self._apply_umap(vectors, job)

        # Apply HDBSCAN clustering
        logger.info(
            f"Applying HDBSCAN: min_cluster_size={job.visualization_config.min_cluster_size}, "
            f"min_samples={job.visualization_config.min_samples}"
        )
        labels = await self._apply_hdbscan(umap_vectors, job)

        # Generate cluster labels/names
        cluster_labels = await self._generate_cluster_labels(
            labels, texts, job, llm_provider
        )

        # Generate interactive visualization
        logger.info("Generating interactive visualization with datamapplot")
        html_content = await self._generate_visualization(
            umap_vectors, labels, cluster_labels, texts, job.visualization_config
        )

        # Prepare result
        unique_clusters = len(set(labels[labels >= 0]))  # Exclude noise points (-1)
        result = {
            "html": html_content,
            "point_count": len(vectors),
            "cluster_count": unique_clusters,
            "stats": {
                "unique_clusters": unique_clusters,
                "noise_points": int(np.sum(labels == -1)),
                "umap_n_neighbors": job.visualization_config.n_neighbors,
                "umap_n_components": job.visualization_config.n_components,
                "hdbscan_min_cluster_size": job.visualization_config.min_cluster_size,
            },
        }

        logger.info(
            f"Visualization processing complete: {result['point_count']} points, "
            f"{result['cluster_count']} clusters"
        )

        return result

    async def _fetch_vectors_from_qdrant(
        self, collection_name: str, owner: str
    ) -> tuple[np.ndarray, list[str], list[str]]:
        """
        Fetch all vectors from a Qdrant collection.

        Args:
            collection_name: Qdrant collection name
            owner: Owner/username for audit logging

        Returns:
            Tuple of (vectors array, point IDs, texts)

        Raises:
            Exception: If collection not found or fetch fails
        """
        fetch_start = time.time()
        try:
            # Get collection info
            logger.debug(f"Getting collection info for {collection_name}")
            collection_info = self.qdrant.get_collection(collection_name)
            point_count = collection_info.points_count
            logger.debug(f"Collection {collection_name} has {point_count} points")

            # Scroll all points
            vectors = []
            ids = []
            texts = []

            offset = None
            limit = 1000  # Batch size for scrolling
            batches = 0
            prev_offset = None

            while True:
                points, offset = self.qdrant.scroll(
                    collection_name=collection_name,
                    limit=limit,
                    offset=offset,
                    with_vectors=True,
                    with_payload=True,
                )

                # Check if we've reached the end (no points returned)
                if not points:
                    logger.debug(f"Scroll complete: no points returned")
                    break
                
                batches += 1
                logger.debug(f"Fetched batch {batches}: {len(points)} points, next offset: {offset}")

                for point in points:
                    vectors.append(point.vector)
                    ids.append(str(point.id))
                    # Try to extract text from payload
                    text = ""
                    if point.payload:
                        text = point.payload.get("text", "")
                    texts.append(text)
                
                # Check if there are more pages (offset is None means this was the last page)
                if offset is None:
                    logger.debug(f"Scroll complete: offset is None (last page processed)")
                    break
                
                # Prevent infinite loops if offset isn't changing
                if offset == prev_offset:
                    logger.warning(f"Offset not advancing: {offset}. Breaking to prevent infinite loop.")
                    break
                
                prev_offset = offset

            # Convert to numpy array
            vectors_array = np.array(vectors, dtype=np.float32)

            fetch_elapsed = time.time() - fetch_start
            
            # Validate we fetched the expected number of points
            if len(vectors) != point_count:
                logger.warning(
                    f"Point count mismatch: expected {point_count}, fetched {len(vectors)}. "
                    f"This may indicate incomplete data retrieval."
                )
            
            logger.info(
                f"Fetched {len(vectors)} vectors from {collection_name} for owner {owner} "
                f"in {fetch_elapsed:.3f}s ({batches} batches, expected {point_count})"
            )
            return vectors_array, ids, texts

        except Exception as e:
            fetch_elapsed = time.time() - fetch_start
            logger.error(
                f"Failed to fetch vectors from Qdrant in {fetch_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True
            )
            raise

    async def _apply_umap(
        self, vectors: np.ndarray, job: VisualizationTransformJob
    ) -> np.ndarray:
        """
        Apply UMAP dimensionality reduction.

        Args:
            vectors: Input vectors
            job: Visualization job with UMAP config

        Returns:
            UMAP-reduced vectors
        """
        umap_start = time.time()
        try:
            logger.debug(f"Initializing UMAP with {vectors.shape[0]} vectors")
            umap = UMAP(
                n_neighbors=job.visualization_config.n_neighbors,
                n_components=job.visualization_config.n_components,
                min_dist=job.visualization_config.min_dist,
                metric=job.visualization_config.metric,
                random_state=42,  # For reproducibility
            )

            logger.debug("Running UMAP fit_transform...")
            reduced = umap.fit_transform(vectors)
            reduced_array: np.ndarray = np.asarray(reduced, dtype=np.float32)  # type: ignore
            
            umap_elapsed = time.time() - umap_start
            logger.info(f"UMAP complete in {umap_elapsed:.3f}s: {vectors.shape} -> {reduced_array.shape}")
            return reduced_array

        except Exception as e:
            umap_elapsed = time.time() - umap_start
            logger.error(f"UMAP processing failed in {umap_elapsed:.3f}s: {type(e).__name__}: {e}", exc_info=True)
            raise

    async def _apply_hdbscan(
        self, vectors: np.ndarray, job: VisualizationTransformJob
    ) -> np.ndarray:
        """
        Apply HDBSCAN clustering.

        Args:
            vectors: Input vectors (typically UMAP output)
            job: Visualization job with HDBSCAN config

        Returns:
            Cluster labels array
        """
        hdbscan_start = time.time()
        try:
            logger.debug(f"Initializing HDBSCAN with {vectors.shape[0]} vectors")
            clusterer = HDBSCAN(
                min_cluster_size=job.visualization_config.min_cluster_size,
                min_samples=job.visualization_config.min_samples or 5,
            )

            logger.debug("Running HDBSCAN fit_predict...")
            labels = clusterer.fit_predict(vectors)
            unique_clusters = len(set(labels[labels >= 0]))
            noise_count = int(np.sum(labels == -1))

            hdbscan_elapsed = time.time() - hdbscan_start
            logger.info(
                f"HDBSCAN complete in {hdbscan_elapsed:.3f}s: {unique_clusters} clusters, "
                f"{noise_count} noise points"
            )
            return labels

        except Exception as e:
            hdbscan_elapsed = time.time() - hdbscan_start
            logger.error(f"HDBSCAN clustering failed in {hdbscan_elapsed:.3f}s: {type(e).__name__}: {e}", exc_info=True)
            raise

    async def _generate_cluster_labels(
        self,
        labels: np.ndarray,
        texts: list[str],
        job: VisualizationTransformJob,
        llm_provider: Optional[LLMProvider] = None,
    ) -> Dict[int, str]:
        """
        Generate human-readable labels for each cluster.

        If LLM config is provided and llm_provider available, use LLM naming.
        Otherwise, use simple numeric labels.

        Args:
            labels: Cluster labels from HDBSCAN
            texts: Associated text for each point
            job: Visualization job with LLM config
            llm_provider: Optional LLM provider

        Returns:
            Dictionary mapping cluster ID to label
        """
        label_start = time.time()
        try:
            cluster_labels = {}
            unique_clusters = sorted([l for l in set(labels[labels >= 0])])
            logger.debug(f"Generating labels for {len(unique_clusters)} clusters")

            # Check if we should use LLM naming
            use_llm = (
                llm_provider is not None
                and job.llm_config is not None
                and job.llm_config.api_key  # Will be falsy if empty string or None
                and len(job.llm_config.api_key.strip()) > 0  # Check for non-empty after stripping
            )
            
            # Debug logging for LLM config
            if job.llm_config:
                logger.debug(
                    f"LLM config present: provider={job.llm_config.provider}, "
                    f"model={job.llm_config.model}, "
                    f"api_key_present={bool(job.llm_config.api_key and job.llm_config.api_key.strip())}"
                )
            else:
                logger.debug("No LLM config in job")
            
            if llm_provider is None:
                logger.debug("LLM provider not initialized")

            if use_llm:
                # Type guard: at this point, both should not be None
                assert llm_provider is not None, "llm_provider should not be None"
                assert job.llm_config is not None, "job.llm_config should not be None"
                
                logger.info(
                    f"Generating cluster labels using LLM: "
                    f"{job.llm_config.provider}/{job.llm_config.model}"
                )

                for cluster_idx, cluster_id in enumerate(unique_clusters, 1):
                    cluster_indices = np.where(labels == cluster_id)[0]
                    cluster_texts = [texts[i] for i in cluster_indices if texts[i]]

                    if not cluster_texts:
                        # Empty cluster - use numeric label
                        cluster_labels[cluster_id] = f"Cluster {cluster_id}"
                        logger.debug(f"Cluster {cluster_id}: no texts, using numeric label")
                        continue

                    # Sample texts (up to 5, matching SAMPLE pattern)
                    sample_texts = cluster_texts[:5]

                    try:
                        llm_start = time.time()
                        label = await llm_provider.generate_topic_name(
                            sample_texts, job.llm_config
                        )
                        llm_elapsed = time.time() - llm_start
                        cluster_labels[cluster_id] = label
                        logger.info(f"Cluster {cluster_id} ({cluster_idx}/{len(unique_clusters)}) -> '{label}' ({llm_elapsed:.3f}s)")
                    except Exception as e:
                        logger.warning(
                            f"Failed to generate label for cluster {cluster_id}: {type(e).__name__}: {e}, "
                            f"using numeric fallback"
                        )
                        cluster_labels[cluster_id] = f"Cluster {cluster_id}"
            else:
                # Use simple numeric labels
                if job.llm_config:
                    logger.warning(
                        f"LLM config provided but not using LLM (provider_present={llm_provider is not None}, "
                        f"api_key_valid={bool(job.llm_config.api_key and len(job.llm_config.api_key.strip()) > 0)}), "
                        f"using numeric labels"
                    )
                else:
                    logger.debug("No LLM config provided, using numeric labels")

                for cluster_id in unique_clusters:
                    cluster_labels[cluster_id] = f"Cluster {cluster_id}"

            label_elapsed = time.time() - label_start
            logger.info(f"Generated labels for {len(cluster_labels)} clusters in {label_elapsed:.3f}s")
            return cluster_labels

        except Exception as e:
            label_elapsed = time.time() - label_start
            logger.error(
                f"Cluster label generation failed in {label_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True
            )
            raise

    async def _generate_visualization(
        self,
        vectors: np.ndarray,
        labels: np.ndarray,
        cluster_labels: Dict[int, str],
        texts: list[str],
        config: VisualizationConfig,
    ) -> str:
        """
        Generate interactive visualization with datamapplot.

        Args:
            vectors: reduced vectors
            labels: Cluster labels
            cluster_labels: Cluster ID to label mapping
            texts: Point descriptions
            config: Visualization configuration with datamapplot parameters

        Returns:
            HTML content of interactive visualization
        """
        viz_start = time.time()
        try:
            # Create data for visualization
            logger.debug(f"Preparing label names for {len(labels)} points")
            label_names = [cluster_labels.get(int(label), f"Cluster {label}") for label in labels]

            # Generate interactive HTML map with configurable parameters
            plot_start = time.time()
            logger.debug("Calling datamapplot.create_interactive_plot...")
            fig = datamapplot.create_interactive_plot(
                vectors,
                np.array(label_names),
                font_family=config.font_family,
                darkmode=config.darkmode,
                noise_label="Noise",
                noise_color=config.noise_color,
                label_wrap_width=config.label_wrap_width,
                use_medoids=config.use_medoids,
                cluster_boundary_polygons=config.cluster_boundary_polygons,
                polygon_alpha=config.polygon_alpha,
                min_fontsize=config.min_fontsize,
                max_fontsize=config.max_fontsize,
            )
            plot_elapsed = time.time() - plot_start
            logger.debug(f"datamapplot plot creation completed in {plot_elapsed:.3f}s")

            # Convert the interactive figure to HTML string
            html_start = time.time()
            logger.debug("Converting figure to HTML...")
            html_content = fig._repr_html_()
            html_elapsed = time.time() - html_start
            
            if html_content is None:
                logger.error("Failed to generate HTML from interactive figure")
                raise RuntimeError("Failed to generate HTML from interactive figure")
            
            viz_elapsed = time.time() - viz_start
            logger.info(
                f"Generated interactive visualization in {viz_elapsed:.3f}s "
                f"(HTML size: {len(html_content)} bytes, conversion: {html_elapsed:.3f}s)"
            )
            return html_content

        except Exception as e:
            viz_elapsed = time.time() - viz_start
            logger.error(
                f"Visualization generation failed in {viz_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True
            )
            raise


async def process_visualization_job(
    job: VisualizationTransformJob, llm_provider: Optional[LLMProvider] = None
) -> Dict[str, Any]:
    """
    Process a visualization transform job.

    This is the main entry point called from main.py.

    Args:
        job: Visualization transform job from NATS
        llm_provider: Optional LLM provider for topic naming

    Returns:
        Result dictionary with html, point_count, cluster_count, stats
    """
    processor = VisualizationProcessor(job.vector_database_config.connection_url)
    return await processor.process_job(job, llm_provider)
