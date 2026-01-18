# Datasets Worker

Embedding generation worker for Semantic Explorer. Processes dataset chunks, generates vector embeddings using configured embedders, and uploads results to Qdrant for semantic search.

## Overview

The datasets worker consumes jobs from the `DATASET_TRANSFORMS` NATS stream, fetches text chunks from PostgreSQL datasets, generates embeddings in batches using embedder APIs (OpenAI, Cohere, Jina AI, Voyage AI, or local FastEmbed), and uploads the vectors to Qdrant with metadata.

## Architecture

```mermaid
graph TB
    subgraph "Job Processing"
        NATS[NATS JetStream<br/>DATASET_TRANSFORMS]
        WORKER[Datasets Worker<br/>Pull Consumer]
    end

    subgraph "Processing Pipeline"
        FETCH[Fetch Chunks<br/>from PostgreSQL]
        BATCH[Batch Chunks<br/>for Embedder]
        EMBED[Generate Embeddings<br/>via Embedder API]
        UPLOAD[Upload Vectors<br/>to Qdrant]
        RECORD[Record Progress<br/>in PostgreSQL]
    end

    subgraph "Embedder Providers"
        OPENAI[OpenAI<br/>text-embedding-3-*]
        COHERE[Cohere<br/>embed-english-v3]
        JINA[Jina AI<br/>jina-embeddings-v2]
        VOYAGE[Voyage AI<br/>voyage-2]
        LOCAL[Local FastEmbed<br/>embedding-inference-api]
    end

    subgraph "Storage"
        DB[(PostgreSQL<br/>Chunks & Batches)]
        QD[(Qdrant<br/>Vector Store)]
    end

    NATS --> WORKER
    WORKER --> FETCH
    FETCH --> DB
    FETCH --> BATCH
    BATCH --> EMBED

    EMBED --> OPENAI
    EMBED --> COHERE
    EMBED --> JINA
    EMBED --> VOYAGE
    EMBED --> LOCAL

    OPENAI --> UPLOAD
    COHERE --> UPLOAD
    JINA --> UPLOAD
    VOYAGE --> UPLOAD
    LOCAL --> UPLOAD

    UPLOAD --> QD
    UPLOAD --> RECORD
    RECORD --> DB
```

## Features

### Supported Embedders

#### OpenAI

```json
{
  "provider": "openai",
  "base_url": "https://api.openai.com/v1",
  "model": "text-embedding-3-small",
  "api_key": "sk-..."
}
```

**Models**:
- `text-embedding-3-small` (1536 dimensions) - Cost-effective, fast
- `text-embedding-3-large` (3072 dimensions) - Higher quality
- `text-embedding-ada-002` (1536 dimensions) - Legacy model

**Max Batch Size**: 2048 chunks per request

#### Cohere

```json
{
  "provider": "cohere",
  "base_url": "https://api.cohere.ai/v1",
  "model": "embed-english-v3.0",
  "api_key": "..."
}
```

**Models**:
- `embed-english-v3.0` (1024 dimensions) - English text
- `embed-multilingual-v3.0` (1024 dimensions) - 100+ languages
- `embed-english-light-v3.0` (384 dimensions) - Lightweight

**Max Batch Size**: 96 chunks per request

#### Internal Embedding Inference API

```json
{
  "provider": "internal",
  "base_url": "http://embedding-inference-api:8090",
  "model": "BAAI/bge-small-en-v1.5"
}
```

**Models** (via FastEmbed):
- `BAAI/bge-small-en-v1.5` (384 dimensions)
- `BAAI/bge-base-en-v1.5` (768 dimensions)
- `BAAI/bge-large-en-v1.5` (1024 dimensions)
- `sentence-transformers/all-MiniLM-L6-v2` (384 dimensions)

**Max Batch Size**: Configurable, default 100

### Batch Processing

Chunks are processed in configurable batches:

1. **Fetch batch** from dataset
2. **Generate embeddings** via embedder API
3. **Upload vectors** to Qdrant collection
4. **Record batch** metadata in PostgreSQL
5. **Publish progress** update to NATS
6. **Repeat** until all chunks processed

