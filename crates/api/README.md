# API Crate - Semantic Explorer REST Server

Production-grade REST API server built with Actix-web providing comprehensive document management, semantic search, and RAG chat capabilities with enterprise security features.

## 📋 Overview

The `semantic-explorer` API crate is the core HTTP server that orchestrates the entire platform. It handles:

### Core Features
- 🔐 **OIDC Authentication** - OAuth2/OIDC integration with automatic token refresh and multi-session support
- 📁 **Collection Management** - Create, update, delete document collections with hierarchical organization
- 📊 **Dataset Operations** - Build structured datasets with chunking, deduplication, and metadata extraction
- 🔍 **Semantic Search** - Vector similarity search with quantized embeddings, metadata filtering, and result ranking
- 💬 **RAG Chat** - Chat with document collections using retrieval-augmented generation (RAG)
- 🎯 **Multi-LLM Support** - Compare results across OpenAI, Anthropic, local models
- 🔧 **Embedder Configuration** - Manage multiple embedding models and providers (OpenAI, Cohere, local)
- 📝 **Transform Orchestration** - Coordinate document extraction, embedding, visualization pipelines
- 📡 **Real-time Updates** - Server-Sent Events (SSE) for monitoring transform progress
- 📊 **Comprehensive Audit Logging** - Immutable audit trail of all operations via NATS
- 📈 **OpenAPI Documentation** - Auto-generated API docs at `/api/openapi.json`

### Enterprise Features  
- 🛡️ **Row-Level Security** - Database-enforced access control via PostgreSQL RLS
- 🌐 **HTTP Caching** - ETag and conditional request support (If-None-Match, If-Modified-Since)
- ⚡ **Rate Limiting** - Token-bucket algorithm per user via Redis
- 🔐 **Encryption** - AES-256 encryption for API keys at rest (optional)
- 🔄 **Idempotency** - Idempotent request handling via Redis deduplication
- 📊 **Prometheus Metrics** - Real-time metrics export for monitoring (error rates, latency, costs)
- 🔍 **Distributed Tracing** - OpenTelemetry integration for end-to-end tracing
- 🏗️ **High Availability** - Connection pooling, async workers, horizontal scaling

## 🏗️ Architecture

```mermaid
graph TB
    subgraph "External"
        USER[Users<br/>Browser/CLI]
        OIDC[OIDC Provider<br/>Dex/Keycloak]
        EMBEDDER[Embedder APIs<br/>OpenAI/Cohere]
        LLM[LLM APIs<br/>OpenAI/Cohere/Anthropic]
    end

    subgraph "semantic-explorer API"
        HTTP[HTTP Layer<br/>Actix-web]
        AUTH[Authentication<br/>OIDC Middleware]
        API[API Handlers]
        AUDIT[Audit<br/>Logging]

        subgraph "Domain Modules"
            COLL[Collections]
            DS[Datasets]
            EMB[Embedders]
            LLMS[LLMs]
            SEARCH[Search]
            CHAT[Chat/RAG]
            TRANS[Transforms]
        end

        subgraph "Storage Layer"
            PG[PostgreSQL<br/>Adapter]
            S3[S3 Storage<br/>Adapter]
            QD[Qdrant<br/>Adapter]
            NT[NATS<br/>Publisher]
        end
    end

    subgraph "Infrastructure"
        POSTGRES[(PostgreSQL)]
        MINIO[(S3/MinIO)]
        QDRANT[(Qdrant)]
        NATS[(NATS JetStream)]
    end

    USER --> HTTP
    HTTP --> AUTH
    AUTH --> OIDC
    AUTH --> API
    API --> AUDIT
    API --> COLL & DS & EMB & LLMS & SEARCH & CHAT & TRANS

    COLL & DS --> PG & S3
    EMB & LLMS --> PG
    SEARCH --> QD & EMBEDDER
    CHAT --> QD & LLM
    TRANS --> NT

    PG --> POSTGRES
    S3 --> MINIO
    QD --> QDRANT
    NT --> NATS
```

