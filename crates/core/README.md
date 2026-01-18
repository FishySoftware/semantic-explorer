# Semantic Explorer Core Library

Shared library providing common functionality for all Semantic Explorer services. Includes configuration management, observability setup, NATS JetStream initialization, encryption services, storage utilities, and worker base implementations.

## Overview

The `semantic-explorer-core` crate provides the foundational building blocks used across the API server and all worker services. It ensures consistent configuration, observability, security, and messaging patterns throughout the system.

## Architecture

```mermaid
graph TB
    subgraph "Core Library Modules"
        CONFIG[Configuration<br/>AppConfig]
        OBS[Observability<br/>OTEL + Metrics]
        NATS[NATS/JetStream<br/>Stream Setup]
        ENCRYPT[Encryption<br/>AES-256-GCM]
        HTTP[HTTP Client<br/>TLS Support]
        STORAGE[Storage<br/>S3/Qdrant/Postgres]
        WORKER[Worker Base<br/>Job Processing]
        EMBEDDER[Embedder<br/>Config & Client]
        TLS[TLS/SSL<br/>Certificates]
        MODELS[Shared Models<br/>Data Types]
    end

    subgraph "Consuming Services"
        API[API Server]
        WC[worker-collections]
        WD[worker-datasets]
        WV[worker-visualizations]
    end

    API --> CONFIG
    API --> OBS
    API --> NATS
    API --> ENCRYPT
    API --> HTTP
    API --> STORAGE

    WC --> CONFIG
    WC --> OBS
    WC --> WORKER
    WC --> EMBEDDER

    WD --> CONFIG
    WD --> OBS
    WD --> WORKER
    WD --> EMBEDDER

    WV --> CONFIG
    WV --> OBS
```

## Features

### Configuration Management

Centralized environment variable configuration with validation and defaults:

- **Server Configuration**: Host, port, public URL, CORS, TLS
- **Database Configuration**: Connection URL, pool settings, timeouts
- **NATS Configuration**: URL, stream settings, consumer parameters
- **Qdrant Configuration**: URL, API key, timeout
- **S3 Configuration**: Endpoint, credentials, bucket, region
- **Observability Configuration**: OTLP endpoint, service name/version
- **TLS Configuration**: Certificate paths for server and client
- **Inference Configuration**: Embedding and LLM API URLs

### Observability

Production-ready OpenTelemetry integration:

- **Tracing**: OTLP exporter for distributed tracing (Jaeger, Quickwit)
- **Metrics**: Prometheus exporter with custom metrics registry
- **Logging**: Structured JSON logging with trace/span correlation
- **Service Identification**: Automatic service name, version, environment tagging

### NATS JetStream

Automated stream and consumer setup:

- **Streams**:
  - `COLLECTION_TRANSFORMS` - File extraction jobs
  - `DATASET_TRANSFORMS` - Embedding generation jobs
  - `VISUALIZATION_TRANSFORMS` - UMAP/HDBSCAN jobs
  - `TRANSFORM_STATUS` - Real-time status updates
  - `DLQ_COLLECTION_TRANSFORMS` - Dead letter queue for failed collection jobs
  - `DLQ_DATASET_TRANSFORMS` - Dead letter queue for failed dataset jobs
  - `DLQ_VISUALIZATION_TRANSFORMS` - Dead letter queue for failed viz jobs

- **Consumer Configuration**:
  - Pull-based consumers with acknowledgment
  - Configurable max delivery attempts
  - Automatic DLQ routing on failure
  - Idempotent job processing

### Encryption Service

AES-256-GCM encryption for sensitive configuration:

- **Key Management**: Master key from environment variable
- **Encrypt/Decrypt**: API keys, tokens, credentials
- **Nonce Generation**: Cryptographically secure random nonces
- **Error Handling**: Clear error messages for troubleshooting

### Storage Utilities

Helper functions for external storage systems:

- **PostgreSQL**: Connection pool initialization with retry logic
- **S3/MinIO**: Client configuration with path-style and virtual-hosted URLs
- **Qdrant**: Client setup with optional API key authentication

### Worker Base

Common worker functionality for job processing:

- **Job Loop**: Pull from NATS, process, acknowledge/reject
- **Retry Logic**: Exponential backoff with max attempts
- **Error Handling**: Dead letter queue routing on failure
- **Graceful Shutdown**: Signal handling for clean termination
- **Telemetry**: Automatic metrics and tracing for worker operations

