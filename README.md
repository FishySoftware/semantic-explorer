# Semantic Explorer

Semantic Explorer enables you to upload documents, extract and process their content, generate vector embeddings, perform semantic search and topic modeling across your document collections.

## Features

- 📄 **Multi-format Document Processing** - Extract text from PDF, Microsoft Office, OpenDocument, HTML, XML, and plain text files
- 🔍 **Semantic Search** - Vector similarity search powered by Qdrant with metadata filtering
- 📊 **Dataset Management** - Build structured datasets from processed documents with automatic chunking
- 🚀 **Async Job Processing** - Background workers handle document extraction and embedding generation via NATS
- 🔐 **OpenID Connect Authentication** - Secure user authentication with OIDC integration
- 🎯 **Transform Pipelines** - Automated workflows to process collections into searchable datasets
- 📈 **Full Observability** - OpenTelemetry tracing, Prometheus metrics, and Grafana dashboards
- 🗄️ **S3-compatible Storage** - Store documents in any S3-compatible object storage
- 🎨 **Modern UI** - Clean Svelte-based interface for managing collections and datasets

## Architecture

Semantic Explorer follows a 3-tier architecture:

1. **API Layer** - REST API built with Actix-web, handles HTTP requests and authentication
2. **Storage Layer** - PostgreSQL for metadata, S3 for files, Qdrant for vector embeddings
3. **Worker Layer** - Async job processors for document extraction and embedding generation

### Usage Flow

#### Collections
Start by creating a collection and uploading your documents. Organize your content for processing.

#### Embedders
Configure your embedding providers (OpenAI, Cohere, etc.) that will be used to generate vector embeddings.

#### Transform (Collection)
Create a collection transform to extract text and generate chunks, populating a dataset for embedding.

#### Datasets
Review the generated dataset containing your processed text chunks ready for embedding.

#### Transform (Dataset)
Create dataset transforms to generate embeddings using your configured embedders.

#### Search
Execute searches across multiple embedded datasets to compare embedding model performance.

#### Visualize
*(Coming Soon)*

## Quick Start

### Prerequisites

- Rust 1.83+ (2024 edition)
- Docker and Docker Compose
- Node.js 18+ (for UI development)

### Run with Docker Compose

The fastest way to get started is using the included Docker Compose stack:

```bash
cd deployment/compose
docker compose up -d
```

This starts:
- API server (port 8080)
- PostgreSQL database
- Qdrant vector database
- NATS message queue
- Rustfs (S3-compatible storage)
- Dex OIDC provider
- OpenTelemetry Collector
- Prometheus & Grafana
- Quickwit (log and trace aggregation)

Access the application at [http://localhost:8080](http://localhost:8080)

### Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/FishySoftware/semantic-explorer.git
   cd semantic-explorer
   ```

2. **Start infrastructure services**
   ```bash
   docker compose -f deployment/compose/compose.yaml up -d postgres nats qdrant rustfs dex
   ```

3. **Set environment variables**
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

4. **Start the API server**
   ```bash
   cargo run --bin api
   ```

5. **Start background workers**
   ```bash
   # In separate terminals
   cargo run --bin worker-collections
   cargo run --bin worker-datasets
   ```

6. **Start the UI (optional)**
   ```bash
   cd semantic-explorer-ui
   npm install

   npm run dev
   or
   npm run build-watch
   ```

Alternatively, you can launch [Tasks](.vscode/tasks.json) in VSCode:
- Run API
- Run worker-collections
- Run worker-datasets
- Run UI 

## Configuration

Semantic Explorer is configured via environment variables. Key settings:

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | Required |
| `NATS_URL` | NATS server URL | `nats://localhost:4222` |
| `QDRANT_URL` | Qdrant server URL | `http://localhost:6334` |
| `AWS_ENDPOINT_URL` | S3 endpoint (MinIO/Rustfs/AWS) | Required |
| `AWS_ACCESS_KEY_ID` | S3 access key | Required |
| `AWS_SECRET_ACCESS_KEY` | S3 secret key | Required |
| `OIDC_ISSUER_URL` | OpenID Connect issuer | Required |
| `OIDC_CLIENT_ID` | OIDC client ID | Required |
| `OIDC_CLIENT_SECRET` | OIDC client secret | Required |
| `PORT` | API server port | `8080` |
| `RUST_LOG` | Log level | `info` |




## API Documentation

Interactive API documentation is available via Swagger UI at:

```
http://localhost:8080/swagger-ui/
```

## Development

### Project Structure

```
semantic-explorer/
├── crates/
│   ├── api/                    # Main REST API server
│   ├── core/                   # Shared libraries
│   ├── worker-collections/     # Document extraction worker
│   └── worker-datasets/        # Embedding generation worker
├── semantic-explorer-ui/       # Svelte frontend
├── deployment/
│   ├── compose/               # Docker Compose deployment
│   └── helm/                  # Kubernetes Helm charts
└── .github/workflows/         # CI/CD pipelines
```

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
# Linting
cargo clippy -- -D warnings

# Formatting
cargo fmt --check

# Type checking
cargo check

# Detected unused dependencies
cargo machete

# Security auditing (RSA is problematic right now, pending fix.)
cargo audit
```

### Database Migrations

Migrations are located in `crates/api/migrations/` and are automatically applied on startup.

## Supported Document Formats

- **PDF** - `.pdf`
- **Microsoft Office** - `.doc`, `.docx`, `.xls`, `.xlsx`, `.ppt`, `.pptx`
- **OpenDocument** - `.odt`, `.ods`, `.odp`
- **Web** - `.html`, `.xml`
- **Text** - `.txt`, `.csv`

## Observability

### Metrics

Prometheus metrics are exposed at:
```
http://localhost:8080/metrics
```

### Tracing

OpenTelemetry traces are exported to the configured OTLP endpoint. View traces in Grafana or Jaeger.

### Logging

Logs are structured and exported via OpenTelemetry.

## Deployment

### Docker

Build and run with Docker:

```bash
cd deployment/compose
docker compose up -d
```

### Kubernetes

Deploy using Helm:

```bash
helm install semantic-explorer deployment/helm/semantic-explorer/
```

## Contributing
Open issues or pull requests on GitHub.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Author

Jonathan Poisson

---

For questions or issues, please open a GitHub issue.