## Request Processing Flow

```mermaid
sequenceDiagram
    participant Client
    participant Actix as Actix-web
    participant Auth as OIDC Auth
    participant Middleware as Request ID
    participant Handler as API Handler
    participant Audit as Audit Logger
    participant DB as PostgreSQL
    participant Store as S3/Qdrant

    Client->>Actix: HTTP Request
    Actix->>Middleware: Add Request ID
    Middleware->>Auth: Validate Session
    Auth->>Auth: Check Token/Cookie

    alt Unauthenticated
        Auth-->>Client: 401 Unauthorized
    else Authenticated
        Auth->>Handler: User Context
        Handler->>DB: Query/Mutate
        Handler->>Store: Read/Write
        Handler->>Audit: Log Event
        Handler-->>Client: JSON Response
    end
```

## Technologies

| Technology | Version | Purpose |
|------------|---------|---------|
| Rust | 2024 Edition | Language |
| actix-web | 4.12 | HTTP framework |
| actix-web-openidconnect | workspace | OIDC authentication |
| sqlx | 0.8 | PostgreSQL async client |
| qdrant-client | 1.16 | Vector database client |
| aws-sdk-s3 | 1.119 | S3-compatible storage |
| async-nats | 0.45 | Message queue client |
| utoipa | 5.4 | OpenAPI documentation |
| opentelemetry | workspace | Distributed tracing |
| rustls | 0.23 | TLS support |

## Module Structure

| Module | Description |
|--------|-------------|
| `api/` | HTTP endpoint handlers |
| `auth/` | OIDC authentication integration |
| `audit` | Security event logging |
| `storage/postgres/` | Database queries and migrations |
| `storage/rustfs/` | S3 file operations |
| `storage/qdrant/` | Vector database operations |
| `transforms/` | Job orchestration and result listeners |
| `collections/` | Collection domain logic |
| `datasets/` | Dataset domain logic |
| `embedders/` | Embedder configuration |
| `llms/` | LLM provider configuration |
| `search/` | Semantic search implementation |
| `chat/` | RAG chat implementation |
| `embedding/` | Embedding generation |
| `errors` | Unified error types |
| `middleware` | Request ID middleware |
| `observability` | Prometheus metrics endpoint |

## API Endpoints

### Authentication
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/auth_callback` | OIDC callback handler |
| GET | `/api/user/current` | Get current authenticated user |

### Collections
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/collections` | List user's collections |
| POST | `/api/collections` | Create collection |
| GET | `/api/collections/{id}` | Get collection details |
| PATCH | `/api/collections/{id}` | Update collection |
| DELETE | `/api/collections/{id}` | Delete collection |
| POST | `/api/collections/{id}/upload` | Upload file to collection |
| GET | `/api/collections/{id}/files` | List collection files |
| GET | `/api/collections/{id}/files/{fileId}` | Download file |
| DELETE | `/api/collections/{id}/files/{fileId}` | Delete file |
| GET | `/api/collections/{id}/search` | Search within collection |

### Datasets
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/datasets` | List user's datasets |
| POST | `/api/datasets` | Create dataset |
| GET | `/api/datasets/{id}` | Get dataset details |
| PATCH | `/api/datasets/{id}` | Update dataset |
| DELETE | `/api/datasets/{id}` | Delete dataset |
| GET | `/api/datasets/{id}/items` | List dataset items |
| GET | `/api/datasets/{id}/items/summary` | Dataset items summary |
| GET | `/api/datasets/{id}/items/{itemId}/chunks` | Get item chunks |
| GET | `/api/datasets/{id}/embedders` | Get assigned embedders |

### Embedders
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/embedders` | List user's embedders |
| POST | `/api/embedders` | Create embedder config |
| GET | `/api/embedders/{id}` | Get embedder details |
| PATCH | `/api/embedders/{id}` | Update embedder |
| DELETE | `/api/embedders/{id}` | Delete embedder |
| POST | `/api/embedders/{id}/test` | Test embedder connection |

