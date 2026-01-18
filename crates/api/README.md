# Semantic Explorer API Server

The main REST API server for Semantic Explorer, providing endpoints for collection management, dataset processing, embedding generation, search, and RAG-based chat.

## Overview

The API server orchestrates all system operations:
- **Collection & Dataset Management**: CRUD operations for data organization
- **Transform Orchestration**: Publish jobs to NATS for workers to process
- **Search**: Semantic search across embedded datasets
- **RAG Chat**: Context-aware conversations with LLM integration
- **Real-time Updates**: Server-Sent Events (SSE) for transform progress
- **Authentication**: OIDC integration
- **Observability**: Prometheus metrics, OpenTelemetry tracing and structured logging

## Architecture

```mermaid
graph TD
    subgraph "HTTP Handlers"
        API[API Endpoints<br/>OpenAPI/Swagger]
        AUTH[OIDC Auth Middleware]
        CORS[CORS Middleware]
    end

    subgraph "Core Services"
        COLL[Collections Service]
        DS[Datasets Service]
        EMB[Embedders Service]
        TRANS[Transforms Service]
        SEARCH[Search Service]
        CHAT[Chat Service]
    end

    subgraph "Background Workers"
        SCANNER[Transform Scanners<br/>Periodic retry]
        LISTENER[Result Listeners<br/>Status updates]
        AUDIT[Audit Event Consumer]
    end

    subgraph "External Dependencies"
        PG[(PostgreSQL)]
        NATS[NATS JetStream]
        QD[(Qdrant)]
        S3[(S3/MinIO)]
    end

    API --> AUTH
    AUTH --> CORS
    CORS --> COLL
    CORS --> DS
    CORS --> EMB
    CORS --> TRANS
    CORS --> SEARCH
    CORS --> CHAT

    COLL --> PG
    COLL --> S3
    DS --> PG
    EMB --> PG
    TRANS --> NATS
    TRANS --> PG
    SEARCH --> QD
    SEARCH --> PG
    CHAT --> QD
    CHAT --> EMB

    SCANNER --> NATS
    SCANNER --> PG
    LISTENER --> NATS
    LISTENER --> PG
    AUDIT --> NATS
    AUDIT --> PG
```

## Features

### Core Functionality

- **Collections**: Upload and organize source documents (PDF, Office, Markdown, code files, archives)
- **Datasets**: Store extracted text chunks from processed collections, or direct API uploads
- **Embedders**: Configure embedding providers (OpenAI, Cohere, Jina AI, Voyage AI, Internal embedding-inference-api)
- **LLMs**: Configure LLM providers for chat/RAG (OpenAI, Cohere, Internal LLM inference API)
- **Collection Transforms**: Extract text from files and generate chunks into datasets
- **Dataset Transforms**: Generate embeddings for chunks and upload to Qdrant
- **Visualization Transforms**: Create UMAP/HDBSCAN visualizations
- **Search**: Semantic search with configurable weights
- **Chat**: RAG-based conversations with context retrieval and streaming responses
- **Marketplace**: Share and discover public collections, datasets, embedders, and LLMs

### Security & Authentication

- **OIDC Integration**: OpenID Connect authentication
- **API Key Encryption**: AES-256-GCM encryption for stored credentials
- **CORS**: Configurable cross-origin resource sharing
- **Security Headers**: X-Frame-Options, X-Content-Type-Options, CSP
- **Audit Logging**: All API actions logged to PostgreSQL with user identity

### Observability

- **Metrics**: Prometheus metrics on `/metrics` endpoint
  - HTTP request count, duration, status codes
  - Database query performance
  - Storage operation latency
  - Search query timing
  - SSE connection health
- **Tracing**: OpenTelemetry distributed tracing (OTLP export)
- **Logging**: Structured JSON logging with trace/span correlation
- **Health Checks**: `/health/live` (liveness) and `/health/ready` (readiness)

## API Endpoints

### Health

- `GET /health/live` - Liveness probe (always returns 200)
- `GET /health/ready` - Readiness probe (checks database connection)

