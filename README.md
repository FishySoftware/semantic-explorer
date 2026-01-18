# Semantic Explorer

A comprehensive, production-ready semantic search and RAG (Retrieval-Augmented Generation) platform for document analysis, visualization, and AI-powered chat. Process documents, generate embeddings, visualize high-dimensional data, and perform hybrid search across your knowledge base.

## Overview

Semantic Explorer is a full-stack platform that transforms unstructured documents into searchable, semantically-indexed knowledge bases with interactive visualizations and AI chat capabilities. Built with Rust for performance, Python for ML workloads, and Svelte for the UI.

### Key Features

- **Multi-Format Document Processing**: Extract and chunk text from 10+ file formats (PDF, Office docs, Markdown, code files, archives)
- **Flexible Embedding Pipeline**: Support for local and cloud embedding providers (OpenAI, Cohere, Jina AI, Voyage AI, FastEmbed)
- **Hybrid Search**: Combine vector similarity with BM25 keyword search for optimal retrieval
- **Interactive Visualizations**: UMAP dimensionality reduction with HDBSCAN clustering and LLM-generated topic labels
- **RAG Chat Interface**: Conversational AI with context-aware responses and streaming support
- **Scalable Architecture**: Microservices with NATS JetStream message queue, PostgreSQL, and Qdrant vector database
- **Production Observability**: OpenTelemetry metrics, distributed tracing, structured logging, and Grafana dashboards
- **Multiple Deployment Options**: Docker Compose, Kubernetes/Helm, air-gapped environments

## Architecture

```mermaid
graph TB
    subgraph "Frontend"
        UI[Svelte UI<br/>Port 8080]
    end

    subgraph "API Layer"
        API[API Server<br/>Rust/Actix-web<br/>Port 8080]
    end

    subgraph "Message Queue"
        NATS[NATS JetStream<br/>Streams: COLLECTION_TRANSFORMS<br/>DATASET_TRANSFORMS<br/>VISUALIZATION_TRANSFORMS]
    end

    subgraph "Worker Services"
        WC[worker-collections<br/>File extraction & chunking]
        WD[worker-datasets<br/>Embedding generation]
        WV[worker-visualizations-py<br/>UMAP + HDBSCAN]
    end

    subgraph "Optional Inference APIs"
        EMB[embedding-inference-api<br/>FastEmbed + ONNX<br/>Port 8090]
        LLM[llm-inference-api<br/>mistral.rs<br/>CUDA support]
    end

    subgraph "Data Stores"
        PG[(PostgreSQL<br/>Metadata)]
        QD[(Qdrant<br/>Vector Store)]
        S3[(S3/MinIO<br/>Files)]
    end

    subgraph "Observability"
        PROM[Prometheus<br/>Metrics]
        JAEGER[Jaeger<br/>Traces]
        QUICK[Quickwit<br/>Logs]
        GRAF[Grafana<br/>Dashboards]
    end

    UI -->|HTTP/REST| API
    API -->|Publish Jobs| NATS
    NATS -->|Subscribe| WC
    NATS -->|Subscribe| WD
    NATS -->|Subscribe| WV

    API -->|SQL| PG
    API -->|Vector Search| QD
    API -->|Upload/Download| S3

    WC -->|Store Chunks| PG
    WC -->|Files| S3
    WD -->|Generate Embeddings| EMB
    WD -->|Upload Vectors| QD
    WD -->|Metadata| PG
    WV -->|Download Vectors| QD
    WV -->|Upload HTML| S3
    WV -->|LLM API| LLM

    API -->|Metrics| PROM
    WC -->|Metrics| PROM
    WD -->|Metrics| PROM
    WV -->|Metrics| PROM

    API -->|Traces| JAEGER
    API -->|Logs| QUICK

    GRAF -->|Query| PROM
    GRAF -->|Query| QUICK
    GRAF -->|Query| PG
```

### Data Flow

