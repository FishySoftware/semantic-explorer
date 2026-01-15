# Worker Datasets - Vector Embedding Generation

Background worker service for generating vector embeddings from document chunks and storing them in Qdrant for semantic search. Handles batch processing, API cost optimization, and quantized embedding storage.

## 📋 Overview

The `worker-datasets` service processes document chunks via NATS JetStream, generates vector embeddings using multiple embedder APIs, and stores them in Qdrant for semantic search.

### Responsibilities
- 📥 Subscribe to dataset transform jobs from NATS JetStream
- 📥 Download chunk batches from S3 storage
- 🔤 Generate embeddings via API providers (OpenAI, Cohere, local models)
- 🎯 Create and manage Qdrant vector collections
- 💾 Store embeddings with metadata in Qdrant
- 📊 Apply quantization for 10x faster search
- 🔄 Handle request deduplication via NATS JetStream
- 💰 Track API costs and usage metrics
- 📝 Publish embedding results to API and downstream workers

### Supported Embedders
- **OpenAI** - text-embedding-3-small, text-embedding-3-large
- **Cohere** - embed-english-v3.0, embed-english-light-v3.0  
- **Local Models** - ONNX models via ort
- **Custom** - Plugin architecture for additional providers

## 🏗️ Module Structure

### Embedder Module (`embedder.rs`)

Core embedding generation logic:

```rust
pub enum EmbedderProvider {
    OpenAI { model: String },
    Cohere { model: String },
    Local { model_path: String },
}

pub struct EmbedderClient {
    provider: EmbedderProvider,
    batch_size: usize,
    deduplication: NATS JetStream,
}

impl EmbedderClient {
    // Generate embeddings for batch of texts
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Embedding>>;
    
    // Get cost for embeddings
    pub async fn calculate_cost(&self, token_count: u64) -> Decimal;
}
```

**OpenAI Embeddings:**
```rust
// text-embedding-3-small (512 dims, $0.02 per 1M tokens)
// text-embedding-3-large (3072 dims, $0.13 per 1M tokens)

// Batching: 100 texts per request
// Retry: Exponential backoff up to 3 times
// Cost tracking: Per token calculation
```

**Cohere Embeddings:**
```rust
// embed-english-v3.0 (1024 dims, $0.10 per 1M tokens)
// embed-english-light-v3.0 (384 dims, $0.03 per 1M tokens)

// Batching: 96 texts per request (API limit)
// Truncation: Handles long texts gracefully
// Cost tracking: Per token calculation
```

**Quantization:**
```rust
// Product Quantization (PQ):
// - Compress 1536 dims to 256 bytes
// - 10x memory reduction
// - Minimal accuracy loss (< 2%)
// - 10x faster nearest-neighbor search

// Scalar Quantization:
// - Compress to 8-bit integers
// - 4x memory reduction
// - Better accuracy than PQ

// No Quantization:
// - Full precision storage
// - Maximum accuracy
// - 4x more storage
```

## 🚀 Getting Started

### Prerequisites
- Rust 1.85+
- PostgreSQL 14+ (configuration, cost tracking)
- NATS 2.10+ (job queue)
- S3-compatible storage
- Qdrant 1.8+ (vector storage)
- NATS JetStream with duplicate_window)
- API keys (OpenAI, Cohere, or local model)

### Local Development

```bash
# Copy environment
cp .env.example .env

# Configure:
# NATS_SERVER_URL=nats://localhost:4222
# DATABASE_URL=postgresql://user:pass@localhost:5432/db
# QDRANT_URL=http://localhost:6334
# EMBEDDER_PROVIDER=openai  # or cohere, local
# OPENAI_API_KEY=sk-...
# # Deduplication handled by NATS JetStream duplicate_window

# Run migrations
cd ../api
sqlx migrate run --database-url "$DATABASE_URL"

# Start worker
cd ../worker-datasets
cargo run
```

### Docker

```bash
docker run \
  -e NATS_SERVER_URL="nats://nats:4222" \
  -e QDRANT_URL="http://qdrant:6334" \
  -e EMBEDDER_PROVIDER="openai" \
  -e OPENAI_API_KEY="sk-..." \
  semantic-explorer-worker-datasets
```

## 📊 Job Processing Flow

