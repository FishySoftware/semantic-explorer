# Semantic Explorer API Server

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![Actix-web](https://img.shields.io/badge/actix--web-4.x-blue.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**The main REST API server for Semantic Explorer**

</div>

Provides endpoints for collection management, dataset processing, embedding generation, search, chat, and visualizations.

---

## Overview

The API server orchestrates all system operations:

- **Collection & Dataset Management**: CRUD operations for data organization
- **Transform Orchestration**: Event-driven job dispatch to NATS workers
- **Embedding Visualizations**: 2D visualizations of vector embeddings using UMAP dimensionality reduction and HDBSCAN clustering
- **Search**: Vector search across embedded datasets
- **Chat**: Context-aware conversations with LLM integration
- **Real-time Updates**: Server-Sent Events (SSE) for transform progress
- **Authentication**: OIDC integration
- **Observability**: Prometheus metrics, OpenTelemetry tracing, structured logging
- **Reliability**: NATS-coordinated reconciliation for recovering missed work and failed batches
- **Circuit Breakers**: Automatic failure isolation for external services
- **Adaptive Workers**: Workers self-pace based on downstream 503 backpressure

---

## Architecture

```mermaid
graph TD
    subgraph "HTTP Layer"
        PROM_MW[Prometheus Metrics]
        CORS[CORS Middleware]
        SEC[Security Headers]
        COMP[Compression]
        AUTH[OIDC Auth Middleware]
        API[API Endpoints]
    end

    subgraph "Services"
        direction LR
        COLL[Collections]
        DS[Datasets]
        EMB[Embedders]
        TRANS[Transforms]
        SEARCH[Search]
        CHAT[Chat]
    end

    subgraph "Background Tasks"
        direction LR
        TRIGGER[Event-Driven Triggers]
        LISTENER[Result Listeners]
        AUDIT[Audit Consumer]
        RECON[Reconciliation<br/>NATS-coordinated]
    end

    subgraph "Infrastructure"
        direction LR
        PG[(PostgreSQL)]
        NATS[NATS JetStream]
        QD[(Qdrant)]
        S3[(S3/MinIO)]
        VK[(Valkey Cache)]
    end

    subgraph "Embedding Providers"
        direction LR
        INT_EMB[Internal<br/>embedding-inference-api]
        OAI_EMB[OpenAI]
        COH_EMB[Cohere]
    end

    subgraph "LLM Providers"
        direction LR
        INT_LLM[Internal<br/>llm-inference-api]
        OAI_LLM[OpenAI]
        COH_LLM[Cohere]
    end

    PROM_MW --> CORS --> SEC --> COMP --> AUTH --> API
    API --> COLL & DS & EMB & TRANS & SEARCH & CHAT

    COLL --> PG & S3 & VK
    DS --> PG & VK
    EMB --> PG & VK
    TRANS --> NATS & PG

    SEARCH --> QD & PG
    SEARCH --> INT_EMB
    SEARCH --> OAI_EMB
    SEARCH --> COH_EMB

    CHAT --> QD & EMB
    CHAT --> INT_LLM
    CHAT --> OAI_LLM
    CHAT --> COH_LLM
    CHAT --> INT_EMB
    CHAT --> OAI_EMB
    CHAT --> COH_EMB

    TRIGGER --> NATS & PG
    LISTENER --> NATS & PG
    AUDIT --> NATS & PG
    RECON --> NATS & PG & S3
```

---

## API Endpoints

<details>
<summary><strong>Authentication</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/auth/authorize` | Get OIDC authorization URL |
| `GET` | `/api/auth/token` | Get access token from session cookie |
| `POST` | `/api/token` | Exchange auth code for tokens |
| `GET` | `/auth_callback` | OIDC callback handler |
| `GET` | `/logout` | Logout and clear session |

</details>

<details>
<summary><strong>Health</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health/live` | Liveness probe |
| `GET` | `/health/ready` | Readiness probe (checks PostgreSQL, Qdrant, S3, NATS) |

</details>

<details>
<summary><strong>Collections</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/collections` | List collections |
| `GET` | `/api/collections/{id}` | Get collection |
| `POST` | `/api/collections` | Create collection |
| `PATCH` | `/api/collections/{id}` | Update collection |
| `DELETE` | `/api/collections/{id}` | Delete collection |
| `POST` | `/api/collections/{id}/files` | Upload files |
| `GET` | `/api/collections/{id}/files` | List files |
| `GET` | `/api/collections/{id}/files/{path}` | Download file |
| `DELETE` | `/api/collections/{id}/files/{path}` | Delete file |
| `GET` | `/api/collections/search` | Search collections |
| `GET` | `/api/collections-allowed-file-types` | List allowed file types |

</details>

<details>
<summary><strong>Datasets</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/datasets` | List datasets |
| `GET` | `/api/datasets/{id}` | Get dataset |
| `POST` | `/api/datasets` | Create dataset |
| `PATCH` | `/api/datasets/{id}` | Update dataset |
| `DELETE` | `/api/datasets/{id}` | Delete dataset |
| `GET` | `/api/datasets/{id}/items` | List dataset items |
| `GET` | `/api/datasets/{id}/items-summary` | Get items summary |
| `GET` | `/api/datasets/{id}/items/{item_id}/chunks` | Get item chunks |
| `DELETE` | `/api/datasets/{id}/items/{item_id}` | Delete item |
| `POST` | `/api/datasets/{id}/items` | Upload to dataset |

</details>

<details>
<summary><strong>Embedded Datasets</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/embedded-datasets` | List embedded datasets |
| `GET` | `/api/embedded-datasets/{id}` | Get embedded dataset |
| `PATCH` | `/api/embedded-datasets/{id}` | Update embedded dataset |
| `DELETE` | `/api/embedded-datasets/{id}` | Delete embedded dataset |
| `GET` | `/api/embedded-datasets/{id}/stats` | Get statistics |
| `GET` | `/api/embedded-datasets/{id}/points` | List vector points |
| `GET` | `/api/embedded-datasets/{id}/points/{point_id}/vector` | Get point vector |
| `GET` | `/api/embedded-datasets/{id}/processed-batches` | Get processed batches |
| `GET` | `/api/datasets/{dataset_id}/embedded-datasets` | Get by source dataset |
| `POST` | `/api/embedded-datasets/standalone` | Create standalone embedded dataset |
| `POST` | `/api/embedded-datasets/{id}/push-vectors` | Push vectors to embedded dataset |

</details>

<details>
<summary><strong>Embedders</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/embedders` | List embedders |
| `GET` | `/api/embedders/{id}` | Get embedder |
| `POST` | `/api/embedders` | Create embedder |
| `PATCH` | `/api/embedders/{id}` | Update embedder |
| `DELETE` | `/api/embedders/{id}` | Delete embedder |
| `POST` | `/api/embedders/{id}/test` | Test embedder connection |

</details>

<details>
<summary><strong>LLMs</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/llms` | List LLMs |
| `GET` | `/api/llms/{id}` | Get LLM |
| `POST` | `/api/llms` | Create LLM |
| `PATCH` | `/api/llms/{id}` | Update LLM |
| `DELETE` | `/api/llms/{id}` | Delete LLM |

</details>

<details>
<summary><strong>Inference APIs</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/embedding-inference/models` | List available embedding models |
| `GET` | `/api/llm-inference/models` | List available LLM models (supports quantized GGUF) |


</details>

<details>
<summary><strong>Collection Transforms</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/collection-transforms` | List transforms |
| `GET` | `/api/collection-transforms/{id}` | Get transform |
| `POST` | `/api/collection-transforms` | Create transform |
| `PATCH` | `/api/collection-transforms/{id}` | Update transform |
| `DELETE` | `/api/collection-transforms/{id}` | Delete transform |
| `POST` | `/api/collection-transforms/{id}/trigger` | Trigger execution |
| `GET` | `/api/collection-transforms/{id}/stats` | Get statistics |
| `GET` | `/api/collection-transforms/{id}/processed-files` | List processed files |
| `POST` | `/api/collection-transforms/{id}/retry-failed` | Retry failed files |
| `POST` | `/api/collection-transforms/batch-stats` | Batch stats |
| `GET` | `/api/collection-transforms/stream` | SSE status stream |
| `GET` | `/api/collections/{collection_id}/transforms` | Get by collection |
| `GET` | `/api/collections/{collection_id}/failed-files` | Get failed files |
| `GET` | `/api/datasets/{dataset_id}/collection-transforms` | Get by dataset |

</details>

<details>
<summary><strong>Dataset Transforms</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/dataset-transforms` | List transforms |
| `GET` | `/api/dataset-transforms/{id}` | Get transform |
| `POST` | `/api/dataset-transforms` | Create transform |
| `PATCH` | `/api/dataset-transforms/{id}` | Update transform |
| `DELETE` | `/api/dataset-transforms/{id}` | Delete transform |
| `POST` | `/api/dataset-transforms/{id}/trigger` | Trigger execution |
| `GET` | `/api/dataset-transforms/{id}/stats` | Get statistics |
| `GET` | `/api/dataset-transforms/{id}/detailed-stats` | Get detailed stats |
| `GET` | `/api/dataset-transforms/{id}/batches` | List batches |
| `GET` | `/api/dataset-transforms/{id}/batches/{batch_id}` | Get batch |
| `GET` | `/api/dataset-transforms/{id}/batches/stats` | Batch stats |
| `POST` | `/api/dataset-transforms/{id}/retry-failed` | Retry failed batches |
| `POST` | `/api/dataset-transforms/{id}/batches/{batch_id}/retry` | Retry single batch |
| `GET` | `/api/dataset-transforms/stream` | SSE status stream |
| `GET` | `/api/datasets/{dataset_id}/transforms` | Get by dataset |

</details>

<details>
<summary><strong>Visualization Transforms</strong></summary>

Visualization transforms generate interactive 2D scatter plots from high-dimensional vector embeddings using:
- **UMAP**: Dimensionality reduction (N-d → 2D)
- **HDBSCAN**: Automatic cluster detection
- **LLM Naming**: Optional AI-generated cluster labels
- **datamapplot**: Interactive HTML visualization output

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/visualization-transforms` | List transforms |
| `GET` | `/api/visualization-transforms/{id}` | Get transform |
| `POST` | `/api/visualization-transforms` | Create transform |
| `PATCH` | `/api/visualization-transforms/{id}` | Update transform |
| `DELETE` | `/api/visualization-transforms/{id}` | Delete transform |
| `POST` | `/api/visualization-transforms/{id}/trigger` | Trigger execution |
| `GET` | `/api/visualization-transforms/{id}/stats` | Get statistics |
| `GET` | `/api/visualization-transforms/{id}/visualizations` | List visualizations |
| `GET` | `/api/visualization-transforms/{id}/visualizations/{visualization_id}` | Get visualization |
| `GET` | `/api/visualization-transforms/{id}/visualizations/{visualization_id}/download` | Download HTML |
| `GET` | `/api/visualizations/recent` | Get recent |
| `GET` | `/api/embedded-datasets/{id}/visualizations` | Get by embedded dataset |
| `GET` | `/api/visualization-transforms/stream` | SSE status stream |

</details>

<details>
<summary><strong>Search</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/search` | Vector search across embedded datasets |

</details>

<details>
<summary><strong>Chat</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/chat/sessions` | Create chat session |
| `GET` | `/api/chat/sessions` | List sessions |
| `GET` | `/api/chat/sessions/{id}` | Get session |
| `DELETE` | `/api/chat/sessions/{id}` | Delete session |
| `GET` | `/api/chat/sessions/{id}/messages` | List messages |
| `POST` | `/api/chat/sessions/{id}/messages` | Send message |
| `POST` | `/api/chat/sessions/{id}/messages/stream` | Stream message (SSE) |
| `POST` | `/api/chat/messages/{message_id}/regenerate` | Regenerate message |

</details>

<details>
<summary><strong>Marketplace</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/marketplace/collections` | List public collections |
| `GET` | `/api/marketplace/collections/recent` | Recent public collections |
| `GET` | `/api/marketplace/datasets` | List public datasets |
| `GET` | `/api/marketplace/datasets/recent` | Recent public datasets |
| `GET` | `/api/marketplace/embedders` | List public embedders |
| `GET` | `/api/marketplace/embedders/recent` | Recent public embedders |
| `GET` | `/api/marketplace/llms` | List public LLMs |
| `GET` | `/api/marketplace/llms/recent` | Recent public LLMs |
| `POST` | `/api/marketplace/collections/{id}/grab` | Clone collection |
| `POST` | `/api/marketplace/datasets/{id}/grab` | Clone dataset |
| `POST` | `/api/marketplace/embedders/{id}/grab` | Clone embedder |
| `POST` | `/api/marketplace/llms/{id}/grab` | Clone LLM |

</details>

<details>
<summary><strong>Other</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/swagger-ui` | Interactive API documentation |
| `GET` | `/api/users/@me` | Get current user info |
| `GET` | `/api/status/nats` | NATS connection status |
| `GET` | `/metrics` | Prometheus metrics |

</details>

---

## Environment Variables

This service uses shared configuration from `semantic-explorer-core`. See the [root README](../../README.md) for the complete environment variable reference.

### Required Variables

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL connection string |
| `AWS_REGION` | S3 region |
| `AWS_ENDPOINT_URL` | S3 endpoint URL |
| `S3_BUCKET_NAME` | S3 bucket name |
| `ENCRYPTION_MASTER_KEY` | 32-byte hex key for AES-256-GCM encryption |
| `OIDC_CLIENT_ID` | OIDC client identifier |
| `OIDC_CLIENT_SECRET` | OIDC client secret |
| `OIDC_ISSUER_URL` | OIDC issuer URL |

### Optional Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HOSTNAME` | `localhost` | Server bind address |
| `PORT` | `8080` | Server port |
| `PUBLIC_URL` | - | External URL for OIDC callbacks |
| `NATS_URL` | `nats://localhost:4222` | NATS server URL |
| `QDRANT_URL` | `http://localhost:6334` | Qdrant gRPC endpoint |
| `EMBEDDING_INFERENCE_API_URL` | `http://localhost:8090` | Local embedding API |
| `LLM_INFERENCE_API_URL` | `http://localhost:8091` | Local LLM API |
| `CORS_ALLOWED_ORIGINS` | - | Comma-separated allowed origins |
| `LOG_FORMAT` | `json` | `json` or `pretty` |
| `RECONCILIATION_INTERVAL_SECS` | `300` | NATS-coordinated reconciliation interval (batch recovery + backfill scans) |

### Valkey Cache (Optional)

| Variable | Default | Description |
|----------|---------|-------------|
| `VALKEY_URL` | `redis://localhost:6379` | Valkey/Redis connection URL |
| `VALKEY_READ_URL` | Same as `VALKEY_URL` | Read replica URL |
| `VALKEY_PASSWORD` | - | Authentication password |
| `VALKEY_TLS_ENABLED` | `false` | Enable TLS for Valkey connections |
| `VALKEY_POOL_SIZE` | `10` | Connection pool size |
| `VALKEY_BEARER_CACHE_TTL_SECS` | `3600` | Bearer token cache TTL (1 hour) |
| `VALKEY_RESOURCE_CACHE_TTL_SECS` | `300` | Resource listing cache TTL (5 min) |
| `VALKEY_CONNECT_TIMEOUT_SECS` | `5` | Connection timeout |
| `VALKEY_RESPONSE_TIMEOUT_SECS` | `2` | Response timeout |

> Valkey is optional — the system degrades gracefully without it.

### Worker Configuration Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WORKER_SEARCH_BATCH_SIZE` | `200` | Batch size for search operations |
| `WORKER_CHAT_BATCH_SIZE` | `500` | Batch size for chat document inserts |
| `WORKER_DATASET_BATCH_SIZE` | `1000` | Batch size for dataset processing |
| `WORKER_S3_DELETE_BATCH_SIZE` | `1000` | Batch size for S3 delete operations |
| `WORKER_QDRANT_UPLOAD_CHUNK_SIZE` | `200` | Chunk size for Qdrant uploads |

> **Note:** NATS consumer tuning, circuit breaker, retry policy, and embedding retry parameters
> are hardcoded with production-tested defaults and no longer require environment variables.

---

## Building

```bash
# Debug build
cargo build -p semantic-explorer

# Release build
cargo build -p semantic-explorer --release
```

The binary will be at `target/release/semantic-explorer`.

### Docker

```bash
# From repository root
docker build -f crates/api/Dockerfile -t semantic-explorer:latest .
```

---

## Running

```bash
# Set required environment variables
export DATABASE_URL=postgresql://user:pass@localhost:5432/semantic_explorer
export ENCRYPTION_MASTER_KEY=$(openssl rand -hex 32)
# ... set other required variables

# Run
cargo run -p semantic-explorer
```

---

## Health Checks

```bash
# Liveness (process running)
curl http://localhost:8080/health/live

# Readiness (database connected)
curl http://localhost:8080/health/ready
```

---

## Metrics

Prometheus metrics available at `/metrics`.

### HTTP Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `http_requests_total` | Counter | Request count by method, path, status |
| `http_request_duration_seconds` | Histogram | Request duration |
| `http_requests_in_flight` | Gauge | Active requests |

### SSE Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `sse_connections_active` | Gauge | Active SSE connections |
| `sse_messages_sent` | Counter | SSE messages sent |

---

## Security

### Authentication

OIDC authentication required for all `/api/*` endpoints. Health endpoints are unauthenticated.

The API supports two authentication methods:

**1. Bearer Token (recommended for programmatic access)**

Pass the access token in the `Authorization` header:

```bash
curl 'https://your-instance.example.com/api/users/@me' \
  -H 'Authorization: Bearer <ACCESS_TOKEN>'
```

```python
import requests

headers = {'Authorization': 'Bearer <ACCESS_TOKEN>'}
response = requests.get('https://your-instance.example.com/api/users/@me', headers=headers)
print(response.json())
```

**2. Cookie-based (browser sessions)**

Handled automatically by the OIDC login flow. The browser stores an `HttpOnly` session cookie.

**Obtaining a Bearer Token**

*From an active browser session:*

```bash
# If you're already logged in via the browser, retrieve your token:
curl 'https://your-instance.example.com/api/auth/token' \
  -b 'access_token=<SESSION_COOKIE>'
```

*Programmatic OIDC flow (for API clients / CLI tools):*

```bash
# Step 1: Get the authorization URL
curl 'https://your-instance.example.com/api/auth/authorize'
# Returns: { "authorization_url": "...", "nonce": "...", "pkce_verifier": "..." }

# Step 2: Open the authorization_url in a browser, authenticate, and capture the ?code= parameter

# Step 3: Exchange the code for tokens
curl -X POST 'https://your-instance.example.com/api/token' \
  -H 'Content-Type: application/json' \
  -d '{"code": "<AUTH_CODE>", "nonce": "<NONCE>"}'
# Returns: { "access_token": "...", "token_type": "Bearer", ... }

# Step 4: Use the access_token as a Bearer token
curl 'https://your-instance.example.com/api/users/@me' \
  -H 'Authorization: Bearer <ACCESS_TOKEN>'
```

### Encryption

API keys for embedders and LLMs are encrypted with AES-256-GCM before storage.

Generate a master key:
```bash
openssl rand -hex 32
```

### Audit Logging

All API actions logged to PostgreSQL `audit_events` table with:
- User identity (OIDC subject)
- Action type
- Resource type and ID
- Timestamp and IP address

---

## License

Apache License 2.0
