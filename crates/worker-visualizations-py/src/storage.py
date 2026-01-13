"""
S3 Storage Module

Handles uploading visualization results to S3 with owner/transform/timestamp tracking.
"""

import logging
import os
import time
from datetime import datetime, timezone
from typing import Optional

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError

logger = logging.getLogger(__name__)


class S3Storage:
    """S3 storage client for visualization results."""

    def __init__(self):
        """Initialize S3 client with configuration from environment."""
        init_start = time.time()
        logger.debug("Initializing S3 storage client")
        self.endpoint_url = os.getenv("S3_ENDPOINT")

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
                f"(endpoint: {self.endpoint_url or 'default AWS'})"
            )
        except Exception as e:
            init_elapsed = time.time() - init_start
            logger.error(
                f"Failed to initialize S3 client in {init_elapsed:.3f}s: {type(e).__name__}: {e}",
                exc_info=True,
            )
            raise

    def _ensure_bucket_exists(self, bucket_name: str) -> None:
        """
        Ensure the S3 bucket exists, creating it if necessary.

        Args:
            bucket_name: Name of the bucket to check/create

        Raises:
            Exception: If bucket check or creation fails
        """
        try:
            # Try to head the bucket to see if it exists
            self.s3_client.head_bucket(Bucket=bucket_name)
            logger.debug(f"Bucket {bucket_name} already exists")
        except ClientError as e:
            # Check if it's a 404 (bucket doesn't exist) or 403 (no permission)
            error_code = e.response.get("Error", {}).get("Code", "")
            status_code = e.response.get("ResponseMetadata", {}).get(
                "HTTPStatusCode", 0
            )

            if status_code == 404 or error_code == "NoSuchBucket":
                # Bucket doesn't exist, create it
                logger.info(f"Bucket {bucket_name} does not exist, creating it")
                try:
                    create_start = time.time()
                    self.s3_client.create_bucket(Bucket=bucket_name)
                    create_elapsed = time.time() - create_start
                    logger.info(
                        f"Successfully created bucket {bucket_name} in {create_elapsed:.3f}s"
                    )
                except Exception as create_error:
                    logger.error(
                        f"Failed to create bucket {bucket_name}: {type(create_error).__name__}: {create_error}"
                    )
                    raise
            else:
                # Some other error (like 403 Forbidden)
                logger.error(
                    f"Error checking bucket {bucket_name}: {type(e).__name__}: {e}"
                )
                raise
        except Exception as e:
            # Catch any non-ClientError exceptions
            logger.error(
                f"Unexpected error checking bucket {bucket_name}: {type(e).__name__}: {e}"
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
        Upload visualization HTML to S3.

        Args:
            owner: Owner/username
            transform_id: Visualization transform ID
            visualization_id: Visualization ID (for audit trail)
            html_content: HTML content to upload

        Returns:
            S3 key where content was stored

        Raises:
            Exception: If upload fails
        """
        upload_start = time.time()
        try:
            logger.debug(
                f"Starting S3 upload for transform {transform_id}, visualization {visualization_id}"
            )

            # Generate S3 path
            timestamp_str = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            bucket_name = f"visualizations-{transform_id}"
            s3_path = f"visualization-{timestamp_str}.html"
            content_size = len(html_content.encode("utf-8"))

            # Ensure bucket exists before uploading
            logger.debug(f"Ensuring bucket {bucket_name} exists")
            self._ensure_bucket_exists(bucket_name)

            # Upload to S3
            logger.debug(
                f"Uploading to s3://{bucket_name}/{s3_path} ({content_size} bytes)"
            )
            put_start = time.time()
            self.s3_client.put_object(
                Bucket=bucket_name,
                Key=s3_path,
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
                f"Successfully uploaded to s3://{bucket_name}/{s3_path} in {upload_elapsed:.3f}s "
                f"(size: {content_size} bytes, put: {put_elapsed:.3f}s)"
            )
            return s3_path

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
        Generate a presigned URL for accessing a visualization.

        Args:
            owner: Owner/username (for validation)
            transform_id: Visualization transform ID (for validation)
            s3_key: S3 key from database
            expires_in: URL expiration time in seconds (default: 1 hour)

        Returns:
            Presigned URL for accessing the visualization

        Raises:
            Exception: If URL generation fails
        """
        url_start = time.time()
        try:
            logger.debug(f"Generating presigned URL for {transform_id}/{s3_key}")
            bucket_name = f"visualizations-{transform_id}"

            # Validate the s3_key contains the owner/transform
            if not s3_key.startswith(f"{owner}-{transform_id}-visualizations"):
                logger.error(
                    f"S3 key mismatch: key {s3_key} doesn't match owner {owner} / transform {transform_id}"
                )
                raise ValueError("Invalid S3 key for this owner/transform")

            # Generate presigned URL
            url_gen_start = time.time()
            url = self.s3_client.generate_presigned_url(
                "get_object",
                Params={
                    "Bucket": bucket_name,
                    "Key": s3_key,
                },
                ExpiresIn=expires_in,
            )
            url_gen_elapsed = time.time() - url_gen_start
            url_elapsed = time.time() - url_start

            logger.info(
                f"Generated presigned URL for s3://{bucket_name}/{s3_key} in {url_elapsed:.3f}s "
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
        Delete a visualization from S3.

        Args:
            owner: Owner/username (for validation)
            transform_id: Visualization transform ID (for validation)
            s3_key: S3 key to delete

        Raises:
            Exception: If deletion fails or key doesn't match owner/transform
        """
        delete_start = time.time()
        try:
            logger.debug(f"Starting deletion of s3://{transform_id}/{s3_key}")
            bucket_name = f"visualizations-{transform_id}"

            # Validate the s3_key contains the owner/transform
            if not s3_key.startswith(f"{owner}-{transform_id}-visualizations"):
                logger.error(
                    f"S3 key mismatch: key {s3_key} doesn't match owner {owner} / transform {transform_id}"
                )
                raise ValueError("Invalid S3 key for this owner/transform")

            logger.debug(
                f"Deleting s3://{bucket_name}/{s3_key} (owner: {owner}, transform: {transform_id})"
            )

            del_start = time.time()
            self.s3_client.delete_object(
                Bucket=bucket_name,
                Key=s3_key,
            )
            del_elapsed = time.time() - del_start
            delete_elapsed = time.time() - delete_start

            logger.info(
                f"Successfully deleted s3://{bucket_name}/{s3_key} in {delete_elapsed:.3f}s "
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