```mermaid
sequenceDiagram
    participant User
    participant UI
    participant API
    participant NATS
    participant WC as worker-collections
    participant WD as worker-datasets
    participant WV as worker-visualizations
    participant PG as PostgreSQL
    participant QD as Qdrant
    participant S3

    User->>UI: Upload files
    UI->>API: POST /collections/{id}/files
    API->>S3: Store files
    API->>PG: Create Collection

    User->>UI: Create CollectionTransform
    UI->>API: POST /collection_transforms
    API->>NATS: Publish to COLLECTION_TRANSFORMS
    API-->>UI: Transform created

    NATS->>WC: Consume job
    WC->>S3: Download files
    WC->>WC: Extract text & chunk
    WC->>PG: Store chunks in Dataset
    WC->>NATS: Publish TRANSFORM_STATUS

    User->>UI: Create DatasetTransform
    UI->>API: POST /dataset_transforms
    API->>NATS: Publish to DATASET_TRANSFORMS

    NATS->>WD: Consume job
    WD->>PG: Fetch chunks
    WD->>WD: Batch chunks
    WD->>API: Call embedder
    WD->>QD: Upload vectors
    WD->>PG: Create EmbeddedDataset
    WD->>NATS: Publish TRANSFORM_STATUS

    User->>UI: Create VisualizationTransform
    UI->>API: POST /visualization_transforms
    API->>NATS: Publish to VISUALIZATION_TRANSFORMS

    NATS->>WV: Consume job
    WV->>QD: Fetch all vectors
    WV->>WV: UMAP + HDBSCAN
    WV->>API: Generate cluster names (LLM)
    WV->>S3: Upload interactive HTML
    WV->>PG: Store metadata

    User->>UI: Search query
    UI->>API: POST /search
    API->>API: Embed query
    API->>QD: Vector search
    QD-->>API: Results with scores
    API-->>UI: Ranked results

    User->>UI: Chat message
    UI->>API: POST /chat (SSE)
    API->>API: Embed message
    API->>QD: Retrieve context
    API->>API: Call LLM with context
    API-->>UI: Stream response
```

## Project Structure

```
semantic-explorer/
├── crates/
│   ├── api/                          # Main API server (Rust)
│   ├── core/                         # Shared library & utilities (Rust)
│   ├── worker-collections/           # File extraction worker (Rust)
│   ├── worker-datasets/              # Embedding worker (Rust)
│   ├── worker-visualizations-py/     # Visualization worker (Python)
│   ├── embedding-inference-api/      # Local embedding server (Rust)
│   └── llm-inference-api/            # Local LLM server (Rust)
├── semantic-explorer-ui/             # Svelte frontend
├── deployment/
│   ├── compose/                      # Docker Compose configs
│   └── helm/                         # Kubernetes/Helm charts
├── Cargo.toml                        # Rust workspace definition
└── README.md                         # This file
```

### Component Documentation

- **[API Server](crates/api/README.md)** - REST API, authentication, search, chat
- **[Core Library](crates/core/README.md)** - Shared configuration, observability, NATS setup
- **[Collections Worker](crates/worker-collections/README.md)** - File extraction and chunking
- **[Datasets Worker](crates/worker-datasets/README.md)** - Embedding generation
- **[Visualizations Worker](crates/worker-visualizations-py/README.md)** - UMAP/HDBSCAN processing
- **[Embedding Inference API](crates/embedding-inference-api/README.md)** - Local embedding server
- **[LLM Inference API](crates/llm-inference-api/README.md)** - Local LLM server
- **[Svelte UI](semantic-explorer-ui/README.md)** - Frontend application

## Quick Start

### Prerequisites

- **Docker & Docker Compose** (v2.0+) or **Kubernetes** (v1.28+) with Helm
- **Rust** (1.75+) for local development
- **Python** (3.11+) for visualization worker development
- **Node.js** (20+) for UI development

### Development with Docker Compose

```bash
# Clone the repository
git clone https://github.com/yourusername/semantic-explorer.git
cd semantic-explorer

# Start all services (minimal development stack)
cd deployment/compose
docker compose -f compose.dev.yaml up -d

# Check service health
docker compose -f compose.dev.yaml ps

# View logs
docker compose -f compose.dev.yaml logs -f semantic-explorer

# Access the UI
open http://localhost:8080
```

The dev stack includes:
- API server + UI
- 3 worker services
- PostgreSQL, NATS, Qdrant, MinIO
- Prometheus, Grafana, Jaeger (observability)

### Production Deployment with Docker Compose

