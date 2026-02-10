# Semantic Explorer Core Library

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Shared library providing common functionality for all Semantic Explorer services**

</div>

Configuration, observability, NATS JetStream, encryption, storage utilities, and worker implementations.

---

## Modules

| Module | Description |
|--------|-------------|
| `config` | Centralized environment variable configuration |
| `observability` | OpenTelemetry tracing, Prometheus metrics, structured logging, GPU monitoring |
| `nats` | JetStream stream and consumer initialization |
| `adaptive_concurrency` | Dynamic concurrency controller with 503 backpressure |
| `encryption` | AES-256-GCM encryption for API keys |
| `storage` | S3 document upload/download utilities |
| `embedder` | Embedding API client (OpenAI, Cohere, internal) |
| `http_client` | Shared HTTP client with TLS support |
| `tls` | Certificate loading for server and client TLS |
| `models` | Shared data types for job messages |
| `subjects` | NATS subject/topic constants |
| `worker` | Base worker loop and job processing |
| `validation` | Input validation utilities |
| `owner_info` | Owner identification helpers |
| `retry` | Configurable retry policies with exponential backoff |
| `circuit_breaker` | Circuit breaker pattern for external service resilience |

---

## NATS JetStream Streams

The library initializes these streams on startup:

| Stream | Subject | Retention | Purpose |
|--------|---------|-----------|---------|
| `COLLECTION_TRANSFORMS` | `workers.collection-transform` | WorkQueue | File extraction jobs |
| `DATASET_TRANSFORMS` | `workers.dataset-transform` | WorkQueue | Embedding generation jobs |
| `VISUALIZATION_TRANSFORMS` | `workers.visualization-transform` | WorkQueue | UMAP/HDBSCAN jobs |
| `DLQ_TRANSFORMS` | `dlq.*-transforms` | Limits (30 days) | Dead letter queue |
| `TRANSFORM_STATUS` | `transforms.*.status.>` | Limits (1 hour) | SSE real-time updates |

### Subject Format

Status subjects follow this pattern:
```
transforms.{type}.status.{owner_hash}.{resource_id}.{transform_id}
```

Examples:
- `transforms.collection.status.abc123.42.101`
- `transforms.dataset.status.abc123.42.102`
- `transforms.visualization.status.abc123.42.103`

---

## Environment Variables

All configuration is loaded from environment variables at startup. See the [root README](../../README.md) for the complete reference.

### Configuration Structures

<details>
<summary><strong>DatabaseConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | - | PostgreSQL connection string (**required**) |
| `DB_MAX_CONNECTIONS` | `15` | Maximum pool connections |
| `DB_MIN_CONNECTIONS` | `2` | Minimum pool connections |
| `DB_ACQUIRE_TIMEOUT_SECS` | `30` | Connection acquire timeout |
| `DB_IDLE_TIMEOUT_SECS` | `300` | Idle connection timeout |
| `DB_MAX_LIFETIME_SECS` | `1800` | Maximum connection lifetime |

</details>

<details>
<summary><strong>NatsConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `NATS_URL` | `nats://localhost:4222` | NATS server URL |
| `NATS_REPLICAS` | `3` | Stream replica count |

> NATS consumer tuning (ack pending, ack wait) is hardcoded with production-tested defaults.

</details>

<details>
<summary><strong>QdrantConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `QDRANT_URL` | `http://localhost:6334` | Qdrant gRPC endpoint |
| `QDRANT_API_KEY` | - | API key for authentication |
| `QDRANT_TIMEOUT_SECS` | `30` | Request timeout |
| `QDRANT_CONNECT_TIMEOUT_SECS` | `10` | Connection timeout |
| `QDRANT_QUANTIZATION_TYPE` | `none` | `none`, `scalar`, or `product` |

</details>

<details>
<summary><strong>S3Config</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `AWS_REGION` | - | AWS region (**required**) |
| `AWS_ACCESS_KEY_ID` | - | Access key (optional if using IAM) |
| `AWS_SECRET_ACCESS_KEY` | - | Secret key (optional if using IAM) |
| `AWS_ENDPOINT_URL` | - | S3 endpoint (**required**) |
| `S3_BUCKET_NAME` | - | Bucket name (**required**) |
| `S3_MAX_DOWNLOAD_SIZE_BYTES` | `104857600` | Max download (100MB) |
| `S3_MAX_UPLOAD_SIZE_BYTES` | `1073741824` | Max upload (1GB) |

</details>

<details>
<summary><strong>ServerConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `HOSTNAME` | `localhost` | Server bind address |
| `PORT` | `8080` | Server port |
| `PUBLIC_URL` | - | External URL for OIDC callbacks |
| `CORS_ALLOWED_ORIGINS` | - | Comma-separated allowed origins |
| `STATIC_FILES_DIR` | `./semantic-explorer-ui/` | Path to static UI files |
| `SHUTDOWN_TIMEOUT_SECS` | - | Graceful shutdown timeout |

</details>

