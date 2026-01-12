⚠️ **This project is under development, check back later!**

---

# Semantic Explorer

Production-grade semantic exploration platform with advanced caching, real-time monitoring, and enterprise-grade security features.

## 🎯 Core Features

### Document Processing
- 📄 **Multi-format Support** - PDF, Microsoft Office, OpenDocument, HTML, XML, plain text
- 🔄 **Async Job Processing** - Background workers handle extraction, embedding, visualization via NATS JetStream
- 📊 **Structured Datasets** - Automatic chunking, deduplication, metadata extraction
- 🎯 **Custom Transforms** - Collection, Dataset, and Visualization pipeline stages

### Search & Discovery
- 🔍 **Semantic Search** - Vector similarity with Qdrant, metadata filtering, side-by-side model comparison
- 📈 **UMAP/HDBSCAN Clustering** - produce visualizations of embedding spaces
- 🧠 **Multi-LLM Support** - Compare results across Cohere, OpenAI, Anthropic, etc.

### Enterprise Security
- 🔐 **OIDC Authentication** - OpenID Connect with Dex integration, automatic token refresh
- 🛡️ **Row-Level Security (RLS)** - Database-level access control via PostgreSQL policies
- 🔒 **End-to-End Encryption** - AES-256 encryption for sensitive data at rest
- 📝 **Comprehensive Audit Logging** - All operations logged to audit trail with immutable records via NATS

### Database & Storage
- 🗄️ **PostgreSQL Database** - Robust relational database for metadata and state
- 📦 **S3-compatible Storage** - AWS S3, MinIO, or any S3-compatible provider
- 🔴 **Redis Cluster** - Rate limiting, session management, and request deduplication
- 📍 **Qdrant Vector DB** - Production-grade vector search with quantization (product/scalar)

### Observability & Monitoring
- 📊 **Prometheus Metrics** - Real-time metrics collection (error rates, latency, throughput)
- 📈 **Grafana Dashboards** - Pre-configured dashboards for API, workers, and infrastructure
- 🔍 **OpenTelemetry Tracing** - Distributed tracing across all services

### Performance Optimizations
- ⚙️ **Connection Pooling** - Tuned for high concurrency with prepared statement caching
- 🎯 **Quantized Embeddings** - Product quantization for 10x faster nearest-neighbor search
- 🔄 **HTTP Caching** - ETag-based cache validation, conditional requests
- 🔄 **Request Deduplication** - Prevents duplicate processing of identical requests via Redis

### Session Management
- 👤 **Multi-session Support** - Multiple concurrent sessions per user with limits
- 🔄 **Token Rotation** - Refresh token rotation for enhanced security
- ⏱️ **Configurable Timeouts** - Session expiration and inactivity timeouts

## 🚀 Quick Start

### Prerequisites
- Docker & Docker Compose
- PostgreSQL 14+ (or use Docker)
- Redis 7+ Cluster mode (or use Docker)
- Qdrant 1.8+ (or use Docker)
- Rust 1.75+ (for local development)
- Node.js 18+ (for UI development)

### Development Setup

```bash
# Clone repository
git clone <repo-url>
cd semantic-explorer

# Copy environment template
cp crates/api/.env.example crates/api/.env
# Edit crates/api/.env with your configuration

# Start infrastructure (PostgreSQL, Redis, Qdrant, NATS, etc.)
cd deployment/compose
docker-compose -f compose.dev.yaml up -d

# Start API server (Terminal 1)
cd ../../crates/api
cargo run

# Start UI (Terminal 2)
cd ../../semantic-explorer-ui
npm install
npm run dev

# Start worker services (Terminal 3, 4, 5)
# Terminal 3: Collections worker
cd ../../crates/worker-collections
cargo run

# Terminal 4: Datasets worker  
cd ../../crates/worker-datasets
cargo run

# Terminal 5: Visualizations worker
cd ../../crates/worker-visualizations-py
source venv/bin/activate
pip install -r requirements.txt
python src/main.py
```

### Access Services
- **API**: http://localhost:8000 (API docs at `/api/openapi.json`)
- **UI**: http://localhost:5173
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (default: admin/admin)
- **Qdrant**: http://localhost:6334

### Component Documentation
- [API Crate](./crates/api/README.md) - REST API, middlewares, auth
- [Core Library](./crates/core/README.md) - Shared utilities, config, encryption
- [Collections Worker](./crates/worker-collections/README.md) - Document extraction
- [Datasets Worker](./crates/worker-datasets/README.md) - Embedding generation
- [Visualizations Worker](./crates/worker-visualizations-py/README.md) - UMAP clustering
- [UI](./semantic-explorer-ui/README.md) - Frontend implementation

