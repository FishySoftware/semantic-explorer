# semantic-explorer-core

Core library providing shared utilities, configuration management, and infrastructure abstractions for the Semantic Explorer platform.

## Overview

This crate serves as the foundational library for all Semantic Explorer services, providing:

- Centralized configuration management
- S3 storage client initialization
- NATS JetStream setup and consumer patterns
- OpenTelemetry observability infrastructure
- HTTP client with TLS/mTLS support
- Shared domain models and types
- Input validation utilities

## Architecture

```mermaid
graph TB
    subgraph "semantic-explorer-core"
        CONFIG[Configuration<br/>Management]
        STORAGE[S3 Storage<br/>Client]
        NATS[NATS JetStream<br/>Infrastructure]
        OTEL[OpenTelemetry<br/>Metrics & Tracing]
        HTTP[HTTP Client<br/>TLS/mTLS]
        MODELS[Shared Models<br/>& Types]
        VALIDATION[Input<br/>Validation]
    end

    subgraph "Consumers"
        API[semantic-explorer<br/>API Server]
        WC[worker-collections]
        WD[worker-datasets]
    end

    API --> CONFIG
    API --> STORAGE
    API --> NATS
    API --> OTEL
    API --> HTTP
    API --> MODELS

    WC --> CONFIG
    WC --> STORAGE
    WC --> NATS
    WC --> OTEL
    WC --> MODELS

    WD --> CONFIG
    WD --> STORAGE
    WD --> NATS
    WD --> OTEL
    WD --> MODELS
```

## Module Structure

| Module | Description |
|--------|-------------|
| `config` | Environment-based configuration loading with fail-fast validation |
| `storage` | AWS S3 client initialization and file operations |
| `nats` | JetStream stream/consumer setup and configuration |
| `worker` | Generic worker framework for background job processing |
| `observability` | OpenTelemetry metrics definitions and recording functions |
| `http_client` | Shared HTTP client with TLS certificate support |
| `models` | Domain models for jobs, transforms, and embedder configurations |
| `validation` | Input validation utilities and error types |

## Technologies

| Technology | Version | Purpose |
|------------|---------|---------|
| Rust | 2024 Edition | Language |
| tokio | workspace | Async runtime |
| aws-sdk-s3 | workspace | S3-compatible storage |
| async-nats | workspace | NATS messaging |
| opentelemetry | workspace | Distributed tracing & metrics |
| reqwest | workspace | HTTP client |
| serde | workspace | Serialization |

## Data Flow

```mermaid
sequenceDiagram
    participant App as Application
    participant Config as config.rs
    participant Storage as storage.rs
    participant NATS as nats.rs
    participant Metrics as observability.rs

    App->>Config: AppConfig::from_env()
    Config-->>App: Validated configuration

    App->>Storage: initialize_client()
    Storage-->>App: S3 Client

    App->>NATS: initialize_jetstream()
    NATS-->>NATS: Create/update streams
    NATS-->>App: Ready

    App->>Metrics: init_metrics_otel()
    Metrics-->>App: Metrics initialized

    loop Processing
        App->>Metrics: record_*()
        Metrics-->>Metrics: Export to OTLP
    end
```

## Environment Variables

### Database Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `DATABASE_URL` | string | **required** | PostgreSQL connection string |
| `DB_MAX_CONNECTIONS` | integer | `50` | Maximum connection pool size |
| `DB_MIN_CONNECTIONS` | integer | `2` | Minimum connection pool size |
| `DB_ACQUIRE_TIMEOUT_SECS` | integer | `30` | Connection acquisition timeout |
| `DB_IDLE_TIMEOUT_SECS` | integer | `300` | Idle connection timeout |
| `DB_MAX_LIFETIME_SECS` | integer | `1800` | Maximum connection lifetime |

### NATS Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `NATS_URL` | string | `nats://localhost:4222` | NATS server URL |

### Qdrant Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `QDRANT_URL` | string | `http://localhost:6334` | Qdrant server URL |
| `QDRANT_API_KEY` | string | *optional* | Qdrant API key |
| `QDRANT_TIMEOUT_SECS` | integer | `30` | Request timeout |
| `QDRANT_CONNECT_TIMEOUT_SECS` | integer | `10` | Connection timeout |

### S3 Storage Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `AWS_REGION` | string | **required** | AWS region |
| `AWS_ACCESS_KEY_ID` | string | **required** | AWS access key |
| `AWS_SECRET_ACCESS_KEY` | string | **required** | AWS secret key |
| `AWS_ENDPOINT_URL` | string | **required** | S3-compatible endpoint URL |

### Server Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `HOSTNAME` | string | `localhost` | Server bind address |
| `PORT` | integer | `8080` | Server port |
| `STATIC_FILES_DIR` | string | `./semantic-explorer-ui/` | UI static files directory |
| `CORS_ALLOWED_ORIGINS` | string | *empty* | Comma-separated CORS origins |
| `SHUTDOWN_TIMEOUT_SECS` | integer | `30` | Graceful shutdown timeout |

### Observability Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SERVICE_NAME` | string | `semantic-explorer` | Service name for tracing |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | string | `http://localhost:4317` | OTLP exporter endpoint |
| `LOG_FORMAT` | string | `json` | Log format (`json` or `pretty`) |
| `RUST_LOG` | string | `info` | Tracing filter directive |