### LLMs
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/llms` | List user's LLM configs |
| POST | `/api/llms` | Create LLM config |
| GET | `/api/llms/{id}` | Get LLM details |
| PATCH | `/api/llms/{id}` | Update LLM |
| DELETE | `/api/llms/{id}` | Delete LLM |

### Collection Transforms
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/collection-transforms` | List transforms |
| POST | `/api/collection-transforms` | Create transform |
| GET | `/api/collection-transforms/{id}` | Get transform |
| PATCH | `/api/collection-transforms/{id}` | Update transform |
| DELETE | `/api/collection-transforms/{id}` | Delete transform |
| POST | `/api/collection-transforms/{id}/trigger` | Manually trigger |
| GET | `/api/collection-transforms/{id}/stats` | Get statistics |
| GET | `/api/collection-transforms/{id}/files` | List processed files |
| GET | `/api/collection-transforms/{id}/stream` | SSE status stream |

### Dataset Transforms
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/dataset-transforms` | List transforms |
| POST | `/api/dataset-transforms` | Create transform |
| GET | `/api/dataset-transforms/{id}` | Get transform |
| PATCH | `/api/dataset-transforms/{id}` | Update transform |
| DELETE | `/api/dataset-transforms/{id}` | Delete transform |
| POST | `/api/dataset-transforms/{id}/trigger` | Manually trigger |
| GET | `/api/dataset-transforms/{id}/stats` | Get statistics |
| GET | `/api/dataset-transforms/{id}/detailed-stats` | Detailed metrics |
| GET | `/api/dataset-transforms/{id}/batches` | List batches |
| GET | `/api/dataset-transforms/{id}/stream` | SSE status stream |

### Embedded Datasets
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/embedded-datasets` | List embedded datasets |
| GET | `/api/embedded-datasets/{id}` | Get details |
| PATCH | `/api/embedded-datasets/{id}` | Update |
| DELETE | `/api/embedded-datasets/{id}` | Delete |
| GET | `/api/embedded-datasets/{id}/stats` | Embedding statistics |
| GET | `/api/embedded-datasets/{id}/points` | List vector points |
| GET | `/api/embedded-datasets/{id}/points/{pointId}/vector` | Get point vector |

### Visualization Transforms
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/visualization-transforms` | List transforms |
| POST | `/api/visualization-transforms` | Create transform |
| GET | `/api/visualization-transforms/{id}` | Get transform |
| PATCH | `/api/visualization-transforms/{id}` | Update transform |
| DELETE | `/api/visualization-transforms/{id}` | Delete transform |
| POST | `/api/visualization-transforms/{id}/trigger` | Manually trigger |
| GET | `/api/visualization-transforms/{id}/stream` | SSE status stream |
| GET | `/api/visualizations` | List visualizations |
| GET | `/api/visualizations/{id}` | Get visualization |
| GET | `/api/visualizations/{id}/download` | Download HTML |

### Search
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/search` | Semantic search across datasets |

### Chat
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/chat/sessions` | Create chat session |
| GET | `/api/chat/sessions` | List user's sessions |
| GET | `/api/chat/sessions/{id}` | Get session details |
| DELETE | `/api/chat/sessions/{id}` | Delete session |
| GET | `/api/chat/sessions/{id}/messages` | Get conversation |
| POST | `/api/chat/sessions/{id}/messages` | Send message |
| POST | `/api/chat/sessions/{id}/messages/stream` | Stream response (SSE) |
| POST | `/api/chat/messages/{id}/regenerate` | Regenerate response |

### Authentication Sessions
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/auth/sessions` | List user's active login sessions |
| DELETE | `/api/auth/sessions/{session_id}` | Revoke specific session |
| DELETE | `/api/auth/sessions` | Revoke all sessions (logout everywhere) |