### Embedder Management

Embedder configuration and HTTP client:

- **Provider Support**: OpenAI, Cohere, Jina AI, Voyage AI, local inference API
- **API Client**: HTTP client with authentication headers
- **Batch Processing**: Helper for batching embedding requests
- **Error Handling**: Retry logic and rate limiting

### TLS/SSL

Certificate loading and configuration:

- **Server TLS**: Load cert/key pairs for HTTPS
- **Client TLS**: CA certificates for outbound connections
- **Rustls Integration**: Modern TLS library with strong defaults

## Modules

### `config`

Application configuration from environment variables:

```rust
use semantic_explorer_core::config::AppConfig;

let config = AppConfig::from_env()?;
println!("Database URL: {}", config.database.url);
println!("NATS URL: {}", config.nats.url);
```

**Key Structures**:
- `AppConfig` - Root configuration
- `ServerConfig` - HTTP server settings
- `DatabaseConfig` - PostgreSQL settings
- `NatsConfig` - NATS/JetStream settings
- `QdrantConfig` - Qdrant vector DB settings
- `S3Config` - S3/MinIO settings
- `TlsConfig` - TLS/SSL certificate paths
- `InferenceConfig` - Embedding API settings
- `LlmInferenceConfig` - LLM API settings

### `observability`

OpenTelemetry setup and initialization:

```rust
use semantic_explorer_core::observability::init_observability;

// Initialize tracing, metrics, and logging
let prometheus = init_observability()?;

// Prometheus exporter for Actix-web
app.wrap(prometheus.clone());
```

**Features**:
- OTLP trace exporter (gRPC)
- Prometheus metrics exporter
- Structured logging with JSON formatting
- Automatic trace/span propagation
- Service resource attributes

### `nats`

NATS JetStream stream and consumer initialization:

```rust
use semantic_explorer_core::nats::initialize_jetstream;

let nats_client = async_nats::connect(&config.nats.url).await?;
initialize_jetstream(&nats_client, &config.nats).await?;
```

**Streams Created**:
- `COLLECTION_TRANSFORMS` (work queue)
- `DATASET_TRANSFORMS` (work queue)
- `VISUALIZATION_TRANSFORMS` (work queue)
- `TRANSFORM_STATUS` (interest-based, 1hr retention)
- Dead letter queues for each transform type

**Consumer Patterns**:
- Pull-based with explicit ack
- Max delivery attempts (default 5)
- DLQ routing on max attempts

### `encryption`

AES-256-GCM encryption for secrets:

```rust
use semantic_explorer_core::encryption::EncryptionService;

let service = EncryptionService::from_env()?;

// Encrypt
let encrypted = service.encrypt("my-api-key")?;

// Decrypt
let decrypted = service.decrypt(&encrypted)?;
```

**Security**:
- AES-256-GCM authenticated encryption
- Random 96-bit nonces
- Base64 encoding for storage
- Master key from `ENCRYPTION_MASTER_KEY` env var

### `storage`

Storage system initialization:

```rust
use semantic_explorer_core::storage::{postgres, s3, qdrant};

// PostgreSQL connection pool
let pool = postgres::initialize_pool(&config.database).await?;

// S3 client
let s3_client = s3::initialize_client(&config.s3).await?;

// Qdrant client
let qdrant_client = qdrant::initialize_client(&config.qdrant).await?;
```

### `worker`

Base worker implementation:

```rust
use semantic_explorer_core::worker::{WorkerContext, BaseWorker};

struct MyWorker;

impl BaseWorker for MyWorker {
    async fn process_job(&self, ctx: &WorkerContext, job_data: Vec<u8>) -> Result<()> {
        // Process job...
        Ok(())
    }
}

let worker = MyWorker;
worker.run(ctx).await?;
```

**Features**:
- Automatic NATS pull consumer loop
- Ack/nack handling with DLQ routing
- Metrics for job count, duration, success/failure
- Graceful shutdown on SIGTERM/SIGINT

### `embedder`

Embedder configuration and client:

```rust
use semantic_explorer_core::embedder::{EmbedderConfig, EmbedderClient};

let embedder = EmbedderConfig {
    provider: "openai".to_string(),
    model: "text-embedding-3-small".to_string(),
    api_key: Some("sk-...".to_string()),
    ..Default::default()
};

let client = EmbedderClient::new(&embedder)?;
let embeddings = client.embed(&["hello world"]).await?;
```

