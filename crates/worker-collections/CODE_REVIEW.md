## Overview
The collections worker handles file extraction, chunking, and chunk upload back to S3. The happy path works, but several choices limit throughput and add duplication compared to the dataset worker.

## High
- **Jobs ignore the bucket supplied by control-plane** – `CollectionTransformJob` carries a `bucket` field, yet `process_file_job` reads `S3_BUCKET_NAME` from the environment and bails if it is missing. See [crates/worker-collections/src/job.rs#L25-L41](crates/worker-collections/src/job.rs#L25-L41) and [crates/core/src/models.rs#L5-L17](crates/core/src/models.rs#L5-L17). This breaks multi-bucket deployments and forces every worker pod to share the same env configuration. Use the bucket coming from the job payload.
- **Semantic chunking reimplements an embedder client** – `strategies::semantic::chunk_async` spins up a fresh `reqwest::Client` for every call and re-creates provider-specific payloads. Compare [crates/worker-collections/src/chunk/strategies/semantic.rs#L125-L291](crates/worker-collections/src/chunk/strategies/semantic.rs#L125-L291) with the dataset worker’s shared embedder helper. Extract this logic into a shared module (likely under `crates/core`) so retries, auth, and batching stay uniform.

## Medium
- **Chunking clones large strings unnecessarily** – `ChunkingService::chunk_text` clones the full `text` multiple times (semantic branch included) and rebuilds vectors before applying overlap. See [crates/worker-collections/src/chunk/service.rs#L18-L103](crates/worker-collections/src/chunk/service.rs#L18-L103). Refactoring to work with slices or streaming iterators would reduce memory spikes on 100 MB+ documents.
- **Uploading a single JSON blob limits recoverability** – The worker serializes every chunk into one JSON file before uploading (see [crates/worker-collections/src/job.rs#L250-L274](crates/worker-collections/src/job.rs#L250-L274)). Consider chunked uploads or partitioning to allow retries if S3 rejects large payloads.

## Low
- **Env var validation returns `Ok(())` but still counts as success** – In the early validation branches (e.g., [crates/worker-collections/src/job.rs#L43-L73](crates/worker-collections/src/job.rs#L43-L73)) the worker reports success to JetStream even though nothing was done. Returning an error instead would surface the misconfiguration to the DLQ automatically.