```
1. Listen on NATS: "dataset.transform.pending"

2. Receive job:
   {
     "job_id": "uuid",
     "dataset_id": "uuid",
     "chunks": [
       {"id": "uuid", "text": "Lorem ipsum...", "metadata": {...}}
     ],
     "embedder": {
       "provider": "openai",
       "model": "text-embedding-3-small"
     },
     "quantization": "product"
   }

3. Message deduplication via NATS Msg-Id (idempotency)
   GET request-dedup:{hash}  → Cache hit = skip

4. Extract texts and create batches
   [text1, text2, ...] grouped by 100

5. Call embedder API
   POST https://api.openai.com/v1/embeddings
   → [embedding1, embedding2, ...]

6. Apply quantization
   1536-dim → 256 bytes (product quantization)

7. Upsert to Qdrant
   PUT /collections/dataset-123/points
   {
     "id": 1,
     "vector": [compressed_vector],
     "payload": {
       "chunk_id": "uuid",
       "text": "Lorem ipsum...",
       "metadata": {...}
     }
   }

8. Track costs and metrics
   INSERT INTO embedding_costs
   UPDATE metrics: embeddings_generated_total

9. Publish completion
   NATS publish "dataset.transform.complete"
```

## 🔧 Configuration

### Environment Variables

```bash
# NATS JetStream Configuration
NATS_URL=nats://localhost:4222

# Qdrant Vector Database
QDRANT_URL=http://localhost:6334
QDRANT_API_KEY=              # Optional
QDRANT_QUANTIZATION_TYPE=product  # product, scalar, none
QDRANT_TIMEOUT_SECS=30
QDRANT_CONNECT_TIMEOUT_SECS=10

# AWS S3 Configuration
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your-key
AWS_SECRET_ACCESS_KEY=your-secret
AWS_ENDPOINT_URL=http://localhost:9000
S3_BUCKET_NAME=semantic-explorer-files

# Embedder Provider Configuration
# Choose one provider and set corresponding API key

# OpenAI Embeddings (recommended)
OPENAI_API_KEY=sk-...

# Cohere Embeddings
COHERE_API_KEY=...

# NATS provides message deduplication
# Deduplication handled by NATS JetStream duplicate_window

# Worker Configuration
SERVICE_NAME=worker-datasets     # Optional, defaults to worker-datasets
MAX_CONCURRENT_JOBS=10          # Number of jobs to process concurrently

# File Processing
MAX_FILE_SIZE_MB=100            # Maximum file size to process

# Observability
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
LOG_FORMAT=json
RUST_LOG=worker_datasets=debug
```

## 💰 Cost Tracking

The worker tracks API call costs for billing and optimization:

```sql
-- Cost tracking table
CREATE TABLE embedding_costs (
    id UUID PRIMARY KEY,
    dataset_id UUID,
    embedder_provider VARCHAR,
    embedder_model VARCHAR,
    token_count INT,
    cost_usd DECIMAL,
    created_at TIMESTAMP
);

-- Query monthly costs
SELECT 
    embedder_model,
    SUM(token_count) as total_tokens,
    SUM(cost_usd) as total_cost
FROM embedding_costs
WHERE created_at >= NOW() - INTERVAL '30 days'
GROUP BY embedder_model;
```

### Cost per Provider (2024 prices)
```
OpenAI text-embedding-3-small: $0.02 / 1M tokens
OpenAI text-embedding-3-large: $0.13 / 1M tokens  
Cohere embed-english-v3.0: $0.10 / 1M tokens
Cohere embed-light-v3.0: $0.03 / 1M tokens
Local Model: $0 (one-time cost)
```

## 📊 Metrics

Prometheus metrics exported on port 8002:

```
# Embedding metrics
worker_embeddings_generated_total{provider,model}
worker_embedding_latency_seconds{provider,model}
worker_embedding_batch_size_avg{provider}
worker_embedding_cache_hits_total
worker_embedding_cache_misses_total

# Cost metrics
worker_embedding_cost_usd_total{provider,model}
worker_embedding_tokens_total{provider,model}

# API metrics
worker_api_requests_total{provider,status}
worker_api_latency_seconds{provider}
worker_api_errors_total{provider,error_type}

# Qdrant metrics
worker_qdrant_upsert_total
worker_qdrant_upsert_latency_seconds
worker_qdrant_vector_count{collection}

# Queue metrics
worker_jobs_processed_total
worker_job_duration_seconds
```

## 🧪 Testing

```bash
# Unit tests
cargo test --lib

# Integration tests (requires services)
cargo test --test '*'

# Test with mock embedder
EMBEDDER_PROVIDER=local cargo test

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

## 📈 Performance Optimization

### Batching
```bash
# Larger batches reduce API calls but increase latency
EMBEDDER_BATCH_SIZE=100  # Default: good balance