**Supported Providers**:
- OpenAI (`text-embedding-3-small`, `text-embedding-3-large`)
- Cohere (`embed-english-v3.0`, `embed-multilingual-v3.0`)
- Jina AI (`jina-embeddings-v2-base-en`)
- Voyage AI (`voyage-2`, `voyage-code-2`)
- Local inference API (FastEmbed)

### `http_client`

HTTP client with TLS configuration:

```rust
use semantic_explorer_core::http_client::{initialize, get_client};

// Initialize once at startup
initialize(&config.tls)?;

// Use throughout application
let client = get_client();
let response = client.get("https://api.example.com").send().await?;
```

**Features**:
- Rustls-based TLS
- Custom CA certificates
- Client certificate authentication
- Connection pooling
- Timeout configuration

### `tls`

TLS certificate loading:

```rust
use semantic_explorer_core::tls::load_tls_config;

let tls_config = load_tls_config("/path/to/cert.pem", "/path/to/key.pem")?;
```

### `models`

Shared data models and types:

```rust
use semantic_explorer_core::models::{TransformStatus, OwnerInfo};

let status = TransformStatus::Running;
let owner = OwnerInfo {
    owner_id: "user-123".to_string(),
    owner_type: "user".to_string(),
};
```

### `subjects`

NATS subject/topic definitions:

```rust
use semantic_explorer_core::subjects::{jobs, status, dlq, consumers};

// Job subjects
let collection_job_subject = jobs::collection_transform("job-123");
let dataset_job_subject = jobs::dataset_transform("job-456");

// Status subjects
let status_subject = status::transform_status("transform-123");

// DLQ subjects
let dlq_subject = dlq::collection_transform("job-123");

// Consumer names
let consumer_name = consumers::COLLECTION_TRANSFORMS;
```

### `validation`

Input validation utilities:

```rust
use semantic_explorer_core::validation::{validate_name, validate_url};

validate_name("my-collection")?;  // OK
validate_name("")?;                // Error: name cannot be empty

validate_url("https://example.com")?;  // OK
validate_url("not-a-url")?;            // Error: invalid URL
```

## Configuration Reference

All configuration loaded from environment variables:

### Server

```bash
API_HOST=0.0.0.0
API_PORT=8080
PUBLIC_URL=https://example.com
CORS_ALLOWED_ORIGINS=https://app.example.com
STATIC_FILES_DIR=./dist
SHUTDOWN_TIMEOUT_SECS=30
```

### Database

```bash
DATABASE_URL=postgresql://user:pass@localhost:5432/db
DATABASE_MAX_CONNECTIONS=20
DATABASE_MIN_CONNECTIONS=2
DATABASE_CONNECT_TIMEOUT=10
DATABASE_IDLE_TIMEOUT=600
DATABASE_MAX_LIFETIME=1800
```

### NATS

```bash
NATS_URL=nats://localhost:4222
NATS_STREAM_REPLICAS=1
NATS_MAX_STREAM_SIZE_BYTES=10737418240
NATS_CONSUMER_MAX_DELIVER=5
```

### Qdrant

```bash
QDRANT_URL=http://localhost:6333
QDRANT_API_KEY=your-api-key
QDRANT_TIMEOUT=60
```

### S3/MinIO

```bash
S3_ENDPOINT=http://localhost:9000
S3_ACCESS_KEY=minioadmin
S3_SECRET_KEY=minioadmin
S3_BUCKET=semantic-explorer
S3_REGION=us-east-1
S3_PATH_STYLE=true
S3_MAX_UPLOAD_SIZE_BYTES=104857600
```

### Encryption

```bash
ENCRYPTION_MASTER_KEY=your-64-char-hex-key
```