### Deployment
- [Deployment Guide](./deployment/DEPLOYMENT_GUIDE.md) - Production deployment steps
- [Docker Compose](./deployment/compose/compose.yaml) - Infrastructure as code
- [Helm Charts](./deployment/helm/semantic-explorer/Chart.yaml) - Kubernetes deployment

## 🏗️ Project Structure

```
semantic-explorer/
├── crates/
│   ├── api/                    # REST API server (Actix-web)
│   │   ├── src/
│   │   │   ├── api/           # HTTP endpoints & handlers
│   │   │   ├── auth/          # OIDC authentication
│   │   │   ├── chat/          # LLM chat endpoints
│   │   │   ├── collections/   # Collection management
│   │   │   ├── datasets/      # Dataset operations
│   │   │   ├── embedding/     # Embedding generation
│   │   │   ├── embedders/     # LLM model drivers
│   │   │   ├── llms/          # LLM integrations
│   │   │   ├── search/        # Semantic search
│   │   │   ├── storage/       # Database & S3 layers
│   │   │   ├── transforms/    # Pipeline transforms
│   │   │   ├── middleware/    # Auth, caching, rate limiting
│   │   │   ├── audit.rs       # Audit logging
│   │   │   └── main.rs        # Server entry point
│   │   └── Dockerfile         # Container image
│   │
│   ├── core/                   # Shared library
│   │   ├── config.rs          # Configuration management
│   │   ├── encryption.rs      # AES-256 encryption
│   │   ├── http_client.rs     # HTTP utilities
│   │   ├── models.rs          # Domain models
│   │   ├── nats.rs            # NATS client
│   │   ├── storage.rs         # S3 client
│   │   ├── observability.rs   # OpenTelemetry setup
│   │   └── worker.rs          # Worker patterns
│   │
│   ├── worker-collections/     # Document extraction worker
│   │   ├── extract/           # Document parsing
│   │   └── chunk/             # Text chunking
│   │
│   ├── worker-datasets/        # Embedding generation worker
│   │   └── embedder.rs        # Embedding logic
│   │
│   └── worker-visualizations-py/  # Python UMAP worker
│       ├── processor.py        # Clustering logic
│       ├── storage.py          # Result persistence
│       └── llm_namer.py        # LLM naming service
│
├── semantic-explorer-ui/       # Svelte frontend
│   └── src/
│       ├── lib/               # Shared components
│       ├── App.svelte         # Root component
│       └── main.ts            # Entry point
│
├── deployment/
    ├── compose/               # Docker Compose configs
    ├── helm/                  # Kubernetes Helm charts
    └── DEPLOYMENT_GUIDE.md    # Deployment instructions

```

## 📊 Technology Stack

### Backend
- **Language**: Rust 1.75+
- **Web Framework**: Actix-web (async HTTP)
- **Database**: PostgreSQL 14+ with RLS & replication
- **Vector DB**: Qdrant (quantized embeddings)
- **Cache**: Redis Cluster
- **Message Queue**: NATS JetStream
- **Authentication**: OIDC (Dex)
- **Storage**: S3-compatible (AWS S3, MinIO)
- **Observability**: OpenTelemetry, Prometheus

### Frontend
- **Framework**: Svelte 5
- **Build Tool**: Vite
- **Language**: TypeScript
- **Styling**: Tailwind CSS

### DevOps
- **Containerization**: Docker
- **Orchestration**: Docker Compose (dev) / Kubernetes + Helm (prod)
- **Monitoring**: Prometheus + Grafana
- **Tracing**: Quickwit
- **CI/CD**: GitHub Actions

## 🔧 Configuration

### Environment Variables

**Database & Storage:**
```bash
DATABASE_URL=postgresql://user:pass@localhost:5432/db
REDIS_CLUSTER_NODES=redis-1:6379,redis-2:6379,...
QDRANT_URL=http://localhost:6334
QDRANT_QUANTIZATION_TYPE=product  # product, scalar, or none
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your-key
AWS_SECRET_ACCESS_KEY=your-secret
AWS_ENDPOINT_URL=http://minio:9000
S3_BUCKET_NAME=semantic-explorer-files
```

