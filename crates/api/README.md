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
- **Transform Orchestration**: Publish jobs to NATS for workers to process
- **Embedding Visualizations**: 2D visualizations of vector embeddings using UMAP dimensionality reduction and HDBSCAN clustering
- **Search**: Vector search across embedded datasets
- **Chat**: Context-aware conversations with LLM integration
- **Real-time Updates**: Server-Sent Events (SSE) for transform progress
- **Authentication**: OIDC integration
- **Observability**: Prometheus metrics, OpenTelemetry tracing, structured logging
- **Reliability**: Reconciliation job for recovering failed batch publishes

---

## Architecture

```mermaid
graph TD
    subgraph "HTTP Layer"
        API[API Endpoints]
        AUTH[OIDC Auth Middleware]
        CORS[CORS Middleware]
    end

    subgraph "Services"
        COLL[Collections]
        DS[Datasets]
        EMB[Embedders]
        TRANS[Transforms]
        SEARCH[Search]
        CHAT[Chat]
    end

    subgraph "Background Tasks"
        SCANNER[Transform Scanners]
        LISTENER[Result Listeners]
        AUDIT[Audit Consumer]
        RECON[Reconciliation Job]
    end

    subgraph "External"
        PG[(PostgreSQL)]
        NATS[NATS JetStream]
        QD[(Qdrant)]
        S3[(S3/MinIO)]
    end

    API --> AUTH --> CORS
    CORS --> COLL & DS & EMB & TRANS & SEARCH & CHAT

    COLL --> PG & S3
    DS --> PG
    EMB --> PG
    TRANS --> NATS & PG
    SEARCH --> QD & PG
    CHAT --> QD & EMB

    SCANNER --> NATS & PG
    LISTENER --> NATS & PG
    AUDIT --> NATS & PG
```

---

## API Endpoints

<details>
<summary><strong>Health</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health/live` | Liveness probe |
| `GET` | `/health/ready` | Readiness probe (checks DB) |

</details>

<details>
<summary><strong>Collections</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/collections` | List collections |
| `GET` | `/api/collections/{id}` | Get collection |
| `POST` | `/api/collections` | Create collection |
| `PUT` | `/api/collections/{id}` | Update collection |
| `DELETE` | `/api/collections/{id}` | Delete collection |
| `POST` | `/api/collections/{id}/files` | Upload files |
| `GET` | `/api/collections/{id}/files` | List files |
| `GET` | `/api/collections/{id}/files/{path}` | Download file |
| `DELETE` | `/api/collections/{id}/files/{path}` | Delete file |
| `GET` | `/api/collections/search` | Search collections |
| `GET` | `/api/collections/allowed-file-types` | List allowed file types |

</details>

<details>
<summary><strong>Datasets</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/datasets` | List datasets |
| `GET` | `/api/datasets/{id}` | Get dataset |
| `POST` | `/api/datasets` | Create dataset |
| `PUT` | `/api/datasets/{id}` | Update dataset |
| `DELETE` | `/api/datasets/{id}` | Delete dataset |
| `GET` | `/api/datasets/{id}/items` | List dataset items |
| `GET` | `/api/datasets/{id}/items/summary` | Get items summary |
| `GET` | `/api/datasets/{id}/items/{item_id}/chunks` | Get item chunks |
| `DELETE` | `/api/datasets/{id}/items/{item_id}` | Delete item |
| `POST` | `/api/datasets/{id}/upload` | Upload to dataset |

</details>

<details>
<summary><strong>Embedded Datasets</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/embedded_datasets` | List embedded datasets |
| `GET` | `/api/embedded_datasets/{id}` | Get embedded dataset |
| `PUT` | `/api/embedded_datasets/{id}` | Update embedded dataset |
| `DELETE` | `/api/embedded_datasets/{id}` | Delete embedded dataset |
| `GET` | `/api/embedded_datasets/{id}/stats` | Get statistics |
| `GET` | `/api/embedded_datasets/{id}/points` | List vector points |
| `GET` | `/api/embedded_datasets/{id}/points/{point_id}/vector` | Get point vector |
| `GET` | `/api/embedded_datasets/{id}/batches` | Get processed batches |
| `GET` | `/api/embedded_datasets/batch-stats` | Batch stats for multiple |
| `GET` | `/api/embedded_datasets/by-dataset/{dataset_id}` | Get by source dataset |