### Observability

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=semantic-explorer
OTEL_SERVICE_VERSION=1.0.0
RUST_LOG=info,semantic_explorer=debug
LOG_FORMAT=json
```

### TLS

```bash
SERVER_SSL_ENABLED=true
SERVER_CERT_PATH=/path/to/cert.pem
SERVER_KEY_PATH=/path/to/key.pem
TLS_CA_CERT_PATH=/path/to/ca.pem
TLS_CLIENT_CERT_PATH=/path/to/client.pem
TLS_CLIENT_KEY_PATH=/path/to/client-key.pem
```

### Inference APIs

```bash
INFERENCE_API_URL=http://localhost:8090
LLM_INFERENCE_API_URL=http://localhost:8091
```

## Usage Examples

### Initialize Full Application Context

```rust
use semantic_explorer_core::{
    config::AppConfig,
    observability,
    nats,
    storage::{postgres, s3, qdrant},
    encryption::EncryptionService,
    http_client,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = AppConfig::from_env()?;

    // Initialize HTTP client
    http_client::initialize(&config.tls)?;

    // Initialize observability
    let prometheus = observability::init_observability()?;

    // Initialize encryption
    let encryption = EncryptionService::from_env()?;

    // Initialize storage
    let pool = postgres::initialize_pool(&config.database).await?;
    let s3_client = s3::initialize_client(&config.s3).await?;
    let qdrant_client = qdrant::initialize_client(&config.qdrant).await?;

    // Initialize NATS
    let nats_client = async_nats::connect(&config.nats.url).await?;
    nats::initialize_jetstream(&nats_client, &config.nats).await?;

    // Start your application...
    Ok(())
}
```

### Implement Custom Worker

```rust
use semantic_explorer_core::{
    worker::{WorkerContext, BaseWorker},
    config::AppConfig,
};
use anyhow::Result;

struct MyCustomWorker {
    // Worker-specific fields
}

impl BaseWorker for MyCustomWorker {
    async fn process_job(&self, ctx: &WorkerContext, job_data: Vec<u8>) -> Result<()> {
        // Deserialize job
        let job: MyJob = serde_json::from_slice(&job_data)?;

        // Process job...

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env()?;

    // Create worker context
    let ctx = WorkerContext::new(
        config,
        "my-worker",
        "COLLECTION_TRANSFORMS",
        semantic_explorer_core::consumers::COLLECTION_TRANSFORMS,
    ).await?;

    // Run worker
    let worker = MyCustomWorker {};
    worker.run(ctx).await?;

    Ok(())
}
```

## Building

```bash
# Build library
cargo build -p semantic-explorer-core

# Run tests
cargo test -p semantic-explorer-core

# Check documentation
cargo doc -p semantic-explorer-core --open
```

## Testing

```bash
# Unit tests
cargo test -p semantic-explorer-core

# With logging
RUST_LOG=debug cargo test -p semantic-explorer-core -- --nocapture
```

## Dependencies

Key dependencies:

- **actix-web-prom** - Prometheus metrics for Actix-web
- **aes-gcm** - AES-GCM encryption
- **async-nats** - NATS client
- **aws-sdk-s3** - S3 client
- **opentelemetry** - Observability framework
- **opentelemetry-otlp** - OTLP exporter
- **opentelemetry-prometheus** - Prometheus metrics
- **reqwest** - HTTP client
- **rustls** - TLS library
- **serde** - Serialization/deserialization
- **sqlx** - PostgreSQL driver (via workspace)
- **tokio** - Async runtime
- **tracing** - Structured logging
- **utoipa** - OpenAPI types

See [Cargo.toml](Cargo.toml) for complete list.

## Development

### Adding New Configuration

1. Add field to appropriate config struct in `config.rs`
2. Add environment variable with `#[serde(default)]` or `#[serde(default = "default_fn")]`
3. Document in module-level docs
4. Update README configuration section

### Adding New NATS Streams

1. Add stream definition to `nats.rs`
2. Add subject helpers to `subjects.rs`
3. Add consumer name constant
4. Update documentation

### Adding New Models

1. Add model to `models.rs`
2. Add serde derives
3. Add utoipa::ToSchema for OpenAPI
4. Add documentation comments

## Troubleshooting

### Configuration Errors

```bash
# Missing required environment variable
Error: environment variable not found: DATABASE_URL

# Solution: Set the variable
export DATABASE_URL=postgresql://localhost/db
```

### Encryption Errors

```bash
# Missing encryption key
Warning: Encryption service not initialized

# Solution: Generate and set key
export ENCRYPTION_MASTER_KEY=$(openssl rand -hex 32)
```

### NATS Connection Errors

```bash
# NATS connection failed
Error: connection refused

# Solution: Check NATS is running
nats server check
```

### TLS Errors

```bash
# Certificate not found
Error: No such file or directory: /path/to/cert.pem

# Solution: Verify certificate paths
ls -l /path/to/cert.pem /path/to/key.pem
```

## License

Apache License 2.0