# For low-latency: EMBEDDER_BATCH_SIZE=10
# For cost optimization: EMBEDDER_BATCH_SIZE=500
```

### Quantization Impact
```
No Quantization:
  - Vector size: 1536 dims × 4 bytes = 6.1 KB
  - Search time: ~50ms per query
  - Memory: 6.1 KB × 1M vectors = 6 TB

Product Quantization:
  - Vector size: 256 bytes
  - Search time: ~5ms per query (10x faster!)
  - Memory: 256 bytes × 1M vectors = 256 GB (24x reduction)
  - Accuracy loss: < 2%
```

### Caching
```bash
# Request deduplication via NATS JetStream
# Reduces duplicate API calls
REQUEST_DEDUP_TTL_SECS=3600  # Cache for 1 hour
ENABLE_EMBEDDING_CACHE=true

# Hit rate: 30-40% for repeated datasets
```

### Scaling
```bash
# Horizontal: Deploy multiple worker instances
# Vertical: Increase NATS_BATCH_SIZE
# Cost: Choose cheaper embedder models (light variants)
```

## 🔐 Security

1. **API Key Management** - Store keys in secrets manager
2. **Cost Limits** - Set monthly budget alerts
3. **Input Validation** - Sanitize texts before embedding
4. **Request Deduplication** - Prevent accidental duplicate calls
5. **Audit Logging** - Track all embedding operations

## 🐛 Debugging

### Monitor queue
```bash
nats stream info DATASET_TRANSFORMS
nats consumer info DATASET_TRANSFORMS worker-datasets
```

### Check costs
```sql
SELECT SUM(cost_usd) FROM embedding_costs 
WHERE created_at > NOW() - INTERVAL '24 hours';
```

### Enable verbose logging
```bash
RUST_LOG=semantic_explorer_worker_datasets=debug cargo run
```

### Test embedder connection
```bash
cargo run --example test_embedder
```

## 🚀 Scaling Strategy

### Cost Optimization
1. Use light embedder models for most datasets (Cohere embed-light)
2. Use premium models only for important searches
3. Cache embeddings to avoid regeneration
4. Monitor costs with Prometheus alerts

### Performance Optimization
1. Enable product quantization (10x faster search)
2. Batch larger chunks together
3. Deploy worker on GPU machine (if using ONNX models)
4. Increase worker count for parallel processing

### Availability
1. Multi-region deployment
2. Automatic failover between embedder providers
3. Circuit breaker for failing APIs
4. Retry with exponential backoff

---

**Version**: 1.0.0  
**Status**: Production Ready ✅  
**Last Updated**: January 2026


```mermaid
graph TB
    subgraph "Message Queue"
        NATS[NATS JetStream<br/>DATASET_TRANSFORMS]
    end

    subgraph "worker-datasets"
        CONSUMER[Job Consumer]
        BATCH[Batch<br/>Processor]
        EMB[Embedder<br/>Client]
        QD_CLIENT[Qdrant<br/>Client]
    end

    subgraph "External APIs"
        OPENAI[OpenAI API<br/>text-embedding-3-*]
        COHERE[Cohere API<br/>embed-*]
    end

    subgraph "Storage"
        S3[(S3 Storage<br/>Batch Files)]
        QDRANT[(Qdrant<br/>Vector DB)]
    end

    subgraph "Output"
        RESULT[NATS Result<br/>worker.result.vector]
    end

    NATS --> CONSUMER
    CONSUMER --> BATCH
    BATCH --> S3
    BATCH --> EMB
    EMB --> OPENAI & COHERE
    BATCH --> QD_CLIENT
    QD_CLIENT --> QDRANT
    CONSUMER --> RESULT
```

## Processing Pipeline

```mermaid
sequenceDiagram
    participant NATS
    participant Worker
    participant S3
    participant Embedder as Embedder API
    participant Qdrant

    NATS->>Worker: DatasetTransformJob
    Worker->>NATS: Progress update (processing)
    Worker->>S3: Download batch file
    S3-->>Worker: BatchItem[]

    loop For each batch (batch_size chunks)
        Worker->>Embedder: Generate embeddings
        Embedder-->>Worker: float[][]
    end

    Worker->>Qdrant: Check collection exists

    alt Collection doesn't exist
        Worker->>Qdrant: Create collection<br/>(vector size, cosine distance)
    else wipe_collection = true
        Worker->>Qdrant: Delete collection
        Worker->>Qdrant: Create collection
    end

    Worker->>Qdrant: Upsert points<br/>(id, vector, payload)
    Worker->>NATS: DatasetTransformResult