### Collections

- `GET /api/collections` - List collections
- `GET /api/collections/{id}` - Get collection details
- `POST /api/collections` - Create collection
- `PUT /api/collections/{id}` - Update collection
- `DELETE /api/collections/{id}` - Delete collection
- `POST /api/collections/{id}/files` - Upload files
- `GET /api/collections/{id}/files` - List files
- `DELETE /api/collections/{id}/files/{path}` - Delete file
- `GET /api/collections/{id}/files/{path}` - Download file

### Datasets

- `GET /api/datasets` - List datasets
- `GET /api/datasets/{id}` - Get dataset details
- `POST /api/datasets` - Create dataset
- `PUT /api/datasets/{id}` - Update dataset
- `DELETE /api/datasets/{id}` - Delete dataset
- `GET /api/datasets/{id}/items` - List dataset items (chunks)
- `GET /api/datasets/{id}/items/summary` - Get items summary
- `GET /api/datasets/{id}/items/{item_id}/chunks` - Get item chunks
- `DELETE /api/datasets/{id}/items/{item_id}` - Delete item

### Embedded Datasets

- `GET /api/embedded_datasets` - List embedded datasets
- `GET /api/embedded_datasets/{id}` - Get embedded dataset details
- `PUT /api/embedded_datasets/{id}` - Update embedded dataset
- `DELETE /api/embedded_datasets/{id}` - Delete embedded dataset
- `GET /api/embedded_datasets/{id}/stats` - Get statistics
- `GET /api/embedded_datasets/{id}/points` - List vector points
- `GET /api/embedded_datasets/{id}/points/{point_id}/vector` - Get vector

### Embedders

- `GET /api/embedders` - List embedders
- `GET /api/embedders/{id}` - Get embedder details
- `POST /api/embedders` - Create embedder
- `PUT /api/embedders/{id}` - Update embedder
- `DELETE /api/embedders/{id}` - Delete embedder
- `POST /api/embedders/{id}/test` - Test embedder

### LLMs

- `GET /api/llms` - List LLMs
- `GET /api/llms/{id}` - Get LLM details
- `POST /api/llms` - Create LLM
- `PUT /api/llms/{id}` - Update LLM
- `DELETE /api/llms/{id}` - Delete LLM

### Collection Transforms

- `GET /api/collection_transforms` - List collection transforms
- `GET /api/collection_transforms/{id}` - Get transform details
- `POST /api/collection_transforms` - Create transform
- `PUT /api/collection_transforms/{id}` - Update transform
- `DELETE /api/collection_transforms/{id}` - Delete transform
- `POST /api/collection_transforms/{id}/trigger` - Trigger execution
- `GET /api/collection_transforms/stream` - SSE status stream
- `GET /api/collection_transforms/{id}/stats` - Get statistics
- `GET /api/collection_transforms/{id}/files` - List processed files

### Dataset Transforms

- `GET /api/dataset_transforms` - List dataset transforms
- `GET /api/dataset_transforms/{id}` - Get transform details
- `POST /api/dataset_transforms` - Create transform
- `PUT /api/dataset_transforms/{id}` - Update transform
- `DELETE /api/dataset_transforms/{id}` - Delete transform
- `POST /api/dataset_transforms/{id}/trigger` - Trigger execution
- `GET /api/dataset_transforms/stream` - SSE status stream
- `GET /api/dataset_transforms/{id}/stats` - Get statistics
- `GET /api/dataset_transforms/{id}/batches` - List batches

### Visualization Transforms

- `GET /api/visualization_transforms` - List visualization transforms
- `GET /api/visualization_transforms/{id}` - Get transform details
- `POST /api/visualization_transforms` - Create transform
- `PUT /api/visualization_transforms/{id}` - Update transform
- `DELETE /api/visualization_transforms/{id}` - Delete transform
- `POST /api/visualization_transforms/{id}/trigger` - Trigger execution
- `GET /api/visualization_transforms/stream` - SSE status stream
- `GET /api/visualizations` - List visualizations
- `GET /api/visualizations/{id}` - Get visualization details
- `GET /api/visualizations/{id}/html` - Download HTML file