**Benefits**:
- Reduced API calls (embedder batch endpoints)
- Better throughput
- Progress tracking (resume on failure)
- Memory-efficient streaming

### Qdrant Integration

Vectors uploaded to Qdrant with rich metadata:

```json
{
  "id": "chunk-uuid",
  "vector": [0.123, -0.456, ...],
  "payload": {
    "dataset_item_id": "item-uuid",
    "chunk_index": 0,
    "text": "Original chunk text",
    "source_file": "document.pdf",
    "author": "John Doe",
    "title": "Annual Report",
    "custom_metadata": {}
  }
}
```

**Collection Management**:
- Auto-create collections with correct vector size
- Distance metric: Cosine similarity
- HNSW indexing for fast search
- Quantization support (optional)

## Configuration

All configuration via environment variables:

### Worker Configuration

```bash
# NATS connection (inherited from core)
NATS_URL=nats://localhost:4222

# Database connection (inherited from core)
DATABASE_URL=postgresql://localhost/semantic_explorer

# Qdrant connection (inherited from core)
QDRANT_URL=http://localhost:6333
QDRANT_API_KEY=your-api-key

# Worker concurrency
WORKER_CONCURRENCY=4

# Max retries for failed jobs
MAX_RETRIES=3

# Job processing timeout (seconds)
JOB_TIMEOUT=1800
```

### Embedding Configuration

```bash
# Batch size for embedding generation
EMBEDDING_BATCH_SIZE=100

# Max chunks per transform job
MAX_CHUNKS_PER_JOB=100000

# Embedder API timeout (seconds)
EMBEDDER_TIMEOUT=60

# Retry strategy for embedder API
EMBEDDER_MAX_RETRIES=3
EMBEDDER_RETRY_DELAY_MS=1000

# Rate limiting
MAX_EMBEDDER_REQUESTS_PER_SECOND=10
```

### Qdrant Configuration

```bash
# Collection creation
QDRANT_CREATE_COLLECTION=true

# Distance metric (Cosine, Euclid, Dot)
QDRANT_DISTANCE=Cosine

# HNSW index parameters
QDRANT_HNSW_M=16
QDRANT_HNSW_EF_CONSTRUCT=100

# Upload batch size
QDRANT_UPLOAD_BATCH_SIZE=100

# Enable quantization (reduce memory)
QDRANT_ENABLE_QUANTIZATION=false
```

### Observability Configuration

```bash
# OpenTelemetry
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=worker-datasets
OTEL_SERVICE_VERSION=1.0.0

# Logging
RUST_LOG=info,worker_datasets=debug
LOG_FORMAT=json
```

## Building

### Debug Build

```bash
cargo build -p worker-datasets
```

### Release Build

```bash
cargo build -p worker-datasets --release
```

Binary location: `target/release/worker-datasets`

### Docker Build

```bash
# Build from repository root
docker build -f crates/worker-datasets/Dockerfile -t worker-datasets:latest .
```

## Running

### Local Development

```bash
# Set required environment variables
export DATABASE_URL=postgresql://localhost/semantic_explorer
export NATS_URL=nats://localhost:4222
export QDRANT_URL=http://localhost:6333

# Run with cargo
cargo run -p worker-datasets

# Or run the compiled binary
./target/release/worker-datasets
```

### Docker

```bash
docker run -d \
  --name worker-datasets \
  -e DATABASE_URL=postgresql://postgres:password@postgres:5432/semantic_explorer \
  -e NATS_URL=nats://nats:4222 \
  -e QDRANT_URL=http://qdrant:6333 \
  worker-datasets:latest
```

## Job Processing Flow