```

## Supported Embedder Providers

| Provider | Models | Batch Size | Notes |
|----------|--------|------------|-------|
| OpenAI | `text-embedding-3-small` (1536d)<br/>`text-embedding-3-large` (3072d)<br/>`text-embedding-ada-002` (1536d) | Up to 2048 | Rate limited by tokens |
| Cohere | `embed-english-v3.0` (1024d)<br/>`embed-multilingual-v3.0` (1024d)<br/>`embed-english-light-v3.0` (384d) | Up to 96 | Input type: search_document |

## Technologies

| Technology | Version | Purpose |
|------------|---------|---------|
| Rust | 2024 Edition | Language |
| tokio | 1.49 | Async runtime |
| async-nats | 0.45 | Message queue consumer |
| aws-sdk-s3 | 1.119 | S3 storage client |
| qdrant-client | 1.16 | Vector database client |
| reqwest | 0.13 | HTTP client for embedder APIs |
| once_cell | 1.21 | Lazy static initialization |

## Module Structure

| Module | Description |
|--------|-------------|
| `main` | Worker initialization and NATS subscription |
| `job` | Job processing, Qdrant operations, result publishing |
| `embedder` | Embedder API client implementations |

## Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `NATS_URL` | string | `nats://localhost:4222` | NATS server URL |
| `AWS_REGION` | string | **required** | S3 region |
| `AWS_ACCESS_KEY_ID` | string | **required** | S3 access key |
| `AWS_SECRET_ACCESS_KEY` | string | **required** | S3 secret key |
| `AWS_ENDPOINT_URL` | string | **required** | S3 endpoint URL |
| `SERVICE_NAME` | string | `worker-datasets` | Service name for tracing |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | string | `http://localhost:4317` | OTLP exporter endpoint |
| `LOG_FORMAT` | string | `json` | Log format (`json` or `pretty`) |
| `RUST_LOG` | string | `info` | Tracing filter directive |
| `MAX_CONCURRENT_JOBS` | integer | `10` | Maximum concurrent job processing |

## Job Message Format

### Input: DatasetTransformJob

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "dataset_transform_id": 123,
  "embedded_dataset_id": 456,
  "owner": "user@example.com",
  "bucket": "dataset-789",
  "batch_file_key": "batches/batch_001.json",
  "collection_name": "embedded_456_embedder_1",
  "batch_size": 100,
  "wipe_collection": false,
  "embedder_config": {
    "provider": "openai",
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-...",
    "model": "text-embedding-3-small",
    "dimensions": 1536
  },
  "vector_database_config": {
    "database_type": "qdrant",
    "connection_url": "http://qdrant:6334",
    "api_key": null
  }
}
```

### Batch File Format

```json
[
  {
    "id": "item_1_chunk_0",
    "text": "This is the first chunk of text...",
    "payload": {
      "dataset_item_id": 1,
      "dataset_id": 789,
      "item_title": "Document Title",
      "chunk_index": 0
    }
  },
  {
    "id": "item_1_chunk_1",
    "text": "This is the second chunk...",
    "payload": {
      "dataset_item_id": 1,
      "dataset_id": 789,
      "item_title": "Document Title",
      "chunk_index": 1
    }
  }
]
```

### Output: DatasetTransformResult

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "dataset_transform_id": 123,
  "embedded_dataset_id": 456,
  "owner": "user@example.com",
  "batch_file_key": "batches/batch_001.json",
  "chunk_count": 42,
  "status": "success",
  "error": null,
  "processing_duration_ms": 3250
}
```

## Qdrant Collection Schema

Collections are created with the following configuration:

```rust
Collection {
    name: "{embedded_dataset_id}_embedder_{embedder_id}",
    vectors: VectorParams {
        size: <embedding_dimensions>,
        distance: Cosine,
    }
}
```

### Point Structure

```json
{
  "id": "item_1_chunk_0",
  "vector": [0.123, -0.456, ...],
  "payload": {
    "text": "Original chunk text (for preview)",
    "dataset_item_id": 1,
    "dataset_id": 789,
    "item_title": "Document Title",
    "chunk_index": 0
  }
}
```

## Observability

### Metrics

The worker exports the following metrics via the core observability module:

- `worker_jobs_total{worker="vector-embed", status="success|failed_*"}` - Job completion counter
- `worker_job_duration_seconds{worker="vector-embed"}` - Job duration histogram
- `dataset_transform_jobs_total{transform_id, embedded_dataset_id, status}` - Transform-specific counter
- `dataset_transform_batches_processed{transform_id, embedded_dataset_id}` - Batches processed counter
- `dataset_transform_chunks_embedded{transform_id, embedded_dataset_id}` - Chunks embedded counter

### Tracing

Jobs are traced with the following span attributes:
- `job_id` - Unique job identifier
- `dataset_transform_id` - Transform pipeline ID
- `embedded_dataset_id` - Target embedded dataset ID
- `collection` - Qdrant collection name

### Error Categories

| Status | Description |
|--------|-------------|
| `success` | Job completed successfully |
| `success_empty` | Empty batch (no items to process) |
| `failed_download` | Could not download batch from S3 |
| `failed_parse` | Invalid batch JSON format |
| `failed_embedding` | Embedder API call failed |
| `failed_mismatch` | Embedding count doesn't match input |
| `failed_upsert` | Qdrant upsert operation failed |

## Embedding Generation Flow

```mermaid
flowchart TD
    A[Receive Batch] --> B{Parse Items}
    B -->|Success| C[Extract Texts]
    B -->|Failure| Z[Report Error]

    C --> D{Provider?}

    D -->|OpenAI| E[POST /embeddings]
    D -->|Cohere| F[POST /embed]

    E --> G[Parse OpenAI Response]
    F --> H[Parse Cohere Response]

    G --> I{Response OK?}
    H --> I

    I -->|Yes| J[Build Points]
    I -->|No| Z

    J --> K[Upsert to Qdrant]
    K --> L{Upsert OK?}

    L -->|Yes| M[Report Success]
    L -->|No| Z

    Z --> N[Send Error Result]
    M --> O[Send Success Result]
```

## Running

### Development

```bash
# Set environment variables
export NATS_URL="nats://localhost:4222"
export AWS_REGION="us-east-1"
export AWS_ACCESS_KEY_ID="minioadmin"
export AWS_SECRET_ACCESS_KEY="minioadmin"
export AWS_ENDPOINT_URL="http://localhost:9000"

# Run the worker
cargo run --bin worker-datasets
```

### Docker

```bash
docker run \
  -e NATS_URL="nats://nats:4222" \
  -e AWS_REGION="us-east-1" \
  -e AWS_ACCESS_KEY_ID="..." \
  -e AWS_SECRET_ACCESS_KEY="..." \
  -e AWS_ENDPOINT_URL="http://minio:9000" \
  ghcr.io/your-org/worker-datasets:latest
```

### Kubernetes

Deploy as part of the Helm chart with horizontal pod autoscaling based on NATS queue depth.

## Concurrency Configuration

### MAX_CONCURRENT_JOBS

Controls the maximum number of embedding jobs processed simultaneously by a single worker instance:

```bash
# Default: 10 jobs per worker
export MAX_CONCURRENT_JOBS=10

# For rate-limited embedder APIs (e.g., development, testing)
export MAX_CONCURRENT_JOBS=3

# For high-throughput deployments with local embedders
export MAX_CONCURRENT_JOBS=5
```

**Recommendation**: Start with the default (10) and adjust based on:
- Embedder API rate limits (OpenAI, Cohere have token/request limits)
- Available memory (scales with batch size and embedding dimensions)
- Qdrant connection pool capacity
- Monitor for rate limit errors and reduce if needed

## Scaling Considerations

- **Horizontal Scaling**: Multiple worker replicas can process jobs in parallel
- **Backpressure**: Controlled via `max_ack_pending` in NATS consumer config
- **Memory**: Memory usage scales with batch size and embedding dimensions
- **Rate Limits**: Embedder APIs have rate limits; configure `batch_size` and `MAX_CONCURRENT_JOBS` accordingly
- **Retries**: Failed jobs are retried up to 5 times with exponential backoff
- **Qdrant Sharding**: For large-scale deployments, configure Qdrant sharding
- **Concurrency**: Use `MAX_CONCURRENT_JOBS` to prevent API rate limiting per replica

## Cost Considerations

Embedding API calls are billed per token/request:

| Provider | Pricing Model | Optimization |
|----------|--------------|--------------|
| OpenAI | Per 1K tokens | Batch requests, use smaller model when possible |
| Cohere | Per API call | Maximize batch size (up to 96) |

Monitor the `dataset_transform_chunks_embedded` metric to track usage.

## License

See LICENSE file in repository root.