### Search

- `POST /api/search` - Hybrid search across embedded datasets

### Chat

- `POST /api/chat/sessions` - Create chat session
- `GET /api/chat/sessions` - List chat sessions
- `GET /api/chat/sessions/{id}` - Get session details
- `DELETE /api/chat/sessions/{id}` - Delete session
- `GET /api/chat/sessions/{id}/messages` - List messages
- `POST /api/chat/sessions/{id}/messages` - Send message (non-streaming)
- `GET /api/chat/sessions/{id}/messages/stream` - Send message (SSE streaming)

### Marketplace

- `GET /api/marketplace/collections` - List public collections
- `GET /api/marketplace/datasets` - List public datasets
- `GET /api/marketplace/embedders` - List public embedders
- `GET /api/marketplace/llms` - List public LLMs
- `POST /api/marketplace/collections/{id}/grab` - Clone collection
- `POST /api/marketplace/datasets/{id}/grab` - Clone dataset

### Interactive Documentation

- `GET /swagger-ui` - Swagger UI for interactive API exploration

## Configuration

All configuration via environment variables:

### Server Configuration

```bash
# Server binding
API_HOST=0.0.0.0                    # Bind address
API_PORT=8080                        # Port number

# Public URL (for OIDC callbacks)
PUBLIC_URL=https://semantic-explorer.example.com

# CORS
CORS_ALLOWED_ORIGINS=https://example.com,https://app.example.com

# Static files (UI) - ONLY FOR LOCAL DEVELOPMENT.  
# In production, the UI is served by API service.
STATIC_FILES_DIR=../semantic-explorer-ui/dist

# Shutdown timeout
SHUTDOWN_TIMEOUT_SECS=30
```

### Database Configuration

```bash
# PostgreSQL connection
DATABASE_URL=postgresql://user:password@localhost:5432/semantic_explorer

# Connection pool
DATABASE_MAX_CONNECTIONS=20
DATABASE_MIN_CONNECTIONS=2
DATABASE_CONNECT_TIMEOUT=10          # seconds
DATABASE_IDLE_TIMEOUT=600            # seconds
DATABASE_MAX_LIFETIME=1800           # seconds
```

### NATS Configuration

```bash
# NATS server
NATS_URL=nats://localhost:4222

# JetStream configuration
NATS_STREAM_REPLICAS=1               # Number of stream replicas
NATS_MAX_STREAM_SIZE_BYTES=10737418240  # 10GB per stream
NATS_CONSUMER_MAX_DELIVER=5          # Max delivery attempts
```

### Qdrant Configuration

```bash
# Qdrant server
QDRANT_URL=http://localhost:6333

# Optional API key
QDRANT_API_KEY=your-api-key

# Connection timeout
QDRANT_TIMEOUT=60                    # seconds
```

### S3/MinIO Configuration

```bash
# S3 endpoint (use AWS S3 endpoint or MinIO)
S3_ENDPOINT=http://localhost:9000

# S3 credentials
S3_ACCESS_KEY=minioadmin
S3_SECRET_KEY=minioadmin

# Bucket name
S3_BUCKET=semantic-explorer

# Region (required for AWS S3)
S3_REGION=us-east-1

# Upload limits
S3_MAX_UPLOAD_SIZE_BYTES=104857600   # 100MB

# Path style (true for MinIO)
S3_PATH_STYLE=true
```

### Encryption Configuration

```bash
# Master key for encrypting API keys (AES-256-GCM)
# Generate with: openssl rand -hex 32
ENCRYPTION_MASTER_KEY=your-64-char-hex-key
```

### OIDC Configuration (Optional)

```bash
# Enable OIDC authentication
OIDC_ENABLED=true

# OIDC provider
OIDC_ISSUER_URL=https://dex.example.com
OIDC_CLIENT_ID=semantic-explorer
OIDC_CLIENT_SECRET=client-secret

```

