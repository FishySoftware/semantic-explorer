"""
S3 Storage Module

Handles uploading visualization results to S3 with owner/transform/timestamp tracking.
"""

import logging
import os
import time
from datetime import datetime, timezone

import boto3
from botocore.config import Config

logger = logging.getLogger(__name__)


class S3Storage:
    """S3 storage client for visualization results."""

    def __init__(self):
        """Initialize S3 client with configuration from environment."""
        init_start = time.time()
        logger.debug("Initializing S3 storage client")
        self.endpoint_url = os.getenv("AWS_ENDPOINT_URL")
        self.bucket_name = os.getenv("S3_BUCKET_NAME")
        if not self.bucket_name:
            raise ValueError("S3_BUCKET_NAME environment variable is required")

        # Configure boto3
        config = Config(
            signature_version="s3v4",
            retries={"max_attempts": 3, "mode": "adaptive"},
            connect_timeout=5,
            read_timeout=30,
        )

        # Create S3 client
        try:
            self.s3_client = boto3.client(
                "s3",
                endpoint_url=self.endpoint_url,
                config=config,
                region_name=os.getenv("AWS_REGION", "us-east-1"),
            )
            init_elapsed = time.time() - init_start
            logger.info(
                f"Initialized S3 client in {init_elapsed:.3f}s "
                f"(endpoint: {self.endpoint_url or 'default AWS'}, bucket: {self.bucket_name})"
            )
        except Exception as e:
            init_elapsed = time.time() - init_start
            logger.error(
                f"Failed to initialize S3 client in {init_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    async def upload_visualization(
        self,
        owner: str,
        transform_id: int,
        visualization_id: int,
        html_content: str,
    ) -> str:
        """
        Upload visualization HTML to S3 using single-bucket architecture.

        Args:
            owner: Owner/username
            transform_id: Visualization transform ID
            visualization_id: Visualization ID (for audit trail)
            html_content: HTML content to upload

        Returns:
            S3 key where content was stored (relative to the collection prefix)

        Raises:
            Exception: If upload fails
        """
        upload_start = time.time()
        try:
            logger.debug(
                f"Starting S3 upload for transform {transform_id}, visualization {visualization_id}"
            )

            # Generate S3 path using single-bucket architecture
            # Pattern: visualizations/{transform_id}/{filename}
            timestamp_str = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            filename = f"visualization-{timestamp_str}.html"
            s3_key = f"visualizations/{transform_id}/{filename}"
            content_size = len(html_content.encode("utf-8"))

            # Upload to S3
            logger.debug(
                f"Uploading to s3://{self.bucket_name}/{s3_key} ({content_size} bytes)"
            )
            put_start = time.time()
            self.s3_client.put_object(
                Bucket=self.bucket_name,
                Key=s3_key,
                Body=html_content.encode("utf-8"),
                ContentType="text/html; charset=utf-8",
                Metadata={
                    "owner": owner,
                    "transform-id": str(transform_id),
                    "visualization-id": str(visualization_id),
                    "timestamp": timestamp_str,
                },
            )
            put_elapsed = time.time() - put_start
            upload_elapsed = time.time() - upload_start

            logger.info(
                f"Successfully uploaded to s3://{self.bucket_name}/{s3_key} in {upload_elapsed:.3f}s "
                f"(size: {content_size} bytes, put: {put_elapsed:.3f}s)"
            )
            # Return the full S3 key - the Rust API expects the complete path
            return s3_key

        except Exception as e:
            upload_elapsed = time.time() - upload_start
            logger.error(
                f"Failed to upload visualization to S3 in {upload_elapsed:.3f}s: "
                f"{type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    async def get_visualization_url(
        self, owner: str, transform_id: int, s3_key: str, expires_in: int = 3600
    ) -> str:
        """
        Generate a presigned URL for accessing a visualization using single-bucket architecture.

        Args:
            owner: Owner/username (for validation)
            transform_id: Visualization transform ID (for validation)
            s3_key: S3 key from database (just the filename)
            expires_in: URL expiration time in seconds (default: 1 hour)

        Returns:
            Presigned URL for accessing the visualization

        Raises:
            Exception: If URL generation fails
        """
        url_start = time.time()
        try:
            # Construct full S3 key using single-bucket architecture
            full_s3_key = f"visualizations/{transform_id}/{s3_key}"
            logger.debug(f"Generating presigned URL for {self.bucket_name}/{full_s3_key}")

            # Generate presigned URL
            url_gen_start = time.time()
            url = self.s3_client.generate_presigned_url(
                "get_object",
                Params={
                    "Bucket": self.bucket_name,
                    "Key": full_s3_key,
                },
                ExpiresIn=expires_in,
            )
            url_gen_elapsed = time.time() - url_gen_start
            url_elapsed = time.time() - url_start

            logger.info(
                f"Generated presigned URL for s3://{self.bucket_name}/{full_s3_key} in {url_elapsed:.3f}s "
                f"(expires in {expires_in}s, generation: {url_gen_elapsed:.3f}s)"
            )
            return url

        except Exception as e:
            url_elapsed = time.time() - url_start
            logger.error(
                f"Failed to generate presigned URL in {url_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    async def delete_visualization(
        self, owner: str, transform_id: int, s3_key: str
    ) -> None:
        """
        Delete a visualization from S3 using single-bucket architecture.

        Args:
            owner: Owner/username (for validation)
            transform_id: Visualization transform ID (for validation)
            s3_key: S3 key to delete (just the filename)

        Raises:
            Exception: If deletion fails
        """
        delete_start = time.time()
        try:
            # Construct full S3 key using single-bucket architecture
            full_s3_key = f"visualizations/{transform_id}/{s3_key}"
            logger.debug(f"Starting deletion of s3://{self.bucket_name}/{full_s3_key}")

            logger.debug(
                f"Deleting s3://{self.bucket_name}/{full_s3_key} (owner: {owner}, transform: {transform_id})"
            )

            del_start = time.time()
            self.s3_client.delete_object(
                Bucket=self.bucket_name,
                Key=full_s3_key,
            )
            del_elapsed = time.time() - del_start
            delete_elapsed = time.time() - delete_start

            logger.info(
                f"Successfully deleted s3://{self.bucket_name}/{full_s3_key} in {delete_elapsed:.3f}s "
                f"(delete op: {del_elapsed:.3f}s)"
            )

        except Exception as e:
            delete_elapsed = time.time() - delete_start
            logger.error(
                f"Failed to delete visualization from S3 in {delete_elapsed:.3f}s: "
                f"{type(e).__name__}: {e}",
                exc_info=True,
            )
            raise