</details>

<details>
<summary><strong>Embedders</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/embedders` | List embedders |
| `GET` | `/api/embedders/{id}` | Get embedder |
| `POST` | `/api/embedders` | Create embedder |
| `PUT` | `/api/embedders/{id}` | Update embedder |
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
| `PUT` | `/api/llms/{id}` | Update LLM |
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
| `GET` | `/api/collection_transforms` | List transforms |
| `GET` | `/api/collection_transforms/{id}` | Get transform |
| `POST` | `/api/collection_transforms` | Create transform |
| `PUT` | `/api/collection_transforms/{id}` | Update transform |
| `DELETE` | `/api/collection_transforms/{id}` | Delete transform |
| `POST` | `/api/collection_transforms/{id}/trigger` | Trigger execution |
| `GET` | `/api/collection_transforms/{id}/stats` | Get statistics |
| `GET` | `/api/collection_transforms/{id}/files` | List processed files |
| `GET` | `/api/collection_transforms/stream` | SSE status stream |
| `GET` | `/api/collection_transforms/batch-stats` | Batch stats |
| `GET` | `/api/collection_transforms/by-collection/{id}` | Get by collection |
| `GET` | `/api/collection_transforms/by-dataset/{id}` | Get by dataset |

</details>

<details>
<summary><strong>Dataset Transforms</strong></summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/dataset_transforms` | List transforms |
| `GET` | `/api/dataset_transforms/{id}` | Get transform |
| `POST` | `/api/dataset_transforms` | Create transform |
| `PUT` | `/api/dataset_transforms/{id}` | Update transform |
| `DELETE` | `/api/dataset_transforms/{id}` | Delete transform |
| `POST` | `/api/dataset_transforms/{id}/trigger` | Trigger execution |
| `GET` | `/api/dataset_transforms/{id}/stats` | Get statistics |
| `GET` | `/api/dataset_transforms/{id}/detailed-stats` | Get detailed stats |
| `GET` | `/api/dataset_transforms/{id}/batches` | List batches |
| `GET` | `/api/dataset_transforms/{id}/batches/{batch_id}` | Get batch |
| `GET` | `/api/dataset_transforms/{id}/batches/{batch_id}/stats` | Batch stats |
| `GET` | `/api/dataset_transforms/stream` | SSE status stream |
| `GET` | `/api/dataset_transforms/batch-stats` | Batch stats |
| `GET` | `/api/dataset_transforms/by-dataset/{id}` | Get by dataset |

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
| `GET` | `/api/visualization_transforms` | List transforms |
| `GET` | `/api/visualization_transforms/{id}` | Get transform |
| `POST` | `/api/visualization_transforms` | Create transform |
| `PUT` | `/api/visualization_transforms/{id}` | Update transform |
| `DELETE` | `/api/visualization_transforms/{id}` | Delete transform |
| `POST` | `/api/visualization_transforms/{id}/trigger` | Trigger execution |
| `GET` | `/api/visualization_transforms/{id}/stats` | Get statistics |
| `GET` | `/api/visualization_transforms/stream` | SSE status stream |
| `GET` | `/api/visualizations` | List visualizations |
| `GET` | `/api/visualizations/{id}` | Get visualization |
| `GET` | `/api/visualizations/{id}/html` | Download HTML |
| `GET` | `/api/visualizations/by-dataset/{id}` | Get by dataset |
| `GET` | `/api/visualizations/recent` | Get recent |

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
| `GET` | `/api/chat/sessions/{id}/messages/stream` | Stream message (SSE) |
| `POST` | `/api/chat/sessions/{id}/messages/{msg_id}/regenerate` | Regenerate message |

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
| `RECONCILIATION_INTERVAL_SECS` | `300` | Reconciliation job interval (seconds) |

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