### TLS Configuration (Optional)

```bash
# Server TLS
SERVER_SSL_ENABLED=true
SERVER_CERT_PATH=/path/to/cert.pem
SERVER_KEY_PATH=/path/to/key.pem

# Client TLS (for outbound connections)
TLS_CA_CERT_PATH=/path/to/ca.pem
TLS_CLIENT_CERT_PATH=/path/to/client-cert.pem
TLS_CLIENT_KEY_PATH=/path/to/client-key.pem

//TODO missing mTLS
```

### Observability Configuration

```bash
# OpenTelemetry
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=semantic-explorer
OTEL_SERVICE_VERSION=1.0.0

# Logging
RUST_LOG=info,semantic_explorer=debug
LOG_FORMAT=json                      # or "text" for development
```

### Inference API Configuration (Optional)

```bash
# Local embedding inference API
EMBEDDING_INFERENCE_API_URL=http://localhost:8090

# Local LLM inference API
LLM_INFERENCE_API_URL=http://localhost:8091
```

## Building

### Debug Build

```bash
cargo build -p semantic-explorer
```

### Release Build (Optimized)

```bash
cargo build -p semantic-explorer --release
```

The binary will be at `target/release/semantic-explorer`.

### Docker Build

```bash
# Build from repository root
docker build -f crates/api/Dockerfile -t semantic-explorer:latest .
```

## Running

### Local Development

```bash
# Set required environment variables
export DATABASE_URL=postgresql://localhost/semantic_explorer
export NATS_URL=nats://localhost:4222
export QDRANT_URL=http://localhost:6333
export S3_ENDPOINT=http://localhost:9000
export S3_ACCESS_KEY=minioadmin
export S3_SECRET_KEY=minioadmin
export ENCRYPTION_MASTER_KEY=$(openssl rand -hex 32)

# Run with cargo
cargo run -p semantic-explorer

# Or run the compiled binary
./target/release/semantic-explorer
```

### Docker

```bash
docker run -d \
  --name semantic-explorer \
  -p 8080:8080 \
  -e DATABASE_URL=postgresql://postgres:password@postgres:5432/semantic_explorer \
  -e NATS_URL=nats://nats:4222 \
  -e QDRANT_URL=http://qdrant:6333 \
  -e S3_ENDPOINT=http://minio:9000 \
  -e S3_ACCESS_KEY=minioadmin \
  -e S3_SECRET_KEY=minioadmin \
  -e ENCRYPTION_MASTER_KEY=your-key \
  semantic-explorer:latest
```

### Health Check

```bash
# Liveness (process running)
curl http://localhost:8080/health/live

# Readiness (database connected)
curl http://localhost:8080/health/ready
```

## Metrics

Prometheus metrics available at `http://localhost:8080/metrics`.

### Key Metrics

#### HTTP Metrics

```
# Request count by method, path, status
http_requests_total{method="GET",path="/api/collections",status="200"}

# Request duration (histogram)
http_request_duration_seconds{method="POST",path="/api/search",status="200"}

# Requests in flight
http_requests_in_flight

# SSE connections
sse_connections_active
sse_messages_sent
sse_connection_duration_seconds
```

#### Database Metrics

```
# Query count by operation and status
database_query_total{operation="select",status="success"}

# Query duration
database_query_duration_seconds{operation="insert"}

# Connection pool
database_connection_pool_size
database_connection_pool_active
database_connection_pool_idle
```

#### Storage Metrics

```
# S3 operations
storage_operations_total{operation="upload",status="success"}
storage_operation_duration_seconds{operation="download"}
storage_file_size_bytes
```

#### Search Metrics

```
# Search requests
search_request_total{endpoint="hybrid"}
search_request_duration_seconds{endpoint="hybrid"}

# Component timing
search_embedder_call_duration_seconds
search_qdrant_query_duration_seconds
search_results_returned
```

#### Transform Metrics

```
# Transform job publishing
transform_job_published_total{type="collection"}
transform_job_publish_duration_seconds{type="dataset"}
```

## Database Schema

