"""
Visualization Processing Pipeline

Handles UMAP dimensionality reduction, HDBSCAN clustering, and datamapplot
interactive visualization generation.
"""

import asyncio
import logging
import time
import tempfile
import os
from typing import Any, Dict, Optional

import numpy as np
import random
from qdrant_client import AsyncQdrantClient
from qdrant_client.http import models as RestModels
from umap import UMAP
from fast_hdbscan import HDBSCAN
import datamapplot

# Maximum points to visualize to prevent OOM
MAX_POINTS = int(os.environ.get("MAX_VISUALIZATION_POINTS", 100_000_000))

try:
    # Try relative imports (for package execution)
    from .models import VisualizationTransformJob, VisualizationConfig
    from .llm_namer import LLMProvider
    from .font_patcher import patch_html_fonts
except ImportError:
    # Fallback to absolute imports (for direct script execution)
    from models import VisualizationTransformJob, VisualizationConfig
    from llm_namer import LLMProvider
    from font_patcher import patch_html_fonts

logger = logging.getLogger(__name__)


class VisualizationProcessor:
    """Processes vectors and generates interactive visualizations."""

    def __init__(self, qdrant_url: str):
        """Initialize processor with Qdrant connection using gRPC."""
        init_start = time.time()
        # Convert HTTP URL to gRPC URL
        grpc_url = qdrant_url
        logger.debug(f"Converting Qdrant URL: {qdrant_url} -> {grpc_url}")
        self.qdrant = AsyncQdrantClient(url=grpc_url, prefer_grpc=True)
        init_elapsed = time.time() - init_start
        logger.info(f"Initialized Async Qdrant client in {init_elapsed:.3f}s: {grpc_url}")


    async def process_job(
        self,
        job: VisualizationTransformJob,
        llm_provider: Optional[LLMProvider] = None,
        progress_callback=None,
    ) -> Dict[str, Any]:
        """
        Process visualization job end-to-end.

        Args:
            job: Visualization transform job
            llm_provider: Optional LLM provider for topic naming
            progress_callback: Optional async callback(stage: str, progress: int) for progress updates

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

        # Fetch vectors from Qdrant (0-20%)
        if progress_callback:
            await progress_callback("fetching_vectors", 5)
        logger.info(
            f"Fetching vectors from Qdrant collection: {job.qdrant_collection_name}"
        )
        vectors, _ids, texts = await self._fetch_vectors_from_qdrant(
            job.qdrant_collection_name, job.owner_id
        )
        if progress_callback:
            await progress_callback("fetching_vectors", 20)

        logger.info(f"Fetched {len(vectors)} vectors")

        # Get current event loop for running CPU-bound tasks
        loop = asyncio.get_running_loop()

        # Apply UMAP dimensionality reduction (20-50%)
        if progress_callback:
            await progress_callback("applying_umap", 25)
        logger.info(
            f"Applying UMAP: n_neighbors={job.visualization_config.n_neighbors}, "
            f"min_dist={job.visualization_config.min_dist}, "
            f"metric={job.visualization_config.metric}"
        )
        # Run UMAP in executor to avoid blocking the event loop
        umap_vectors = await loop.run_in_executor(None, self._run_umap, vectors, job)
        if progress_callback:
            await progress_callback("applying_umap", 50)

        # Apply HDBSCAN clustering (50-70%)
        if progress_callback:
            await progress_callback("clustering", 55)
        logger.info(
            f"Applying HDBSCAN: min_cluster_size={job.visualization_config.min_cluster_size}, "
            f"min_samples={job.visualization_config.min_samples}"
        )
        # Run HDBSCAN in executor
        labels = await loop.run_in_executor(None, self._run_hdbscan, umap_vectors, job)
        if progress_callback:
            await progress_callback("clustering", 70)

        # Generate cluster labels/names (70-85%)
        if progress_callback:
            await progress_callback("naming_clusters", 72)
        cluster_labels = await self._generate_cluster_labels(
            labels, texts, job, llm_provider
        )
        if progress_callback:
            await progress_callback("naming_clusters", 85)

        # Generate interactive visualization (85-100%)
        if progress_callback:
            await progress_callback("generating_html", 88)
        logger.info("Generating interactive visualization with datamapplot")
        # datamapplot can be CPU intensive too, run in executor
        html_content = await loop.run_in_executor(
            None, 
            self._run_generate_visualization,
            umap_vectors, labels, cluster_labels, texts, job.visualization_config
        )
        if progress_callback:
            await progress_callback("generating_html", 100)

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
                "hdbscan_min_cluster_size": job.visualization_config.min_cluster_size,
            },
        }

        logger.info(
            f"Visualization processing complete: {result['point_count']} points, "
            f"{result['cluster_count']} clusters"
        )
        return result

    def _run_umap(self, vectors, job):
        """Synchronous wrapper for UMAP."""
        return self._apply_umap_sync(vectors, job)
    
    def _run_hdbscan(self, vectors, job):
        """Synchronous wrapper for HDBSCAN."""
        return self._apply_hdbscan_sync(vectors, job)

    def _run_generate_visualization(self, vectors, labels, cluster_labels, texts, config):
        """Synchronous wrapper for visualization generation."""
        return self._generate_visualization_sync(vectors, labels, cluster_labels, texts, config)

    async def _fetch_vectors_from_qdrant(
        self, collection_name: str, owner: str
    ) -> tuple[np.ndarray, list[str], list[str]]:
        """
        Fetch vectors from a Qdrant collection, with sampling if too large.

        Args:
            collection_name: Qdrant collection name
            owner: Owner/username for audit logging

        Returns:
            Tuple of (vectors array, point IDs, hover_texts)
        """
        fetch_start = time.time()
        try:
            # Get collection info
            logger.debug(f"Getting collection info for {collection_name}")
            collection_info = await self.qdrant.get_collection(collection_name)
            point_count = collection_info.points_count
            logger.debug(f"Collection {collection_name} has {point_count} points")

            vectors = []
            ids = []
            texts = []

            # If collection is small enough, fetch all
            if point_count is not None and point_count <= MAX_POINTS:
                logger.info(f"Collection size {point_count} <= {MAX_POINTS}. Fetching all points.")
                offset = None
                limit = 1000  # Batch size for scrolling
                batches = 0
                prev_offset = None

                while True:
                    points, offset = await self.qdrant.scroll(
                        collection_name=collection_name,
                        limit=limit,
                        offset=offset,
                        with_vectors=True,
                        with_payload=True,
                    )

                    if not points:
                        break

                    batches += 1
                    for point in points:
                        vectors.append(point.vector)
                        ids.append(str(point.id))
                        texts.append(self._extract_hover_text(point.payload))

                    if offset is None or offset == prev_offset:
                        break
                    prev_offset = offset

            else:
                # Collection too large: Fetch all IDs first, then sample, then fetch details
                logger.info(f"Collection size {point_count} > {MAX_POINTS}. Sampling {MAX_POINTS} random points.")
                
                # Fetch all IDs (lightweight)
                all_ids = []
                offset = None
                limit = 5000 # Larger batch for IDs
                prev_offset = None
                
                while True:
                    points, offset = await self.qdrant.scroll(
                        collection_name=collection_name,
                        limit=limit,
                        offset=offset,
                        with_vectors=False,
                        with_payload=False,
                    )
                    
                    if not points:
                        break
                        
                    for point in points:
                        all_ids.append(point.id)
                        
                    if offset is None or offset == prev_offset:
                        break
                    prev_offset = offset
                    
                # Sample IDs
                if len(all_ids) > MAX_POINTS:
                    sampled_ids = random.sample(all_ids, MAX_POINTS)
                else:
                    sampled_ids = all_ids
                    
                logger.info(f"Sampled {len(sampled_ids)} IDs. Fetching vectors...")
                
                # Retrieve sampled points in batches
                batch_size = 500
                for i in range(0, len(sampled_ids), batch_size):
                    batch_ids = sampled_ids[i:i+batch_size]
                    points = await self.qdrant.retrieve(
                        collection_name=collection_name,
                        ids=batch_ids,
                        with_vectors=True,
                        with_payload=True
                    )
                    
                    for point in points:
                        vectors.append(point.vector)
                        ids.append(str(point.id))
                        texts.append(self._extract_hover_text(point.payload))

            # Convert to numpy array
            vectors_array = np.array(vectors, dtype=np.float32)

            fetch_elapsed = time.time() - fetch_start

            logger.info(
                f"Fetched {len(vectors)} vectors from {collection_name} for owner {owner} "
                f"in {fetch_elapsed:.3f}s"
            )
            return vectors_array, ids, texts

        except Exception as e:
            fetch_elapsed = time.time() - fetch_start
            logger.error(
                f"Failed to fetch vectors from Qdrant in {fetch_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    def _extract_hover_text(self, payload):
        """Helper to extract hover text from payload."""
        hover_text = ""
        if payload:
            title = payload.get("item_title", "")
            text = payload.get("text", "")
            if title and text:
                hover_text = f"{title}\n\n{text}"
            elif title:
                hover_text = title
            else:
                hover_text = text
        return hover_text

    def _apply_umap_sync(
        self, vectors: np.ndarray, job: VisualizationTransformJob
    ) -> np.ndarray:
        """Sync version of apply_umap for executor."""

        umap_start = time.time()
        try:
            logger.debug(f"Initializing UMAP with {vectors.shape[0]} vectors")
            umap = UMAP(
                n_neighbors=job.visualization_config.n_neighbors,
                n_components=2,
                min_dist=job.visualization_config.min_dist,
                metric=job.visualization_config.metric,
                random_state=42,  # For reproducibility
            )

            logger.debug("Running UMAP fit_transform...")
            reduced = umap.fit_transform(vectors)
            reduced_array: np.ndarray = np.asarray(reduced, dtype=np.float32)  # type: ignore

            umap_elapsed = time.time() - umap_start
            logger.info(
                f"UMAP complete in {umap_elapsed:.3f}s: {vectors.shape} -> {reduced_array.shape}"
            )
            return reduced_array

        except Exception as e:
            umap_elapsed = time.time() - umap_start
            logger.error(
                f"UMAP processing failed in {umap_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    def _apply_hdbscan_sync(
        self, vectors: np.ndarray, job: VisualizationTransformJob
    ) -> np.ndarray:
        """
        Apply HDBSCAN clustering (Synchronous).

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
            logger.error(
                f"HDBSCAN clustering failed in {hdbscan_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
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

        Note: Cluster -1 (noise) is intentionally excluded from labeling.

        Args:
            labels: Cluster labels from HDBSCAN
            texts: Associated text for each point
            job: Visualization job with LLM config
            llm_provider: Optional LLM provider

        Returns:
            Dictionary mapping cluster ID to label (excludes cluster -1)
        """
        label_start = time.time()
        try:
            cluster_labels = {}
            # Filter out cluster -1 (noise points) - they should not have labels
            unique_clusters = sorted(
                [cluster_id for cluster_id in set(labels) if cluster_id >= 0]
            )
            logger.debug(
                f"Generating labels for {len(unique_clusters)} clusters (excluding noise cluster -1)"
            )

            # Check if we should use LLM naming
            # For "internal" provider, API key is not required (uses internal inference API)
            is_internal_provider = (
                job.llm_config is not None
                and job.llm_config.provider.lower() == "internal"
            )
            has_valid_api_key = (
                job.llm_config is not None
                and job.llm_config.api_key
                and len(job.llm_config.api_key.strip()) > 0
            )
            use_llm = (
                llm_provider is not None
                and job.llm_config is not None
                and (is_internal_provider or has_valid_api_key)
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
                    f"{job.llm_config.provider}/{job.llm_config.model} "
                    f"(batch_size={job.visualization_config.llm_batch_size}, "
                    f"samples_per_cluster={job.visualization_config.samples_per_cluster})"
                )

                # Process clusters in batches for efficiency
                batch_size = max(1, min(100, job.visualization_config.llm_batch_size))
                samples_per_cluster = max(
                    1, min(100, job.visualization_config.samples_per_cluster)
                )

                for batch_start in range(0, len(unique_clusters), batch_size):
                    batch_end = min(batch_start + batch_size, len(unique_clusters))
                    batch_clusters = unique_clusters[batch_start:batch_end]

                    logger.debug(
                        f"Processing cluster batch {batch_start//batch_size + 1}: "
                        f"clusters {batch_start+1}-{batch_end} of {len(unique_clusters)}"
                    )

                    # Create tasks for parallel LLM requests
                    tasks = []
                    for cluster_id in batch_clusters:
                        cluster_indices = np.where(labels == cluster_id)[0]
                        cluster_texts = [texts[i] for i in cluster_indices if texts[i]]

                        if not cluster_texts:
                            # Empty cluster - use numeric label
                            cluster_labels[cluster_id] = f"Cluster {cluster_id}"
                            logger.debug(
                                f"Cluster {cluster_id}: no texts, using numeric label"
                            )
                            continue

                        # Sample texts based on configuration
                        sample_texts = cluster_texts[:samples_per_cluster]
                        tasks.append(
                            (
                                cluster_id,
                                llm_provider.generate_topic_name(
                                    sample_texts, job.llm_config
                                ),
                            )
                        )

                    # Execute batch in parallel
                    if tasks:
                        batch_start_time = time.time()
                        results = await asyncio.gather(
                            *[task for _, task in tasks], return_exceptions=True
                        )
                        batch_elapsed = time.time() - batch_start_time

                        # Process results
                        for (cluster_id, _), result in zip(tasks, results):
                            if isinstance(result, Exception):
                                logger.warning(
                                    f"Failed to generate label for cluster {cluster_id}: {type(result).__name__}: {result}, "
                                    f"using numeric fallback"
                                )
                                cluster_labels[cluster_id] = f"Cluster {cluster_id}"
                            else:
                                cluster_labels[cluster_id] = result
                                logger.debug(f"Cluster {cluster_id} -> '{result}'")

                        logger.info(
                            f"Batch {batch_start//batch_size + 1} complete: {len(tasks)} clusters in {batch_elapsed:.3f}s "
                            f"({batch_elapsed/len(tasks):.3f}s per cluster)"
                        )
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
            logger.info(
                f"Generated labels for {len(cluster_labels)} clusters in {label_elapsed:.3f}s"
            )
            return cluster_labels

        except Exception as e:
            label_elapsed = time.time() - label_start
            logger.error(
                f"Cluster label generation failed in {label_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    def _generate_visualization_sync(
        self,
        vectors: np.ndarray,
        labels: np.ndarray,
        cluster_labels: Dict[int, str],
        texts: list[str],
        config: VisualizationConfig,
    ) -> str:
        """
        Generate interactive visualization with datamapplot (Synchronous).

        Args:
            vectors: reduced vectors
            labels: Cluster labels
            cluster_labels: Cluster ID to label mapping
            texts: Point descriptions
            config: Visualization configuration with all datamapplot parameters

        Returns:
            HTML content of interactive visualization
        """
        viz_start = time.time()
        try:
            # Create data for visualization
            logger.debug(f"Preparing label names for {len(labels)} points")
            label_names = [
                (
                    config.noise_label
                    if int(label) == -1
                    else cluster_labels.get(int(label), f"Cluster {label}")
                )
                for label in labels
            ]

            # Build kwargs for create_interactive_plot
            plot_kwargs = {
                # Direct create_interactive_plot parameters
                "inline_data": config.inline_data,
                "noise_label": config.noise_label,
                "noise_color": config.noise_color,
                "color_label_text": config.color_label_text,
                "label_wrap_width": config.label_wrap_width,
                "width": config.width,
                "height": config.height,
                "darkmode": config.darkmode,
                "palette_hue_shift": config.palette_hue_shift,
                "palette_hue_radius_dependence": config.palette_hue_radius_dependence,
                "palette_theta_range": config.palette_theta_range,
                "use_medoids": config.use_medoids,
                "cluster_boundary_polygons": config.cluster_boundary_polygons,
                "polygon_alpha": config.polygon_alpha,
                "cvd_safer": config.cvd_safer,
                "enable_topic_tree": config.enable_topic_tree,
                # render_html parameters (passed through **render_html_kwds)
                "title_font_size": config.title_font_size,
                "sub_title_font_size": config.sub_title_font_size,
                "text_collision_size_scale": config.text_collision_size_scale,
                "text_min_pixel_size": config.text_min_pixel_size,
                "text_max_pixel_size": config.text_max_pixel_size,
                "font_family": config.font_family,
                "font_weight": config.font_weight,
                "tooltip_font_family": config.tooltip_font_family,
                "tooltip_font_weight": config.tooltip_font_weight,
                "logo_width": config.logo_width,
                "line_spacing": config.line_spacing,
                "min_fontsize": config.min_fontsize,
                "max_fontsize": config.max_fontsize,
                "text_outline_width": config.text_outline_width,
                "text_outline_color": config.text_outline_color,
                "point_hover_color": config.point_hover_color,
                "point_radius_min_pixels": config.point_radius_min_pixels,
                "point_radius_max_pixels": config.point_radius_max_pixels,
                "point_line_width_min_pixels": config.point_line_width_min_pixels,
                "point_line_width_max_pixels": config.point_line_width_max_pixels,
                "point_line_width": config.point_line_width,
                "cluster_boundary_line_width": config.cluster_boundary_line_width,
                "initial_zoom_fraction": config.initial_zoom_fraction,
            }

            # Add optional parameters if they're set
            if config.title is not None:
                plot_kwargs["title"] = config.title
            if config.sub_title is not None:
                plot_kwargs["sub_title"] = config.sub_title
            if config.logo is not None:
                plot_kwargs["logo"] = config.logo
            if config.point_size_scale is not None:
                plot_kwargs["point_size_scale"] = config.point_size_scale
            if config.background_color is not None:
                plot_kwargs["background_color"] = config.background_color
            if config.background_image is not None:
                plot_kwargs["background_image"] = config.background_image

            # Generate interactive HTML map with all configurable parameters
            plot_start = time.time()
            logger.debug("Calling datamapplot.create_interactive_plot...")
            fig = datamapplot.create_interactive_plot(
                vectors,
                np.array(label_names),
                hover_text=texts if texts else None,
                **plot_kwargs,
            )
            plot_elapsed = time.time() - plot_start
            logger.debug(f"datamapplot plot creation completed in {plot_elapsed:.3f}s")

            # Convert the interactive figure to HTML string using temporary file
            html_start = time.time()
            logger.debug("Converting figure to HTML...")

            # Create a temporary file to save the HTML
            with tempfile.NamedTemporaryFile(
                mode="w+", suffix=".html", delete=False, encoding="utf-8"
            ) as tmp_file:
                tmp_path = tmp_file.name

            try:
                # Save the figure to the temporary file
                fig.save(tmp_path)

                # Read the HTML content
                with open(tmp_path, "r", encoding="utf-8") as f:
                    html_content = f.read()
            finally:
                # Clean up the temporary file
                if os.path.exists(tmp_path):
                    os.unlink(tmp_path)

            html_elapsed = time.time() - html_start

            if html_content is None or len(html_content) == 0:
                logger.error("Failed to generate HTML from interactive figure")
                raise RuntimeError("Failed to generate HTML from interactive figure")

            # Patch HTML to use local embedded fonts instead of Google Fonts
            logger.debug("Patching HTML to use local embedded fonts...")
            html_content = patch_html_fonts(html_content)

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
                exc_info=True,
            )
            raise


async def process_visualization_job(
    job: VisualizationTransformJob,
    llm_provider: Optional[LLMProvider] = None,
    progress_callback=None,
) -> Dict[str, Any]:
    """
    Process a visualization transform job.

    This is the main entry point called from main.py.

    Args:
        job: Visualization transform job from NATS
        llm_provider: Optional LLM provider for topic naming
        progress_callback: Optional async callback(stage: str, progress: int) for progress updates

    Returns:
        Result dictionary with html, point_count, cluster_count, stats
    """
    processor = VisualizationProcessor(job.vector_database_config.connection_url)
    return await processor.process_job(job, llm_provider, progress_callback)
