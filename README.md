⚠️ **This project is under development, check back later!**

---

# Semantic Explorer

Production-grade semantic exploration platform with advanced caching, real-time monitoring, and enterprise-grade security features.

## 🎯 Core Features

### Document Processing
- 📄 **Multi-format Support** - PDF, Microsoft Office (modern and legacy), OpenDocument, HTML, XML, plain text
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
- 📍 **Qdrant Vector DB** - Production-grade vector search with quantization (product/scalar)

### Observability & Monitoring
- 📊 **Prometheus Metrics** - Real-time metrics collection (error rates, latency, throughput)
- 📈 **Grafana Dashboards** - Pre-configured dashboards for API, workers, and infrastructure
- 🔍 **OpenTelemetry Tracing** - Distributed tracing across all services

### Performance Optimizations
- ⚙️ **Connection Pooling** - Tuned for high concurrency with prepared statement caching
- 🎯 **Quantized Embeddings** - Product quantization for 10x faster nearest-neighbor search
- 🔄 **HTTP Caching** - ETag-based cache validation, conditional requests

### Session Management
- 👤 **Multi-session Support** - Multiple concurrent sessions per user with limits
- 🔄 **Token Rotation** - Refresh token rotation for enhanced security
- ⏱️ **Configurable Timeouts** - Session expiration and inactivity timeouts

## 🏗️ Architecture

### High-Level Overview

```mermaid
flowchart LR
    subgraph Clients
        UI[Web UI]
        API_Client[API Clients]
    end

    subgraph Core["Core Services"]
        API[API Server]
    end

    subgraph Workers["Background Workers"]
        W1[Collections]
        W2[Datasets]
        W3[Visualizations]
    end

    subgraph Queue
        NATS[NATS JetStream]
    end

    subgraph Data["Data Layer"]
        PG[(PostgreSQL)]
        QD[(Qdrant)]
        S3[(S3/MinIO)]
        RD[(NATS)]
    end

    Clients --> API
    API --> NATS
    API --> Data
    NATS --> Workers
    Workers --> Data
    Workers --> NATS
```

### Detailed Architecture

```mermaid
flowchart TB
    subgraph clients ["👥 CLIENTS"]
        direction LR
        web["🌐 Web UI<br/>Svelte"]
        ext["🔌 API Clients"]
    end

    subgraph ingress ["🚪 INGRESS"]
        ing["Load Balancer / Ingress Controller"]
    end

    subgraph auth ["🔐 AUTHENTICATION"]
        direction LR
        dex["Dex OIDC"]
        oidc["External IdP<br/>Google · GitHub · LDAP"]
    end

    subgraph api ["📦 API SERVER"]
        direction TB
        server["Actix-web REST API"]
        mw["Middleware Stack<br/>Auth · RateLimit · Cache · Audit"]
        server --- mw
    end

    subgraph workers ["⚙️ WORKERS"]
        direction LR
        wc["📄 Collections<br/>Document Extraction<br/><i>Rust</i>"]
        wd["🧠 Datasets<br/>Embeddings<br/><i>Rust</i>"]
        wv["📊 Visualizations<br/>UMAP · HDBSCAN<br/><i>Python</i>"]
    end

    subgraph queue ["📨 MESSAGE QUEUE"]
        nats["NATS JetStream"]
        streams["Streams: Collections · Datasets · Visualizations · Status · DLQ"]
        nats --- streams
    end

    subgraph storage ["💾 DATA STORAGE"]
        direction LR
        pg["🐘 PostgreSQL<br/>Metadata · RLS · Audit"]
        qd["🔴 Qdrant<br/>Vectors · Search"]
        s3["📦 S3 / MinIO<br/>Files · Artifacts"]
        rd["⚡ NATS<br/>Cache · Sessions"]
    end

    subgraph external ["🌍 EXTERNAL SERVICES"]
        direction LR
        emb["Embedding APIs<br/>OpenAI · Cohere"]
        llm["LLM APIs<br/>GPT-4 · Claude"]
    end

    subgraph observability ["📊 OBSERVABILITY"]
        direction LR
        otel["OTEL Collector"]
        prom["Prometheus"]
        qw["Quickwit"]
        graf["Grafana"]
    end

    %% Main flows
    clients --> ingress --> api
    api <--> auth
    auth <--> oidc
    api --> queue
    queue --> workers
    workers --> queue

    %% Storage connections
    api --> storage
    workers --> storage

    %% External API calls
    wd --> emb
    wv --> llm

    %% Observability (dashed)
    api -.-> otel
    workers -.-> otel
    otel --> prom & qw
    prom & qw --> graf
```

