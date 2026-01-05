# Semantic Explorer

A high-performance document processing and semantic search platform built with Rust.

## Overview

Semantic Explorer provides a complete system for:
- **Document Management**: Upload and organize documents in collections
- **Dataset Creation**: Build datasets from processed documents
- **Transform Pipelines**: Automatically extract, chunk, and index documents
- **Background Processing**: Scalable job processing with NATS and Apalis

## Architecture

```
┌─────────────────┐
│   REST API      │  Actix-web with OpenAPI docs
├─────────────────┤
│  Auth (OIDC)    │  OpenID Connect authentication
├─────────────────┤
│  Business Logic │  Collections, Datasets, Transforms
├─────────────────┤
│  Storage Layer  │
│  ├─ PostgreSQL  │  Metadata storage
│  ├─ S3/MinIO    │  Document storage
│  └─ Qdrant      │  Vector storage (future)
├─────────────────┤
│  Job Queue      │  NATS + Apalis workers
├─────────────────┤
│  Observability  │  OpenTelemetry + Prometheus
└─────────────────┘
```

## Features

### Document Processing
- **Multi-format Support**: PDF, Word, Excel, PowerPoint, OpenDocument, HTML, XML, Text
- **Intelligent Chunking**: Unicode-aware sentence boundary detection
- **Text Cleaning**: Normalization, whitespace handling, control character removal
- **Error Recovery**: Panic handling for problematic PDFs

### API Features
- **RESTful Design**: Standard HTTP methods and status codes
- **OpenAPI/Swagger**: Interactive API documentation at `/swagger-ui`
- **Authentication**: OIDC-based with JWT tokens
- **Authorization**: Row-level security (users can only access their own data)
- **Pagination**: Efficient cursor-based pagination
- **File Upload**: Multipart upload with 1GB limit

### Performance
- **Async Rust**: Non-blocking I/O with Tokio
- **Connection Pooling**: PostgreSQL pool (5-50 connections)
- **Batch Processing**: Parallel document processing
- **Efficient Allocator**: mimalloc on musl targets

### Observability
- **Distributed Tracing**: OpenTelemetry spans with W3C trace context
- **Metrics**: Prometheus metrics at `/metrics`
- **Structured Logging**: JSON logs with correlation IDs
- **Health Checks**: `/health` endpoint

## Quick Start

### Prerequisites
- Rust 1.75+ (edition 2024)
- PostgreSQL 14+
- MinIO or S3-compatible storage
- NATS server
- Qdrant (optional)
- OIDC provider (e.g., Dex, Keycloak)

### Environment Variables

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost/semantic_explorer

# S3/MinIO
AWS_ENDPOINT_URL=http://localhost:9000
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin
AWS_REGION=us-east-1

# NATS
NATS_URL=nats://localhost:4222

# Qdrant
QDRANT_URL=http://localhost:6334

# OIDC
OIDC_CLIENT_ID=semantic-explorer
OIDC_CLIENT_SECRET=your-secret
OIDC_ISSUER_URL=http://localhost:5556/dex

# Server
HOSTNAME=localhost
PORT=8080
STATIC_FILES_DIR=./semantic-explorer-ui/

# Observability
SERVICE_NAME=semantic-explorer
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
LOG_FORMAT=json  # or omit for compact format
RUST_LOG=info,semantic_explorer=debug
```

### Build and Run

```bash
# Development
cargo run

# Production build
cargo build --release

# Run with Docker Compose
docker compose -f deployment/compose/compose.yaml up
```

### API Documentation

Once running, visit:
- Swagger UI: `http://localhost:8080/swagger-ui/`
- OpenAPI spec: `http://localhost:8080/api/openapi.json`
- Metrics: `http://localhost:8080/metrics`
- Health: `http://localhost:8080/health`

## Development

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests (requires test database)
DATABASE_URL=postgresql://localhost/test cargo test -- --ignored

# With coverage
cargo tarpaulin --out Html
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Security audit
cargo audit

# Check for outdated dependencies
cargo outdated
```

### Database Migrations

Migrations are automatically run on startup. Located in:
```
src/storage/postgres/migrations/
```

## API Usage Examples

### Create a Collection

```bash
curl -X POST http://localhost:8080/api/collections \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Research Papers",
    "details": "Academic papers collection",
    "tags": ["research", "ai"]
  }'
```

### Upload Files

```bash
curl -X POST http://localhost:8080/api/collections/1/files \
  -H "Authorization: Bearer $TOKEN" \
  -F "files=@document1.pdf" \
  -F "files=@document2.pdf"
```

### Create a Transform

```bash
curl -X POST http://localhost:8080/api/transforms \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Research to Dataset",
    "collection_id": 1,
    "dataset_id": 1,
    "chunk_size": 200
  }'
```

## Project Structure

```
semantic-explorer/
├── src/
│   ├── main.rs              # Application entry point
│   ├── api/                 # REST API endpoints
│   ├── auth/                # Authentication & authorization
│   ├── collections/         # Collection domain models
│   ├── datasets/            # Dataset domain models
│   ├── transforms/          # Transform domain models
│   ├── storage/             # Storage layer
│   │   ├── postgres/        # PostgreSQL operations
│   │   ├── qdrant/          # Vector database
│   │   └── rustfs/          # S3 operations
│   ├── observability/       # Tracing, metrics, logging
│   └── transforms/          # Document processing
│       ├── apalis/          # Background job workers
│       ├── chunk/           # Text chunking
│       ├── extract/         # Text extraction
│       └── cleanup.rs       # Text cleaning
├── Cargo.toml
└── README.md
```

## Performance Considerations

### Database
- Connection pooling configured (5-50 connections)
- Prepared statements used throughout
- Indexes on foreign keys and frequently queried columns
- Consider adding read replicas for high traffic

### Storage
- S3 multipart upload for large files
- Lazy loading of file contents
- Consider CDN for frequently accessed files

### Job Processing
- NATS for durable message delivery
- Configurable worker concurrency
- Automatic retry with exponential backoff
- Dead letter queue for failed jobs

## Security

### Authentication
- OIDC/OAuth2 with JWT tokens
- Token validation on every request
- Automatic token refresh

### Authorization
- Row-level security (RLS) pattern
- Users can only access their own resources
- SQL queries include user ownership checks

### Input Validation
- Request payload validation with serde
- File type validation based on MIME type
- Size limits on uploads (1GB default)

### Best Practices
- No unsafe code (`#[forbid(unsafe_code)]`)
- SQL injection prevention via parameterized queries
- XSS prevention in API responses
- CORS configuration

## Troubleshooting

### Database Connection Issues
```bash
# Test connection
psql $DATABASE_URL -c "SELECT 1"

# Check migrations
sqlx migrate info
```

### OIDC Issues
```bash
# Verify issuer URL is accessible
curl $OIDC_ISSUER_URL/.well-known/openid-configuration
```

### Worker Not Processing Jobs
```bash
# Check NATS connection
nats-cli server ping

# Monitor job queue
nats-cli stream ls
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request

## License

See LICENSE file for details.

## Support

For issues and questions:
- GitHub Issues: [Report a bug](https://github.com/jpoisso/embedding-evaluation-system/issues)
- Documentation: See `/docs` directory