### Marketplace
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/marketplace/collections` | Browse public collections |
| GET | `/api/marketplace/datasets` | Browse public datasets |
| GET | `/api/marketplace/embedders` | Browse public embedders |
| GET | `/api/marketplace/llms` | Browse public LLMs |
| POST | `/api/marketplace/{type}/{id}/grab` | Clone to user's account |

### Health & Observability
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Basic health check |
| GET | `/health/live` | Liveness probe |
| GET | `/health/ready` | Readiness probe |
| GET | `/metrics` | Prometheus metrics |
| GET | `/swagger-ui/` | OpenAPI documentation |

## Environment Variables

### Authentication (OIDC)

| Variable | Type | Description |
|----------|------|-------------|
| `OIDC_CLIENT_ID` | string | OAuth2 client ID |
| `OIDC_CLIENT_SECRET` | string | OAuth2 client secret |
| `OIDC_ISSUER_URL` | string | OAuth2 issuer URL |
| `OIDC_USE_PKCE` | boolean | Enable PKCE flow (default: false) |

### Session Management

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `OIDC_SESSION_MANAGEMENT_ENABLED` | boolean | `true` | Enable session management |
| `OIDC_SESSION_TIMEOUT_SECS` | integer | `3600` | Session expiration (seconds) |
| `OIDC_INACTIVITY_TIMEOUT_SECS` | integer | `1800` | Inactivity timeout (seconds) |
| `OIDC_MAX_CONCURRENT_SESSIONS` | integer | `5` | Max sessions per user |
| `OIDC_REFRESH_TOKEN_ROTATION_ENABLED` | boolean | `true` | Enable refresh token rotation |

### Server Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `HOSTNAME` | string | `localhost` | Bind hostname |
| `PORT` | integer | `8080` | Listen port |
| `STATIC_FILES_DIR` | string | `./semantic-explorer-ui/` | Static files directory |
| `CORS_ALLOWED_ORIGINS` | string | `` | Comma-separated CORS origins |
| `SHUTDOWN_TIMEOUT_SECS` | integer | `30` | Graceful shutdown timeout |

### Database

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `DATABASE_URL` | string | **required** | PostgreSQL connection string |
| `DB_MAX_CONNECTIONS` | integer | `15` | Max connection pool size |
| `DB_MIN_CONNECTIONS` | integer | `2` | Min connection pool size |
| `DB_ACQUIRE_TIMEOUT_SECS` | integer | `30` | Connection acquisition timeout |
| `DB_IDLE_TIMEOUT_SECS` | integer | `300` | Idle connection timeout |
| `DB_MAX_LIFETIME_SECS` | integer | `1800` | Max connection lifetime |

### Storage (S3)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `AWS_REGION` | string | **required** | S3 region |
| `AWS_ACCESS_KEY_ID` | string | **required** | S3 access key |
| `AWS_SECRET_ACCESS_KEY` | string | **required** | S3 secret key |
| `AWS_ENDPOINT_URL` | string | **required** | S3 endpoint URL |
| `S3_BUCKET_NAME` | string | **required** | S3 bucket for files |
| `MAX_FILE_SIZE_MB` | integer | `100` | Max file size for processing |

### Vector Database (Qdrant)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `QDRANT_URL` | string | `http://localhost:6334` | Qdrant server URL |
| `QDRANT_API_KEY` | string | *optional* | Qdrant API key |
| `QDRANT_TIMEOUT_SECS` | integer | `30` | Request timeout |
| `QDRANT_CONNECT_TIMEOUT_SECS` | integer | `10` | Connection timeout |
| `QDRANT_QUANTIZATION_TYPE` | string | `none` | `none`, `scalar`, or `product` |

### Message Queue (NATS)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `NATS_URL` | string | `nats://localhost:4222` | NATS server URL |
| `NATS_REPLICAS` | integer | `3` | JetStream replication factor |