### Data Flow

```mermaid
flowchart LR
    subgraph ingest ["1️⃣ Ingest"]
        upload["Upload Document"]
    end
    
    subgraph extract ["2️⃣ Extract"]
        parse["Parse & Chunk"]
    end
    
    subgraph embed ["3️⃣ Embed"]
        vectors["Generate Vectors"]
    end
    
    subgraph viz ["4️⃣ Visualize"]
        cluster["UMAP + HDBSCAN"]
    end
    
    subgraph search ["5️⃣ Search"]
        query["Semantic Query"]
    end

    upload -->|"S3 + Queue"| extract
    extract -->|"PostgreSQL"| embed
    embed -->|"Qdrant"| viz
    viz -->|"S3"| search
    search -->|"Qdrant"| results["Results"]
```

| Step | Component | Input | Output | Storage |
|------|-----------|-------|--------|---------|
| 1 | API | Document file | Raw file | S3 |
| 2 | worker-collections | Raw file | Text chunks | PostgreSQL |
| 3 | worker-datasets | Text chunks | Vector embeddings | Qdrant |
| 4 | worker-visualizations-py | Embeddings | 2D layout + clusters | S3 |
| 5 | API | Query text | Ranked results | - |

## 🚀 Quick Start

### Prerequisites
- Docker & Docker Compose
- PostgreSQL 14+ (or use Docker)
- NATS 7+ Cluster mode (or use Docker)
- Qdrant 1.8+ (or use Docker)
- Rust 1.85+ (for local development)
- Node.js 20+ (for UI development)

### Development Setup

```bash
# Clone repository
git clone <repo-url>
cd semantic-explorer

# Copy environment template
cp crates/api/.env.example crates/api/.env
# Edit crates/api/.env with your configuration

# Start infrastructure (PostgreSQL, NATS, Qdrant, NATS, etc.)
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
- **Language**: Rust 1.85+
- **Web Framework**: Actix-web (async HTTP)
- **Database**: PostgreSQL 14+ with RLS & replication
- **Vector DB**: Qdrant (quantized embeddings)
- **Cache**: NATS JetStream
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

All services use environment variables for configuration. See `.env.example` files in each component directory for complete configuration options.

**Core Services:**
- API: [crates/api/.env.example](./crates/api/.env.example)
- Collections Worker: [crates/worker-collections/.env.example](./crates/worker-collections/.env.example)
- Datasets Worker: [crates/worker-datasets/.env.example](./crates/worker-datasets/.env.example)
- Visualizations Worker: [crates/worker-visualizations-py/.env.example](./crates/worker-visualizations-py/.env.example)

**Key Configuration Areas:**

**Database & Storage:**
```bash
DATABASE_URL=postgresql://user:pass@localhost:5432/db
QDRANT_URL=http://localhost:6334
AWS_REGION=us-east-1
AWS_ENDPOINT_URL=http://minio:9000
S3_BUCKET_NAME=semantic-explorer-files
```

**Authentication (OIDC):**
```bash
OIDC_CLIENT_ID=semantic-explorer-client
OIDC_CLIENT_SECRET=your-secret
OIDC_ISSUER_URL=http://localhost:5556
OIDC_SESSION_MANAGEMENT_ENABLED=true
OIDC_SESSION_TIMEOUT_SECS=3600
OIDC_MAX_CONCURRENT_SESSIONS=5
```

**Security:**
```bash
# Generate with: openssl rand -hex 32
ENCRYPTION_MASTER_KEY=your-64-char-hex-key
RATE_LIMIT_ENABLED=true
SERVER_SSL_ENABLED=false
CLIENT_MTLS_ENABLED=false
```

**Observability:**
```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
LOG_FORMAT=json
RUST_LOG=semantic_explorer=debug,actix_web=info
```

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
2. **Infrastructure** - Database, NATS, Qdrant, NATS health and performance

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
- **Rate Limiting** - Token-bucket algorithm via NATS
- **HTTPS/TLS** - Full TLS support in production
- **Secrets Management** - Environment-based secret injection

## 🚀 Deployment

### Docker Compose (Development)

```bash
cd deployment/compose
docker-compose -f compose.dev.yaml up
```

Includes: PostgreSQL, NATS, Qdrant, NATS, Prometheus, Grafana, Quickwit, Dex

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