```bash
cd deployment/compose
docker compose -f compose.yaml up -d
```

See [deployment/compose/README.md](deployment/compose/README.md) for configuration options.

### Kubernetes/Helm Deployment

```bash
# Add Helm repository (if published)
helm repo add semantic-explorer https://charts.example.com
helm repo update

# Install with default values (dev configuration)
helm install my-semantic-explorer semantic-explorer/semantic-explorer \
  -f deployment/helm/semantic-explorer/examples/values-all-included-dev.yaml

# Or for production
helm install my-semantic-explorer semantic-explorer/semantic-explorer \
  -f deployment/helm/semantic-explorer/examples/values-all-included-prod.yaml

# Check deployment status
kubectl get pods -l app.kubernetes.io/name=semantic-explorer

# Port-forward to access UI
kubectl port-forward svc/semantic-explorer 8080:8080
open http://localhost:8080
```

See [deployment/helm/semantic-explorer/README.md](deployment/helm/semantic-explorer/README.md) for detailed Helm configuration.

## Building from Source

### Build All Rust Crates

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Build specific crate
cargo build -p semantic-explorer
cargo build -p worker-collections
```

### Build UI

```bash
cd semantic-explorer-ui
npm install
npm run build
```

### Build Docker Images

```bash
# Build API server
docker build -f crates/api/Dockerfile -t semantic-explorer:latest .

# Build workers
docker build -f crates/worker-collections/Dockerfile -t worker-collections:latest .
docker build -f crates/worker-datasets/Dockerfile -t worker-datasets:latest .
docker build -f crates/worker-visualizations-py/Dockerfile -t worker-visualizations-py:latest .

# Build inference APIs
docker build -f crates/embedding-inference-api/Dockerfile -t embedding-inference-api:latest .
docker build -f crates/llm-inference-api/Dockerfile -t llm-inference-api:latest .
```

## Configuration

All services are configured via environment variables. Key configuration options:

### API Server

```bash
# Server
API_HOST=0.0.0.0
API_PORT=8080
API_WORKERS=4

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/semantic_explorer

# NATS
NATS_URL=nats://localhost:4222

# Qdrant
QDRANT_URL=http://localhost:6333

# S3/MinIO
S3_ENDPOINT=http://localhost:9000
S3_BUCKET=semantic-explorer
S3_ACCESS_KEY=minioadmin
S3_SECRET_KEY=minioadmin

# Observability
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=info,semantic_explorer=debug
```

### Workers

```bash
# Collections Worker
WORKER_CONCURRENCY=4
MAX_FILE_SIZE_MB=100

# Datasets Worker
EMBEDDING_BATCH_SIZE=100
MAX_RETRIES=3

# Visualizations Worker (Python)
UMAP_N_NEIGHBORS=15
HDBSCAN_MIN_CLUSTER_SIZE=10
```

See individual component READMEs for complete configuration reference.

## Running Tests

### Rust Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p semantic-explorer-core

# Run with logging
RUST_LOG=debug cargo test -- --nocapture
```

### Python Tests

```bash
cd crates/worker-visualizations-py
python -m pytest tests/
```

### UI Tests

```bash
cd semantic-explorer-ui
npm test
```

## Observability

### Metrics

Prometheus metrics exposed on `/metrics` endpoint for each service:

- **HTTP Metrics**: Request count, duration, status codes
- **Database Metrics**: Query duration, connection pool stats
- **Worker Metrics**: Job count, duration, success/failure rates
- **Search Metrics**: Query latency, embedder performance
- **NATS Metrics**: Message backlog, consumer lag

Access Prometheus: http://localhost:9090

### Dashboards

Pre-configured Grafana dashboards:

1. **Service Operations** - API/worker throughput and latency
2. **Infrastructure** - Database, NATS, storage health
3. **Audit Events** - User activity and security events
4. **Logs & Traces** - Quickwit log aggregation and trace analysis

Access Grafana: http://localhost:3000 (admin/admin)

### Tracing

Distributed traces available in:
- **Jaeger UI**: http://localhost:16686
- **Quickwit**: http://localhost:7280

### Logging

Structured JSON logs aggregated in Quickwit:
- Query logs by service, level, request ID
- Full-text search across all logs
- Integrated with traces via trace/span IDs