### TLS Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SERVER_SSL_ENABLED` | boolean | `false` | Enable server TLS |
| `TLS_SERVER_CERT_PATH` | string | *conditional* | Server certificate path (required if SSL enabled) |
| `TLS_SERVER_KEY_PATH` | string | *conditional* | Server private key path (required if SSL enabled) |
| `CLIENT_MTLS_ENABLED` | boolean | `false` | Enable client mTLS |
| `TLS_CLIENT_CERT_PATH` | string | *conditional* | Client certificate path (required if mTLS enabled) |
| `TLS_CLIENT_KEY_PATH` | string | *conditional* | Client private key path (required if mTLS enabled) |
| `TLS_CA_CERT_PATH` | string | `/app/certs/ca-bundle.crt` | CA certificate bundle path |

## Observability

### Metrics Exported

The crate exports the following OpenTelemetry metrics:

#### Database Metrics
- `database_query_total` - Counter of database queries by operation, table, status
- `database_query_duration_seconds` - Histogram of query durations
- `database_connection_pool_size` - Gauge of pool size
- `database_connection_pool_active` - Gauge of active connections
- `database_connection_pool_idle` - Gauge of idle connections

#### Storage Metrics
- `storage_operations_total` - Counter of S3 operations by type
- `storage_operation_duration_seconds` - Histogram of operation durations
- `storage_file_size_bytes` - Histogram of file sizes

#### Worker Metrics
- `worker_jobs_total` - Counter of jobs processed
- `worker_job_duration_seconds` - Histogram of job durations
- `worker_job_chunks` - Histogram of chunks per job
- `worker_job_file_size_bytes` - Histogram of file sizes
- `worker_job_failures_total` - Counter of job failures by error type
- `worker_job_retries_total` - Counter of job retries

#### Transform Metrics
- `collection_transform_jobs_total` - Collection transform jobs
- `collection_transform_files_processed` - Files processed
- `collection_transform_items_created` - Dataset items created
- `dataset_transform_jobs_total` - Dataset transform jobs
- `dataset_transform_batches_processed` - Batches processed
- `dataset_transform_chunks_embedded` - Chunks embedded
- `visualization_transform_jobs_total` - Visualization jobs
- `visualization_transform_points_created` - Points created
- `visualization_transform_clusters_created` - Clusters created

#### NATS Metrics
- `nats_stream_messages` - Messages in stream
- `nats_stream_bytes` - Stream size in bytes
- `nats_consumer_pending` - Pending consumer messages
- `nats_consumer_ack_pending` - Messages pending acknowledgement
- `nats_publish_duration_seconds` - Publish operation duration
- `nats_subscribe_latency_seconds` - Subscribe latency

#### Search Metrics
- `search_request_total` - Search requests
- `search_request_duration_seconds` - Total search duration
- `search_embedder_call_duration_seconds` - Embedder API call duration
- `search_qdrant_query_duration_seconds` - Qdrant query duration
- `search_results_returned` - Results per search

#### HTTP Metrics
- `http_requests_total` - HTTP requests by method, path, status
- `http_request_duration_seconds` - Request duration
- `http_requests_in_flight` - Current in-flight requests

#### SSE Metrics
- `sse_connections_active` - Active SSE connections
- `sse_messages_sent` - SSE messages sent
- `sse_connection_duration_seconds` - Connection duration

### Recording Metrics

```rust
use semantic_explorer_core::observability;

// Record a database query
observability::record_database_query("SELECT", "collections", duration, true);

// Record a storage operation
observability::record_storage_operation("upload", duration, Some(file_size), true);

// Record a worker job
observability::record_worker_job("transform-file", duration, "success");

// Update connection pool stats
observability::update_database_pool_stats(50, 10, 40);
```

## Usage Example

```rust
use semantic_explorer_core::config::AppConfig;
use semantic_explorer_core::storage::initialize_client;
use semantic_explorer_core::nats::initialize_jetstream;
use semantic_explorer_core::observability;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration (fails fast if required vars missing)
    let config = AppConfig::from_env()?;

    // Initialize S3 client
    let s3_client = initialize_client().await?;

    // Initialize NATS JetStream
    let nats_client = async_nats::connect(&config.nats.url).await?;
    initialize_jetstream(&nats_client).await?;

    // Initialize metrics
    observability::init_metrics_otel()?;

    // Your application logic...

    Ok(())
}
```

## NATS Streams

The crate configures the following JetStream streams:

| Stream | Subject | Retention | Max Age | Purpose |
|--------|---------|-----------|---------|---------|
| `COLLECTION_TRANSFORMS` | `workers.collection-transform` | WorkQueue | 7 days | File extraction jobs |
| `DATASET_TRANSFORMS` | `workers.dataset-transform` | WorkQueue | 7 days | Embedding generation jobs |
| `VISUALIZATION_TRANSFORMS` | `workers.visualization-transform` | WorkQueue | 7 days | Visualization jobs |
| `DLQ_TRANSFORMS` | `dlq.*-transforms` | Limits | 30 days | Failed job investigation |
| `TRANSFORM_STATUS` | `transforms.*.status.*.*.*` | Limits | 1 hour | Real-time status updates |

## License

See LICENSE file in repository root.