### Redis Cluster

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `REDIS_CLUSTER_NODES` | string | *required if rate limiting enabled* | Comma-separated Redis nodes |
| `REDIS_POOL_SIZE` | integer | `10` | Connection pool size |
| `REDIS_CONNECT_TIMEOUT_SECS` | integer | `5` | Connection timeout |

### Rate Limiting

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `RATE_LIMIT_ENABLED` | boolean | `true` | Enable rate limiting |
| `RATE_LIMIT_DEFAULT_REQUESTS_PER_MINUTE` | integer | `1000` | Default rate limit |
| `RATE_LIMIT_SEARCH_REQUESTS_PER_MINUTE` | integer | `600` | Search rate limit |
| `RATE_LIMIT_CHAT_REQUESTS_PER_MINUTE` | integer | `200` | Chat rate limit |
| `RATE_LIMIT_TRANSFORM_REQUESTS_PER_MINUTE` | integer | `100` | Transform rate limit |
| `RATE_LIMIT_TEST_REQUESTS_PER_MINUTE` | integer | `100` | Test endpoint rate limit |

### Encryption (Optional)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ENCRYPTION_MASTER_KEY` | string | *optional* | 256-bit hex-encoded key for encrypting API keys (64 hex chars) |

### TLS/SSL (Optional)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SERVER_SSL_ENABLED` | boolean | `false` | Enable server TLS |
| `TLS_SERVER_CERT_PATH` | string | *conditional* | Server certificate path (PEM) |
| `TLS_SERVER_KEY_PATH` | string | *conditional* | Server private key path (PEM) |
| `CLIENT_MTLS_ENABLED` | boolean | `false` | Enable client mTLS for outbound requests |
| `TLS_CLIENT_CERT_PATH` | string | *conditional* | Client certificate path (PEM) |
| `TLS_CLIENT_KEY_PATH` | string | *conditional* | Client private key path (PEM) |
| `TLS_CA_CERT_PATH` | string | `/app/certs/ca-bundle.crt` | CA certificate bundle path |

### Observability

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SERVICE_NAME` | string | `semantic-explorer` | Service name for telemetry |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | string | `http://localhost:4317` | OTLP collector endpoint |
| `LOG_FORMAT` | string | `json` | `json` or `pretty` |
| `RUST_LOG` | string | `info` | Log level filter (e.g., `semantic_explorer=debug`) |


## Observability

### Prometheus Metrics

Access metrics at `/metrics`. Key metrics include:

- `http_requests_total` - HTTP requests by method, path, status
- `http_request_duration_seconds` - Request latency histogram
- `database_query_duration_seconds` - Database query latency
- `storage_operation_duration_seconds` - S3 operation latency
- `search_request_duration_seconds` - Search latency
- `sse_connections_active` - Active SSE connections

### Distributed Tracing

The API exports traces to an OTLP-compatible collector. Each request includes:
- Request ID in response header (`x-request-id`)
- Trace context propagation
- Span attributes for database, storage, and external API calls

### Health Checks

```bash
# Liveness probe (is the process running?)
curl http://localhost:8080/health/live

# Readiness probe (is the service ready to accept traffic?)
curl http://localhost:8080/health/ready
```

### Audit Logging

All security-relevant events are logged to the `audit_events` table:

| Event Type | Description |
|------------|-------------|
| `resource_created` | Resource created |
| `resource_read` | Resource accessed |
| `resource_updated` | Resource modified |
| `resource_deleted` | Resource deleted |
| `file_uploaded` | File uploaded |
| `search_executed` | Search performed |
| `chat_message_sent` | Chat message sent |

## Database Schema

The API manages the following PostgreSQL tables:

- `collections` - Document collections
- `datasets` - Structured datasets
- `dataset_items` - Individual dataset items with chunks
- `embedders` - Embedder configurations
- `llms` - LLM provider configurations
- `collection_transforms` - File extraction pipelines
- `collection_transform_files` - Processed files tracking
- `dataset_transforms` - Embedding pipelines
- `dataset_transform_batches` - Batch processing tracking
- `embedded_datasets` - Vector collection metadata
- `visualization_transforms` - Visualization pipelines
- `visualization_outputs` - Generated visualizations
- `chat_sessions` - Chat sessions
- `chat_messages` - Chat messages
- `chat_retrieved_documents` - RAG retrieved documents
- `audit_events` - Security audit trail

## Running

### Development

```bash
# Set required environment variables
export DATABASE_URL="postgres://user:pass@localhost:5432/semantic_explorer"
export OIDC_CLIENT_ID="your-client-id"
export OIDC_CLIENT_SECRET="your-client-secret"
export OIDC_ISSUER_URL="http://localhost:5556"
export AWS_REGION="us-east-1"
export AWS_ACCESS_KEY_ID="minioadmin"
export AWS_SECRET_ACCESS_KEY="minioadmin"
export AWS_ENDPOINT_URL="http://localhost:9000"

# Run the server
cargo run --bin semantic-explorer
```

### Docker

```bash
docker run -p 8080:8080 \
  -e DATABASE_URL="..." \
  -e OIDC_CLIENT_ID="..." \
  # ... other environment variables
  ghcr.io/your-org/semantic-explorer:latest
```

### Kubernetes

See the Helm chart in `/deployment/helm/semantic-explorer/`.

## Security Features

- **Authentication**: OpenID Connect with session management
- **Authorization**: Owner-based resource access control
- **Audit Logging**: All security events logged to database
- **TLS Support**: Configurable server TLS and client mTLS
- **CORS**: Configurable cross-origin resource sharing
- **Input Validation**: Type-safe request validation
- **SQL Injection Prevention**: Parameterized queries via sqlx
- **API Key Encryption**: AES-256-GCM encryption at rest for API keys

## API Key Encryption

All API keys for embedders and LLMs are encrypted at rest using AES-256-GCM encryption.

### How It Works

1. **User provides API key** via POST `/api/embedders` or POST `/api/llms`:
   ```json
   {
     "name": "My OpenAI Config",
     "provider": "openai",
     "api_key": "sk-proj-abc123..."
   }
   ```

2. **Server encrypts the key**:
   - Uses master key from `ENCRYPTION_MASTER_KEY` environment variable
   - Generates random 12-byte nonce
   - Encrypts plaintext using AES-256-GCM
   - Stores as base64: `base64(nonce || ciphertext)`

3. **Key is never exposed**:
   - Client never receives the plaintext key
   - When needed, server decrypts internally before calling external APIs
   - Audit logs don't contain plaintext keys

### Encryption Master Key Setup

**Required for production deployment**:

```bash
# Generate a new 256-bit key (run once)
openssl rand -hex 32
# Output: a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a

# Set as environment variable
export ENCRYPTION_MASTER_KEY="a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a"
```

**Recommended**:
- Store the key in AWS Secrets Manager, HashiCorp Vault, or similar
- Never commit to version control
- Rotate periodically and implement key rotation migrations
- Restrict access to secret management system

### Database Schema

API keys are stored in the `api_key_encrypted` column of embedders and llms tables:

```sql
-- embedders table
CREATE TABLE embedders (
  ...
  api_key_encrypted TEXT, -- base64(nonce || ciphertext) from AES-256-GCM
  ...
)

-- llms table
CREATE TABLE llms (
  ...
  api_key_encrypted TEXT, -- base64(nonce || ciphertext) from AES-256-GCM
  ...
)
```

### Development Mode

If `ENCRYPTION_MASTER_KEY` is not set, the API will:
1. Log a warning that encryption is disabled
2. Still start successfully (for development)
3. In production, this will cause startup failure (recommended)

To enable encryption in development:
```bash
export ENCRYPTION_MASTER_KEY=$(openssl rand -hex 32)
```

## License

See LICENSE file in repository root.