```mermaid
sequenceDiagram
    participant API
    participant NATS
    participant Worker
    participant DB
    participant Embedder as Embedder API
    participant Qdrant

    API->>NATS: Publish DatasetTransform job
    Worker->>NATS: Pull job (with ack)
    Worker->>DB: Fetch transform config & embedder
    Worker->>DB: Count total chunks

    loop For each batch
        Worker->>DB: Fetch batch of chunks
        Worker->>Embedder: Generate embeddings
        Embedder-->>Worker: Return vectors
        Worker->>Qdrant: Upload vectors with metadata
        Worker->>DB: Record batch completion
        Worker->>NATS: Publish progress update
    end

    Worker->>DB: Create EmbeddedDataset record
    Worker->>DB: Update transform status
    Worker->>NATS: ACK job
    Worker->>NATS: Publish completion status
```

## Metrics

Prometheus metrics exposed on port 9090 (configurable):

### Job Metrics

```
# Total jobs processed
dataset_transform_jobs_total{status="success|failure"}

# Job processing duration
dataset_transform_duration_seconds

# Active jobs currently processing
dataset_transform_active_jobs

# Batches processed per job
dataset_transform_batches_processed

# Chunks embedded per job
dataset_transform_chunks_embedded
```

### Embedding Metrics

```
# Embedding API calls
embedder_api_calls_total{provider="openai|cohere|...",status="success|failure"}

# Embedding API latency
embedder_api_duration_seconds{provider="openai|cohere|..."}

# Chunks embedded per call
embedder_chunks_per_call{provider="openai|cohere|..."}

# Embedder API errors
embedder_api_errors_total{provider="openai|cohere|...",error_type="..."}
```

### Qdrant Metrics

```
# Qdrant upload calls
qdrant_upload_total{status="success|failure"}

# Qdrant upload duration
qdrant_upload_duration_seconds

# Vectors uploaded per batch
qdrant_vectors_per_batch

# Qdrant errors
qdrant_errors_total{error_type="..."}
```

### Worker Metrics

```
# Worker health
worker_ready{worker="worker-datasets"}

# Active worker tasks
worker_active_jobs{worker="worker-datasets"}

# Job retries
worker_job_retries_total{worker="worker-datasets"}

# Worker failures
worker_job_failures_total{worker="worker-datasets",error_type="..."}
```

## Error Handling

### Retry Logic

Failed jobs are automatically retried with exponential backoff:

1. **First retry**: 30 seconds
2. **Second retry**: 2 minutes
3. **Third retry**: 5 minutes
4. **Fourth retry**: 15 minutes
5. **Fifth retry**: 1 hour
6. **After max attempts**: Move to Dead Letter Queue (DLQ)

### Dead Letter Queue

Jobs that exceed max delivery attempts are routed to `DLQ_DATASET_TRANSFORMS` stream:

```bash
# Inspect DLQ
nats stream info DLQ_DATASET_TRANSFORMS

# Get DLQ messages
nats consumer next DLQ_DATASET_TRANSFORMS worker-datasets-dlq --count 10

# Replay DLQ message
nats pub DATASET_TRANSFORMS "$(nats consumer next DLQ_DATASET_TRANSFORMS worker-datasets-dlq --count 1 --no-ack)"
```

### Common Errors

#### Embedder API rate limit

```
Error: Rate limit exceeded: 429 Too Many Requests
```

**Solution**: Reduce `EMBEDDING_BATCH_SIZE` or `MAX_EMBEDDER_REQUESTS_PER_SECOND`

#### Embedder API timeout

```
Error: Embedder API timeout after 60s
```

**Solution**: Increase `EMBEDDER_TIMEOUT` or reduce batch size

#### Qdrant connection error

```
Error: Failed to connect to Qdrant: connection refused
```

**Solution**: Verify `QDRANT_URL` and ensure Qdrant is running

#### Collection creation failed

```
Error: Failed to create collection: invalid vector size
```

**Solution**: Check embedder model output dimensionality matches configuration

#### Out of memory

```
Error: Cannot allocate memory
```

**Solution**: Reduce `EMBEDDING_BATCH_SIZE` or `WORKER_CONCURRENCY`

## Deployment

### Docker Compose

