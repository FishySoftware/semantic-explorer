## Overview
The API crate wires together the Actix HTTP surface, storage clients, NATS listeners, and background scanners. The structure is sound, but several choices hurt operability under load or make the binary harder to run in typical Kubernetes environments.

## Critical
- **File uploads are fully buffered in RAM** – The server sets `MultipartFormConfig::default().total_limit(max_upload_size).memory_limit(max_upload_size)` so every multipart payload up to 1 GB is read into memory without spilling to disk. See [crates/api/src/main.rs#L193-L199](crates/api/src/main.rs#L193-L199). Switch to streamed handling (e.g., `MultipartForm` streaming) and keep the in-memory limit modest to avoid OOMs and pod evictions.

## High
- **Encryption is effectively mandatory despite the “warning” log** – `EncryptionService::from_env()` returns an error when `ENCRYPTION_MASTER_KEY` is absent, and `main` propagates that error after printing guidance. See [crates/api/src/main.rs#L56-L65](crates/api/src/main.rs#L56-L65). Either supply a noop fallback so the service actually keeps running without encryption, or fail fast without suggesting it will continue.
- **S3 initialization duplicates credential plumbing and hard-requires static keys** – `storage::s3::initialize_client` rebuilds credentials with `env::var` instead of reusing `AppConfig`, and it demands `AWS_ACCESS_KEY_ID/SECRET` even when running with IAM roles. See [crates/api/src/storage/s3/mod.rs#L14-L43](crates/api/src/storage/s3/mod.rs#L14-L43). Reuse the core S3 config and let the AWS SDK resolve credentials, otherwise EKS pods will crash at startup.

## Medium
- **Result listeners ignore task failures** – `start_result_listeners` spawns four tasks and drops the handles, so the process never notices if a listener exits after an error. See [crates/api/src/transforms/listeners.rs#L108-L197](crates/api/src/transforms/listeners.rs#L108-L197). Retain the `JoinHandle`s (or use `AbortHandle`s) and surface failures via metrics or logs to avoid silent data loss.
- **SSE status publisher uses fire-and-forget semantics** – `publish_transform_status` logs warnings but does not retry or dead-letter failures, meaning transient NATS outages permanently drop UI updates. See [crates/api/src/transforms/listeners.rs#L67-L93](crates/api/src/transforms/listeners.rs#L67-L93). Consider publishing via JetStream with storage or at least add limited retries.

## Low
- **CORS defaults bake in the internal hostname** – When `CORS_ALLOWED_ORIGINS` is empty, the server allows only `http://{hostname}:{port}` from config. See [crates/api/src/main.rs#L140-L166](crates/api/src/main.rs#L140-L166). In multi-service deployments (API behind ingress, UI on another host) this requires manual overrides; defaulting to the configured `PUBLIC_URL` would smooth dev/prod parity.