**Security & Auth:**
```bash
OIDC_CLIENT_ID=your-client-id
OIDC_CLIENT_SECRET=your-secret
OIDC_ISSUER_URL=https://dex.example.com
OIDC_USE_PKCE=false
OIDC_SESSION_MANAGEMENT_ENABLED=true
OIDC_MAX_CONCURRENT_SESSIONS=5
OIDC_REFRESH_TOKEN_ROTATION_ENABLED=true
```

**Session Management:**
```bash
OIDC_SESSION_MANAGEMENT_ENABLED=true    # Enable session management
OIDC_SESSION_TIMEOUT_SECS=3600          # Session expiration (1 hour)
OIDC_INACTIVITY_TIMEOUT_SECS=1800       # Inactivity timeout (30 minutes)
OIDC_MAX_CONCURRENT_SESSIONS=5          # Max sessions per user
OIDC_REFRESH_TOKEN_ROTATION_ENABLED=true  # Enable token rotation
```

**Security:**
```bash
ENCRYPTION_KEY=your-key                 # AES-256 encryption key
MAX_FILE_SIZE_MB=100                    # Max file size for processing (default: 100MB)
```

**Observability:**
```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
LOG_FORMAT=json
RUST_LOG=semantic_explorer=debug
SERVICE_NAME=semantic-explorer
```

See [.env.example](./crates/api/.env.example) for complete configuration options.

## 📊 Monitoring & Observability

### Prometheus Metrics

The API exports metrics at the configured PROMETHEUS_SCRAPE_PORT at `/metrics`:

- **Request Metrics**: Request counts, duration, latency percentiles
- **Error Metrics**: Error rates by endpoint and status code
- **Database Metrics**: Query performance, connection pool usage
- **Cache Metrics**: Cache hit/miss rates
- **Business Metrics**: Documents processed, embeddings generated

### Grafana Dashboards

The following dashboards are pre-configured:

1. **API & Workers** - Request metrics, error rates, latency, throughput across API and workers
2. **Infrastructure** - Database, Redis, Qdrant, NATS health and performance

Access Grafana at http://localhost:3000 (default: admin/admin)

### Distributed Tracing

Enable OpenTelemetry for end-to-end tracing:

```rust
// Traces are automatically collected and sent to Quickwit
// View in Quickwit UI at http://localhost:7280
```

Query trace data using Quickwit's query language for performance analysis.

## 🔐 Security Features

### Authentication & Authorization
- **OIDC Integration** - Secure authentication via Dex or any OIDC provider
- **JWT Tokens** - Secure token-based API access
- **Token Refresh** - Automatic refresh without user interaction
- **Multi-session Support** - Multiple concurrent user sessions

### Data Protection
- **Row-Level Security** - PostgreSQL RLS policies enforce user isolation
- **End-to-End Encryption** - AES-256 encryption for sensitive fields
- **Encrypted Storage** - S3 encryption at rest
- **Audit Logging** - Immutable audit trail of all operations

### Infrastructure Security
- **Rate Limiting** - Token-bucket algorithm via Redis
- **CORS Configuration** - Configurable cross-origin policies
- **HTTPS/TLS** - Full TLS support in production
- **Secrets Management** - Environment-based secret injection

## 🚀 Deployment

### Docker Compose (Development)

```bash
cd deployment/compose
docker-compose -f compose.dev.yaml up
```

Includes: PostgreSQL, Redis, Qdrant, NATS, Prometheus, Grafana, Quickwit, Dex

### Kubernetes (Production)

```bash
helm install semantic-explorer deployment/helm/semantic-explorer \
  --namespace semantic-explorer \
  --values values.yaml
```

Features: Auto-scaling, health checks, persistent volumes, network policies, RBAC

### Manual Deployment

See [DEPLOYMENT_GUIDE.md](./deployment/DEPLOYMENT_GUIDE.md) for detailed steps.


## 📝 Testing

### Unit Tests
```bash
cargo test --lib
```

### Integration Tests
```bash
cargo test --test '*' -- --test-threads=1
```

### E2E Tests
```bash
npm run test --prefix semantic-explorer-ui
```

## 🤝 Contributing

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Make changes and ensure all tests pass: `cargo test && npm test`
3. Format code: `cargo fmt`
4. Run linter: `cargo clippy`
5. Submit pull request with description

## 📄 License

See [LICENSE](./LICENSE) file for details.

## 🆘 Support

- **Issues**: GitHub Issues for bug reports and feature requests
- **Discussions**: GitHub Discussions for questions and ideas
---