<details>
<summary><strong>ObservabilityConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVICE_NAME` | `semantic-explorer` | Service name for telemetry |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP collector endpoint |
| `LOG_FORMAT` | `json` | `json` or `pretty` |

</details>

<details>
<summary><strong>TlsConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_SSL_ENABLED` | `false` | Enable HTTPS |
| `TLS_SERVER_CERT_PATH` | - | Server certificate path |
| `TLS_SERVER_KEY_PATH` | - | Server private key path |
| `CLIENT_MTLS_ENABLED` | `false` | Enable mutual TLS |
| `TLS_CLIENT_CERT_PATH` | - | Client certificate path |
| `TLS_CLIENT_KEY_PATH` | - | Client private key path |
| `TLS_CA_CERT_PATH` | - | CA bundle path |

</details>

<details>
<summary><strong>OidcSessionConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `OIDC_SESSION_MANAGEMENT_ENABLED` | `true` | Enable session management |
| `OIDC_SESSION_TIMEOUT_SECS` | `3600` | Session timeout (1 hour) |
| `OIDC_INACTIVITY_TIMEOUT_SECS` | `1800` | Inactivity timeout (30 min) |
| `OIDC_MAX_CONCURRENT_SESSIONS` | `5` | Max sessions per user |
| `OIDC_REFRESH_TOKEN_ROTATION_ENABLED` | `true` | Enable token rotation |

</details>

<details>
<summary><strong>EmbeddingInferenceConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `EMBEDDING_INFERENCE_API_URL` | `http://localhost:8090` | Local embedding API URL |
| `EMBEDDING_INFERENCE_API_TIMEOUT_SECS` | `120` | Request timeout |

</details>

<details>
<summary><strong>LlmInferenceConfig</strong></summary>

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_INFERENCE_API_URL` | `http://localhost:8091` | Local LLM API URL |
| `LLM_INFERENCE_API_TIMEOUT_SECS` | `120` | Request timeout |

</details>

---

## Embedder Providers

The `embedder` module supports these providers:

| Provider | Value | Default Batch Size | Notes |
|----------|-------|-------------------|-------|
| OpenAI | `openai` | 2048 | Requires `api_key` |
| Cohere | `cohere` | 96 | Requires `api_key`, supports `input_type` |
| Internal | `internal` | 128 | Uses `EMBEDDING_INFERENCE_API_URL` |

---

## Retry Policies

The `retry` module provides configurable retry with exponential backoff:

```rust
use semantic_explorer_core::retry::{RetryPolicy, retry_with_policy, qdrant_retry_policy};

let policy = qdrant_retry_policy();
let result = retry_with_policy(&policy, "qdrant_upsert", || async {
    qdrant_client.upsert_points(...).await
}).await?;
```

### Configuration

Retry policy uses sensible defaults (3 attempts, 100ms initial delay, 10s max delay, 2.0x backoff,
0.1 jitter). These are hardcoded and no longer require environment variables.

Service-specific policies (`qdrant_retry_policy()`, `s3_retry_policy()`, `inference_retry_policy()`)
all use `RetryPolicy::default()`.

---

## Circuit Breakers

The `circuit_breaker` module implements the circuit breaker pattern:

```rust
use semantic_explorer_core::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};

let config = CircuitBreakerConfig::new("qdrant");
let breaker = CircuitBreaker::new(config);

let result = breaker.call("operation", || async {
    external_service.call().await
}).await?;
```

### States

- **Closed**: Normal operation, requests pass through
- **Open**: Service failing, requests fail immediately (fast-fail)
- **Half-Open**: Testing recovery, limited requests allowed

### Configuration

Circuit breaker uses sensible defaults (5 failure threshold, 3 success threshold to close,
30s open timeout, 60s failure window). These are hardcoded via `CircuitBreakerConfig::new(name)`
and no longer require environment variables.

Prefixes used: `qdrant`, `s3`, `inference`, `dataset_scanner`

---

## Encryption

AES-256-GCM encryption for storing sensitive data (API keys).

```rust
use semantic_explorer_core::encryption::EncryptionService;

let service = EncryptionService::from_env()?;
let encrypted = service.encrypt("my-api-key")?;
let decrypted = service.decrypt(&encrypted)?;
```

Requires `ENCRYPTION_MASTER_KEY` environment variable (32-byte hex string).

Generate a key:
```bash
openssl rand -hex 32
```

---

## Usage

Add to `Cargo.toml`:
```toml
[dependencies]
semantic-explorer-core = { path = "../core" }
```

### Configuration

```rust
use semantic_explorer_core::config::AppConfig;

let config = AppConfig::from_env()?;
```

### Observability

```rust
use semantic_explorer_core::observability::init_observability;

let prometheus = init_observability()?;
// Use prometheus middleware with actix-web
```

### NATS JetStream

```rust
use semantic_explorer_core::nats::initialize_jetstream;

let client = async_nats::connect(&config.nats.url).await?;
initialize_jetstream(&client, &config.nats).await?;
```

---

## License

Apache License 2.0