## API Documentation

Interactive OpenAPI/Swagger documentation available at:

```
http://localhost:8080/swagger-ui
```

Key endpoints:

- `POST /collections` - Create collection
- `POST /collections/{id}/files` - Upload files
- `POST /collection_transforms` - Start extraction job
- `POST /dataset_transforms` - Start embedding job
- `POST /visualization_transforms` - Create visualization
- `POST /search` - Hybrid search
- `POST /chat` - RAG chat (SSE streaming)
- `GET /health/ready` - Readiness probe
- `GET /health/live` - Liveness probe

## Security

### Authentication

Optional OIDC (OpenID Connect) authentication:

```bash
# Enable OIDC
OIDC_ENABLED=true
OIDC_ISSUER_URL=https://dex.example.com
OIDC_CLIENT_ID=semantic-explorer
OIDC_REDIRECT_URL=http://localhost:8080/auth/callback
```

Development includes Dex OIDC provider in Docker Compose.

### Encryption

- **Secrets Encryption**: AES-256-GCM encryption for sensitive configuration values
- **TLS**: HTTPS support with custom certificates
- **API Keys**: Secure storage of embedder and LLM API keys

### Audit Logging

All API actions logged to PostgreSQL `audit_events` table:
- User identity (OIDC subject)
- Action type (CREATE, UPDATE, DELETE)
- Resource type and ID
- Timestamp and IP address

## Performance

### Benchmarks

Typical performance on 8-core CPU, 16GB RAM:

- **File Processing**: 100-500 files/minute (varies by format)
- **Embedding Generation**: 1000-5000 chunks/minute (batch size 100)
- **Search Latency**: <100ms p95 (10K vectors)
- **API Throughput**: 1000+ req/s (simple GET endpoints)

### Scaling

**Horizontal Scaling** (Kubernetes):
```yaml
# Scale workers
kubectl scale statefulset worker-datasets --replicas=5

# Auto-scaling
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: worker-datasets
spec:
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

**Vertical Scaling**:
- Increase worker concurrency via `WORKER_CONCURRENCY`
- Tune batch sizes for embedding workers
- Adjust database connection pool size

## Troubleshooting

### Common Issues

**Worker not processing jobs:**
```bash
# Check NATS stream status
docker exec -it nats nats stream ls
docker exec -it nats nats consumer ls COLLECTION_TRANSFORMS

# Check worker logs
docker logs worker-collections -f
```

**Embedding failures:**
```bash
# Check embedder configuration
curl http://localhost:8080/api/embedders

# Test embedder directly
curl -X POST http://localhost:8090/embed \
  -H "Content-Type: application/json" \
  -d '{"input": "test"}'
```

**Search returning no results:**
```bash
# Check Qdrant collection
curl http://localhost:6333/collections

# Check embedded dataset status
curl http://localhost:8080/api/embedded_datasets
```

### Debug Mode

Enable detailed logging:

```bash
# Rust services
RUST_LOG=debug,semantic_explorer=trace

# Python worker
LOG_LEVEL=DEBUG
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines

- Run `cargo fmt` and `cargo clippy` before committing Rust code
- Run `black` and `ruff` for Python code
- Run `npm run format` for UI code
- Add tests for new features
- Update documentation

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## Support

- **Issues**: https://github.com/yourusername/semantic-explorer/issues
- **Discussions**: https://github.com/yourusername/semantic-explorer/discussions
- **Documentation**: https://docs.example.com (if applicable)

## Acknowledgments

Built with:
- [Actix-web](https://actix.rs/) - Rust web framework
- [FastEmbed](https://github.com/Anush008/fastembed-rs) - Fast embedding library
- [mistral.rs](https://github.com/EricLBuehler/mistral.rs) - LLM inference engine
- [UMAP](https://umap-learn.readthedocs.io/) - Dimensionality reduction
- [HDBSCAN](https://hdbscan.readthedocs.io/) - Clustering algorithm
- [Qdrant](https://qdrant.tech/) - Vector database
- [NATS](https://nats.io/) - Message queue
- [Svelte](https://svelte.dev/) - Frontend framework
- [OpenTelemetry](https://opentelemetry.io/) - Observability framework