```yaml
services:
  worker-datasets:
    image: worker-datasets:latest
    environment:
      DATABASE_URL: postgresql://postgres:password@postgres:5432/semantic_explorer
      NATS_URL: nats://nats:4222
      QDRANT_URL: http://qdrant:6333
      RUST_LOG: info
      WORKER_CONCURRENCY: 4
      EMBEDDING_BATCH_SIZE: 100
    depends_on:
      - postgres
      - nats
      - qdrant
    restart: unless-stopped
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: worker-datasets
spec:
  serviceName: worker-datasets
  replicas: 3
  selector:
    matchLabels:
      app: worker-datasets
  template:
    metadata:
      labels:
        app: worker-datasets
    spec:
      containers:
      - name: worker-datasets
        image: worker-datasets:latest
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: semantic-explorer-secrets
              key: database-url
        - name: NATS_URL
          value: nats://nats:4222
        - name: QDRANT_URL
          value: http://qdrant:6333
        - name: WORKER_CONCURRENCY
          value: "4"
        - name: EMBEDDING_BATCH_SIZE
          value: "100"
        resources:
          requests:
            memory: 1Gi
            cpu: 500m
          limits:
            memory: 2Gi
            cpu: 2000m
```

## Development

### Project Structure

```
src/
├── main.rs                   # Worker initialization
├── job.rs                    # Job processing logic
├── embedder/                 # Embedder client
│   ├── mod.rs
│   ├── client.rs             # HTTP client for embedder APIs
│   └── batch.rs              # Batching logic
└── qdrant/                   # Qdrant integration
    ├── mod.rs
    ├── collection.rs         # Collection management
    └── upload.rs             # Vector upload
```

### Testing

```bash
# Run all tests
cargo test -p worker-datasets

# Run with logging
RUST_LOG=debug cargo test -p worker-datasets -- --nocapture

# Integration tests (requires dependencies)
cargo test -p worker-datasets --test '*' -- --ignored
```

## Troubleshooting

### Worker not consuming jobs

```bash
# Check NATS consumer status
nats consumer info DATASET_TRANSFORMS worker-datasets

# Check consumer lag
nats stream info DATASET_TRANSFORMS

# Check worker logs
docker logs worker-datasets -f --tail 100
```

### Embedding failures

```bash
# Enable debug logging
export RUST_LOG=worker_datasets=debug,worker_datasets::embedder=trace

# Test embedder directly
curl -X POST http://localhost:8090/embed \
  -H "Content-Type: application/json" \
  -d '{"input": ["test text"]}'
```

### Qdrant upload failures

```bash
# Check Qdrant health
curl http://localhost:6333/health

# List collections
curl http://localhost:6333/collections

# Check collection info
curl http://localhost:6333/collections/{collection_name}
```

### Memory issues

```bash
# Reduce batch size
export EMBEDDING_BATCH_SIZE=50

# Reduce worker concurrency
export WORKER_CONCURRENCY=2

# Monitor memory usage
docker stats worker-datasets
```

## Performance Tuning

### Throughput Optimization

```bash
# Increase concurrency
export WORKER_CONCURRENCY=8

# Increase batch size (if embedder supports)
export EMBEDDING_BATCH_SIZE=500

# Increase Qdrant upload batch
export QDRANT_UPLOAD_BATCH_SIZE=500
```

### Latency Optimization

```bash
# Increase timeouts
export EMBEDDER_TIMEOUT=120
export JOB_TIMEOUT=3600

# Reduce retry delays
export EMBEDDER_RETRY_DELAY_MS=500
```

### Cost Optimization (Cloud Embedders)

```bash
# Use cost-effective models
# OpenAI: text-embedding-3-small instead of 3-large
# Cohere: embed-english-light-v3.0 instead of v3.0

# Maximize batch size to reduce API calls
export EMBEDDING_BATCH_SIZE=2048  # OpenAI max

# Use local FastEmbed for development/testing
export USE_LOCAL_EMBEDDER=true
```

## Dependencies

Key dependencies:

- **qdrant-client** - Qdrant vector database client
- **reqwest** - HTTP client for embedder APIs
- **serde/serde_json** - JSON serialization
- **tokio** - Async runtime
- **async-nats** - NATS client
- **semantic-explorer-core** - Shared library

See [Cargo.toml](Cargo.toml) for complete list.

## License

Apache License 2.0
