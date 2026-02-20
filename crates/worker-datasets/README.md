# Datasets Worker

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Embedding generation worker for Semantic Explorer**

</div>

Processes dataset chunks, generates vector embeddings, and uploads to Qdrant.

---

## Overview

The datasets worker:

1. Consumes jobs from `DATASET_TRANSFORMS` NATS stream
2. Downloads batch files from S3 (containing text chunks)
3. Generates embeddings using configured embedder (OpenAI, Cohere, or internal)
4. Creates/updates Qdrant collections
5. Upserts vectors with metadata to Qdrant

### Performance Features

- **Qdrant client caching**: Clients cached by URL to avoid connection overhead
- **Adaptive concurrency**: Dynamically adjusts parallelism based on downstream pressure (503s)
- **Automatic retries**: Exponential backoff with sensible defaults for transient failures
- **Health endpoint**: `/healthz`, `/readyz`, `/status` for Kubernetes probes (default port `8083`)

---

## Supported Embedders

Embedder configuration comes from the job payload, not environment variables.

### OpenAI

```json
{
  "provider": "openai",
  "base_url": "https://api.openai.com/v1",
  "model": "text-embedding-3-small",
  "api_key": "sk-..."
}
```

### Cohere

```json
{
  "provider": "cohere",
  "base_url": "https://api.cohere.ai/v1",
  "model": "embed-english-v3.0",
  "api_key": "..."
}
```

### Internal (embedding-inference-api)

```json
{
  "provider": "internal",
  "base_url": "http://localhost:8090",
  "model": "BAAI/bge-small-en-v1.5"
}
```

---

## Environment Variables

### Worker-Specific

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVICE_NAME` | `worker-datasets` | Service name for telemetry |
| `NATS_URL` | `nats://localhost:4222` | NATS server URL |
| `MAX_CONCURRENT_JOBS` | `10` | Concurrent job limit |
| `HEALTH_CHECK_PORT` | `8083` | Health check HTTP server port |
| `EMBEDDING_INFERENCE_API_URL` | `http://localhost:8090` | Internal embedding API URL |
| `EMBEDDING_MAX_CONCURRENT_REQUESTS` | `3` | Max concurrent embedding requests |
| `QDRANT_PARALLEL_UPLOADS` | `4` | Parallel Qdrant upload tasks |

### S3 Storage (from core)

| Variable | Required | Description |
|----------|----------|-------------|
| `AWS_REGION` | Yes | S3 region |
| `AWS_ENDPOINT_URL` | Yes | S3 endpoint URL |
| `AWS_ACCESS_KEY_ID` | No* | S3 access key |
| `AWS_SECRET_ACCESS_KEY` | No* | S3 secret key |
| `S3_FORCE_PATH_STYLE` | No | Use path-style URLs (for MinIO) |

*Uses AWS default credential chain if not set

### Observability

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP collector |
| `RUST_LOG` | `info` | Log level |

> **Note:** NATS consumer tuning, circuit breaker, and retry policy parameters are hardcoded with
> production-tested defaults. The worker automatically adapts its concurrency based on downstream
> service health (503 backpressure).

---

## Job Payload

Jobs from the `DATASET_TRANSFORMS` NATS stream:

```json
{
  "job_id": "uuid",
  "dataset_transform_id": 123,
  "embedded_dataset_id": 456,
  "bucket": "semantic-explorer",
  "batch_file_key": "transforms/batch-001.json",
  "collection_name": "my-collection",
  "batch_size": 100,
  "embedder_config": {
    "provider": "openai",
    "base_url": "https://api.openai.com/v1",
    "model": "text-embedding-3-small",
    "api_key": "sk-..."
  },
  "qdrant_config": {
    "url": "http://localhost:6334",
    "api_key": null
  }
}
```

### Batch File Format

The `batch_file_key` points to a JSON file in S3:

```json
[
  {
    "id": "chunk-uuid-1",
    "text": "Chunk text content...",
    "payload": {
      "source_file": "document.pdf",
      "page": 1,
      "chunk_index": 0
    }
  }
]
```

---

## Qdrant Integration

The worker automatically:

- Creates collections if they don't exist
- Configures cosine distance metric
- Enables on-disk storage for large collections
- Upserts points in chunks of 1000 to avoid overwhelming Qdrant
- Enables on-disk storage for vectors and payloads

---

## Building

```bash
# Debug build
cargo build -p worker-datasets

# Release build
cargo build -p worker-datasets --release
```

### Docker

```bash
docker build -f crates/worker-datasets/Dockerfile -t worker-datasets:latest .
```

---

## Running

```bash
export AWS_REGION=us-east-1
export AWS_ENDPOINT_URL=http://localhost:9000
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
export NATS_URL=nats://localhost:4222

cargo run -p worker-datasets
```

---

## Metrics

Worker metrics via `record_worker_job`:

| Metric | Labels | Description |
|--------|--------|-------------|
| `worker_job_duration_seconds` | `job_type`, `status` | Job processing time |
| `worker_job_total` | `job_type`, `status` | Job count |

Status values: `success`, `success_empty`, `failed_validation`, `failed_download`, `failed_parse`, `failed_embedding`, `failed_mismatch`, `failed_qdrant`

---

## License

Apache License 2.0