### Core Tables

- `collections` - File collections
- `datasets` - Text chunk datasets
- `dataset_items` - Individual items in datasets
- `dataset_item_chunks` - Text chunks
- `embedded_datasets` - Vector embeddings metadata
- `embedders` - Embedding provider configurations
- `llms` - LLM provider configurations

### Transform Tables

- `collection_transforms` - File extraction jobs
- `collection_transform_files` - Per-file processing status
- `dataset_transforms` - Embedding generation jobs
- `dataset_transform_batches` - Batch processing status
- `visualization_transforms` - UMAP/HDBSCAN jobs
- `visualizations` - Generated visualizations

### Supporting Tables

- `chat_sessions` - Chat conversation sessions
- `chat_messages` - Chat messages and responses
- `audit_events` - Audit log

### Migrations

Database migrations managed with `sqlx-cli`:

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
sqlx migrate run --database-url $DATABASE_URL

# Create new migration
sqlx migrate add my_migration_name
```

## Development

### Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- NATS with JetStream enabled
- Qdrant 1.7+
- S3-compatible storage (MinIO for local dev)

### Running Tests

```bash
# Unit tests
cargo test -p semantic-explorer

# Integration tests (requires running dependencies)
TEST_DATABASE_URL=postgresql://localhost/semantic_explorer_test \
cargo test -p semantic-explorer --test '*'
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Check for unsafe code (forbidden by lints)
cargo clippy -p semantic-explorer
```

### Hot Reloading (Development)

```bash
# Install cargo-watch
cargo install cargo-watch

# Run with auto-reload
cargo watch -x 'run -p semantic-explorer'
```

## Troubleshooting

### Database Connection Errors

```bash
# Check PostgreSQL is running
pg_isready -h localhost -p 5432

# Test connection
psql $DATABASE_URL -c "SELECT 1"

# Check connection pool settings
# Increase max connections if needed
export DATABASE_MAX_CONNECTIONS=50
```

### NATS Connection Errors

```bash
# Check NATS server
nats server check

# List streams
nats stream ls

# Check consumer status
nats consumer ls COLLECTION_TRANSFORMS
```

### Qdrant Connection Errors

```bash
# Check Qdrant health
curl http://localhost:6333/health

# List collections
curl http://localhost:6333/collections
```

### High Memory Usage

```bash
# Reduce worker threads
export API_WORKERS=2

# Reduce database pool
export DATABASE_MAX_CONNECTIONS=10

# Enable memory profiling
RUST_LOG=debug cargo run -p semantic-explorer
```

### Slow API Responses

```bash
# Check database query performance
# Enable query logging
export RUST_LOG=sqlx=debug

# Check Prometheus metrics for bottlenecks
curl http://localhost:8080/metrics | grep duration
```

## Production Deployment

### Resource Requirements

**Minimum**:
- 2 CPU cores
- 4GB RAM
- 10GB disk (for logs, temp files)

**Recommended**:
- 4+ CPU cores
- 8GB+ RAM
- 50GB+ disk

### Deployment Checklist

- [ ] Set `ENCRYPTION_MASTER_KEY` (generate securely)
- [ ] Configure `DATABASE_URL` with connection pooling
- [ ] Enable TLS for PostgreSQL, NATS, Qdrant connections
- [ ] Set `PUBLIC_URL` for OIDC callbacks
- [ ] Configure CORS origins for production domains
- [ ] Enable OIDC authentication (`OIDC_ENABLED=true`)
- [ ] Set up TLS certificates (`SERVER_SSL_ENABLED=true`)
- [ ] Configure observability endpoints (OTLP, Prometheus)
- [ ] Set appropriate log levels (`RUST_LOG=info`)
- [ ] Enable JSON logging (`LOG_FORMAT=json`)
- [ ] Configure backup for PostgreSQL and Qdrant
- [ ] Set up monitoring alerts (see Grafana dashboards)
- [ ] Test health check endpoints
- [ ] Configure horizontal pod autoscaling (Kubernetes)

## License

Apache License 2.0
