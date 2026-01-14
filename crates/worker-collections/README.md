# Worker Collections - Document Text Extraction

Background worker service for document text extraction and chunking in the Semantic Explorer platform. Handles multi-format document processing with configurable text chunking strategies.

## 📋 Overview

The `worker-collections` service processes uploaded documents via NATS JetStream, extracting text and splitting it into optimized chunks for embedding generation.

### Responsibilities
- 📥 Subscribe to collection transform jobs from NATS JetStream
- 📥 Download documents from S3 storage (PDF, DOCX, Excel, HTML, XML, etc.)
- 🔤 Extract text content using format-specific extractors
- 📏 Split text into chunks using configurable strategies
- 📤 Upload chunked results back to S3
- 📊 Publish extraction results to API and downstream workers
- 📝 Log transformation progress and metrics

### Supported Formats
- **PDF** - pdf-extract library with OCR support
- **Microsoft Office** - DOCX, XLSX, PPTX via docx-rs
- **OpenDocument** - ODT, ODS via zip extraction
- **HTML/XML** - Web and structured documents via scraper
- **Plain Text** - Direct processing
- **Archives** - ZIP files with recursive extraction

## 🏗️ Module Structure

### Extract Module (`extract/`)

Handles document format detection and text extraction:

```
extract/
├── config.rs          # Extraction configuration types
├── service.rs         # Extraction orchestration
├── mod.rs             # Module exports
├── strategies/        # Format-specific extractors
├── pdf/               # PDF text extraction
├── office/            # DOCX/XLSX/PPTX extraction
├── open_office/       # OpenDocument (ODT/ODS) extraction
├── html/              # HTML web document extraction
└── xml/               # XML structured data extraction
```

**PDF Extraction:**
```rust
// Handles text extraction from PDF files
// Supports both text-based and image-based PDFs
// OCR not included (planned for future)
let text = extract_pdf("document.pdf")?;
```

**Office Extraction:**
```rust
// DOCX: Extract text, preserve paragraphs and structure
// XLSX: Extract cell values with sheet organization
// PPTX: Extract slide text and speaker notes
let text = extract_office_doc("presentation.pptx")?;
```

**HTML Extraction:**
```rust
// Remove scripts and styles
// Extract main content (body text)
// Preserve semantic structure (headers, lists)
let text = extract_html("<html>...</html>")?;
```

### Chunk Module (`chunk/`)

Text chunking strategies for optimal embedding:

```
chunk/
├── config.rs          # Chunking configuration types
├── service.rs         # Chunking orchestration
├── metadata.rs        # Chunk metadata handling
├── strategies/        # Strategy implementations
└── mod.rs             # Module exports
```

**Fixed-Size Chunking** (Default)
```rust
// Configuration
let config = ChunkConfig {
    chunk_size: 512,        // Tokens per chunk
    overlap: 50,            // Overlap tokens
    preserve_paragraphs: true,
};

// Results in: [chunk1, chunk2, chunk3, ...]
// chunk2 starts within chunk1 (overlap)
```

**Semantic Chunking** (Intelligent)
```rust
// Detect natural paragraph/sentence boundaries
// Chunk at semantically meaningful points
// Variable chunk size (respects min/max)
// Better for embedding quality but slower
```

**Sentence Chunking** (Fast)
```rust
// Split on sentence boundaries
// Fast processing
// Smaller, more granular chunks
// Good for dense text extraction
```

## 🚀 Getting Started

### Prerequisites
- Rust 1.85+
- PostgreSQL 14+ (configuration storage)
- NATS 2.10+ (job queue)
- S3-compatible storage (MinIO, AWS S3, etc.)
- Redis 7+ (optional, for metrics/caching)

### Local Development

```bash
# Copy environment template
cp .env.example .env

# Edit .env with:
# NATS_SERVER_URL=nats://localhost:4222
# DATABASE_URL=postgresql://user:pass@localhost:5432/db
# S3_BUCKET=semantic-explorer-files
# S3_ENDPOINT=http://localhost:9000  # MinIO
# etc.

# Run migrations (from api crate)
cd ../api
sqlx migrate run --database-url "$DATABASE_URL"

# Start worker
cd ../worker-collections
cargo run
```

### Docker

```bash
docker build -t semantic-explorer-worker-collections .

docker run \
  -e NATS_SERVER_URL="nats://nats:4222" \
  -e DATABASE_URL="postgresql://user:pass@postgres:5432/db" \
  -e S3_BUCKET="semantic-explorer-files" \
  semantic-explorer-worker-collections
```

## 📊 Job Processing Flow

```
1. Listen on NATS subject: "collection.transform.pending"

2. Receive job:
   {
     "job_id": "uuid",
     "collection_id": "uuid", 
     "document_id": "uuid",
     "file_key": "s3://bucket/path/document.pdf",
     "chunk_config": {
       "chunk_size": 512,
       "overlap": 50,
       "strategy": "fixed_size"
     }
   }

3. Download document from S3
   GET s3://bucket/path/document.pdf

4. Detect format and extract text
   PDF → extract_pdf() → "Lorem ipsum..."

5. Apply chunking strategy
   "Lorem ipsum..." → [chunk1, chunk2, ...]

6. Store chunks in database
   INSERT INTO document_chunks(...)

7. Publish completion event
   NATS publish "collection.transform.complete"
   {
     "job_id": "uuid",
     "status": "completed",
     "chunks_count": 42,
     "total_tokens": 5680
   }
```

## 🔧 Configuration

### Environment Variables

```bash
# NATS JetStream Configuration
NATS_URL=nats://localhost:4222

# AWS S3 Configuration
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your-key
AWS_SECRET_ACCESS_KEY=your-secret
AWS_ENDPOINT_URL=http://localhost:9000
S3_BUCKET_NAME=semantic-explorer-files

# Worker Configuration
SERVICE_NAME=worker-collections     # Optional, defaults to worker-collections
MAX_CONCURRENT_JOBS=10              # Number of jobs to process concurrently

# File Processing
MAX_FILE_SIZE_MB=100                # Maximum file size to process

# Observability
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
LOG_FORMAT=json
RUST_LOG=worker_collections=debug
```

# Observability
LOG_LEVEL=info
PROMETHEUS_PORT=8001
OPENTELEMETRY_ENABLED=true
```

## 📊 Metrics

The worker exports Prometheus metrics on port 8001:

```
# Job processing
worker_jobs_processed_total{status}         # Total jobs processed
worker_job_duration_seconds{status}         # Job duration
worker_job_errors_total{error_type}         # Errors by type

# Document processing
worker_documents_extracted_total            # Documents processed
worker_text_chars_extracted_total           # Characters extracted
worker_chunks_created_total{strategy}       # Chunks created by strategy

# Performance
worker_extraction_duration_seconds{format}  # Extraction time by format
worker_chunking_duration_seconds{strategy}  # Chunking time by strategy
worker_file_size_bytes{format}              # File sizes by format
```

## 🧪 Testing

```bash
# Unit tests
cargo test --lib

# Integration tests (requires services)
cargo test --test '*'

# Test specific module
cargo test chunk::fixed_size::

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

## 📈 Performance Tuning

### Parallelization
```bash
# Increase concurrent job processing
NATS_BATCH_SIZE=20  # Default: 10

# More Tokio worker threads (0 = num_cpus)
TOKIO_WORKER_THREADS=0
```

### Chunking Strategy
```bash
# For dense documents (books, papers)
DEFAULT_CHUNK_STRATEGY=semantic
DEFAULT_CHUNK_SIZE=1024

# For web content
DEFAULT_CHUNK_STRATEGY=sentence
DEFAULT_CHUNK_SIZE=256
```

### Memory Optimization
```bash
# Reduce memory for large files
MAX_FILE_SIZE_MB=50
CHUNK_BATCH_WRITE=100  # Write chunks in batches
```

## 🐛 Debugging

### Enable detailed logging
```bash
RUST_LOG=semantic_explorer_worker_collections=debug cargo run
```

### Check job queue
```bash
# Monitor NATS stream
nats stream ls

# View pending jobs
nats stream info COLLECTION_TRANSFORMS
```

### Test extraction locally
```bash
# Run extractor on file
cargo run --example extract -- input.pdf
# Output: extracted text and statistics
```

## 🔐 Security Considerations

1. **File Size Limits** - Enforce MAX_FILE_SIZE_MB to prevent DoS
2. **Format Validation** - Validate file headers before processing
3. **Input Sanitization** - Clean extracted text of scripts/malicious content
4. **Storage Access** - Use S3 bucket policies to restrict access
5. **Timeout Enforcement** - Set EXTRACTION_TIMEOUT_SECS to prevent hangs

## 📚 Document Format Details

### PDF
- Uses pdf-extract crate for text extraction
- Handles both text-based and image-based PDFs
- Preserves text order and structure
- Max file size: 100MB (configurable)

### DOCX
- Uses docx-rs crate for extraction
- Preserves paragraph structure
- Extracts headers, footers, comments
- Handles tables with semantic preservation

### XLSX
- Extracts cell values by row
- Preserves sheet names and order
- Skips empty cells, maintains structure
- Good for structured data

### HTML
- Uses scraper crate with CSS selectors
- Removes scripts, styles, metadata
- Extracts from main content area
- Handles malformed HTML gracefully

## 🚀 Scaling

### Horizontal Scaling
```bash
# Deploy multiple worker instances
# Each subscribes to same NATS consumer
# NATS automatically load-balances jobs
```

### Vertical Scaling
```bash
# Increase concurrency per instance
NATS_BATCH_SIZE=50
TOKIO_WORKER_THREADS=16
```

### Resource Monitoring
```bash
# Monitor via Prometheus
# CPU: worker_extraction_duration_seconds
# Memory: Watch for leaked buffers
# I/O: Monitor S3 request latency
```

## 🤝 Integration with Other Services

**Upstream (API):**
- Receives jobs from API via NATS
- Reads upload files from S3
- Updates job status in database

**Downstream (Worker-Datasets):**
- Publishes completion events
- Worker-datasets picks up chunks
- Generates embeddings

**Storage:**
- Reads: S3 (document files)
- Writes: S3 (extracted chunks), PostgreSQL (metadata)

## 📖 API Contract

### Input Job Format
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "collection_id": "550e8400-e29b-41d4-a716-446655440001",
  "document_id": "550e8400-e29b-41d4-a716-446655440002",
  "file_key": "collections/123/documents/456/document.pdf",
  "chunk_config": {
    "chunk_size": 512,
    "overlap": 50,
    "strategy": "fixed_size"
  }
}
```

### Output Result Format
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "chunks": [
    {
      "id": "uuid",
      "text": "Extracted text chunk...",
      "char_count": 512,
      "token_estimate": 128
    }
  ],
  "statistics": {
    "total_chars": 50000,
    "total_chunks": 42,
    "processing_time_ms": 2345
  }
}
```

---

**Version**: 1.0.0  
**Status**: Production Ready ✅  
**Last Updated**: January 2026


```mermaid
graph TB
    subgraph "Message Queue"
        NATS[NATS JetStream<br/>COLLECTION_TRANSFORMS]
    end

    subgraph "worker-collections"
        CONSUMER[Job Consumer]

        subgraph "Extract Module"
            PDF[PDF Extractor<br/>pdf-extract]
            OFFICE[Office Extractor<br/>DOCX/XLSX]
            ODT[OpenDocument<br/>ODT/ODS]
            HTML[HTML Extractor<br/>scraper]
            XML[XML Extractor<br/>quick-xml]
            ZIP[ZIP Handler]
        end

        subgraph "Chunk Module"
            FIXED[Fixed Size<br/>Chunking]
            SEMANTIC[Semantic<br/>Chunking]
            SENTENCE[Sentence<br/>Chunking]
        end

        SERVICE[Extraction<br/>Service]
        CHUNKER[Chunking<br/>Service]
    end

    subgraph "Storage"
        S3[(S3 Storage)]
    end

    subgraph "Output"
        RESULT[NATS Result<br/>worker.result.file]
    end

    NATS --> CONSUMER
    CONSUMER --> SERVICE
    SERVICE --> PDF & OFFICE & ODT & HTML & XML & ZIP
    SERVICE --> CHUNKER
    CHUNKER --> FIXED & SEMANTIC & SENTENCE

    S3 <--> CONSUMER
    CONSUMER --> RESULT
```

## Processing Pipeline

```mermaid
sequenceDiagram
    participant NATS
    participant Worker
    participant S3
    participant Extract as Extraction Service
    participant Chunk as Chunking Service

    NATS->>Worker: CollectionTransformJob
    Worker->>S3: Download file
    S3-->>Worker: File content

    Worker->>Extract: Extract text(mime_type, content)
    Extract->>Extract: Detect format
    Extract->>Extract: Parse document

    alt PDF
        Extract->>Extract: pdf-extract
    else Office (DOCX/XLSX)
        Extract->>Extract: ZIP + XML parsing
    else HTML
        Extract->>Extract: scraper library
    else XML
        Extract->>Extract: quick-xml
    end

    Extract-->>Worker: ExtractionResult{text, metadata}

    Worker->>Chunk: Chunk text(config)

    alt Fixed Size
        Chunk->>Chunk: Split by character count
    else Semantic
        Chunk->>Chunk: Call embedder for boundaries
    else Sentence
        Chunk->>Chunk: Split by sentence boundaries
    end

    Chunk-->>Worker: Vec<ChunkWithMetadata>

    Worker->>S3: Upload chunks JSON
    Worker->>NATS: CollectionTransformResult
```

## Supported File Formats

| Format | Extension | Extractor | Notes |
|--------|-----------|-----------|-------|
| PDF | `.pdf` | pdf-extract | Text layer extraction |
| Word | `.docx` | Office (ZIP/XML) | Full text from document.xml |
| Excel | `.xlsx` | Office (ZIP/XML) | Concatenated cell values |
| OpenDocument Text | `.odt` | OpenDocument | Full text extraction |
| OpenDocument Spreadsheet | `.ods` | OpenDocument | Cell value extraction |
| HTML | `.html`, `.htm` | scraper | Text with structure preservation |
| XML | `.xml` | quick-xml | Configurable element extraction |
| ZIP Archive | `.zip` | Built-in | Recursive extraction of contents |
| Plain Text | `.txt` | Direct | No processing needed |

## Chunking Strategies

### Fixed Size Chunking
Splits text into chunks of a specified character count with optional overlap.

```json
{
  "strategy": "fixed",
  "chunk_size": 1000,
  "chunk_overlap": 100
}
```

### Semantic Chunking
Uses an embedder to identify semantic boundaries for more coherent chunks.

```json
{
  "strategy": "semantic",
  "chunk_size": 1000,
  "embedder_config": {
    "provider": "openai",
    "base_url": "https://api.openai.com/v1",
    "model": "text-embedding-3-small"
  }
}
```

### Sentence Chunking
Splits at sentence boundaries while respecting maximum chunk size.

```json
{
  "strategy": "sentence",
  "chunk_size": 1000,
  "chunk_overlap": 50
}
```

## Technologies

| Technology | Version | Purpose |
|------------|---------|---------|
| Rust | 2024 Edition | Language |
| tokio | 1.49 | Async runtime |
| async-nats | 0.45 | Message queue consumer |
| aws-sdk-s3 | 1.119 | S3 storage client |
| pdf-extract | 0.10 | PDF text extraction |
| quick-xml | 0.39 | XML parsing |
| scraper | 0.25 | HTML parsing |
| zip | 7.0 | ZIP archive handling |
| unicode-segmentation | 1.12 | Text segmentation |
| unicode-normalization | 0.1 | Text normalization |

## Module Structure

| Module | Description |
|--------|-------------|
| `main` | Worker initialization and NATS subscription |
| `job` | Job processing logic and result publishing |
| `extract/` | Text extraction from various formats |
| `extract/config` | Extraction configuration types |
| `extract/service` | Extraction orchestration |
| `extract/strategies/` | Format-specific extractors |
| `extract/pdf/` | PDF extraction |
| `extract/office/` | Microsoft Office extraction |
| `extract/open_office/` | OpenDocument extraction |
| `extract/html/` | HTML extraction |
| `extract/xml/` | XML extraction |
| `chunk/` | Text chunking logic |
| `chunk/config` | Chunking configuration types |
| `chunk/service` | Chunking orchestration |
| `chunk/strategies/` | Chunking strategy implementations |
| `chunk/metadata` | Chunk metadata handling |

## Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `NATS_URL` | string | `nats://localhost:4222` | NATS server URL |
| `AWS_REGION` | string | **required** | S3 region |
| `AWS_ACCESS_KEY_ID` | string | **required** | S3 access key |
| `AWS_SECRET_ACCESS_KEY` | string | **required** | S3 secret key |
| `AWS_ENDPOINT_URL` | string | **required** | S3 endpoint URL |
| `SERVICE_NAME` | string | `worker-collections` | Service name for tracing |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | string | `http://localhost:4317` | OTLP exporter endpoint |
| `LOG_FORMAT` | string | `json` | Log format (`json` or `pretty`) |
| `RUST_LOG` | string | `info` | Tracing filter directive |
| `MAX_CONCURRENT_JOBS` | integer | `10` | Maximum concurrent job processing |
| `MAX_FILE_SIZE_MB` | integer | `100` | Maximum file size in MB |

## Job Message Format

### Input: CollectionTransformJob

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "collection_transform_id": 123,
  "owner": "user@example.com",
  "bucket": "collection-456",
  "source_file_key": "documents/report.pdf",
  "extraction_config": {
    "strategy": "default"
  },
  "chunking_config": {
    "strategy": "fixed",
    "chunk_size": 1000,
    "chunk_overlap": 100
  },
  "embedder_config": null
}
```

### Output: CollectionTransformResult

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "collection_transform_id": 123,
  "owner": "user@example.com",
  "source_file_key": "documents/report.pdf",
  "bucket": "collection-456",
  "chunks_file_key": "chunks/550e8400-e29b-41d4-a716-446655440000.json",
  "chunk_count": 42,
  "status": "success",
  "error": null,
  "processing_duration_ms": 1523
}
```

### Chunks File Format

```json
[
  {
    "id": "chunk_0",
    "text": "This is the first chunk of text...",
    "metadata": {
      "source_file": "report.pdf",
      "chunk_index": 0,
      "page": 1
    }
  },
  {
    "id": "chunk_1",
    "text": "This is the second chunk...",
    "metadata": {
      "source_file": "report.pdf",
      "chunk_index": 1,
      "page": 1
    }
  }
]
```

## Observability

### Metrics

The worker exports the following metrics via the core observability module:

- `worker_jobs_total{worker="transform-file", status="success|failed_*"}` - Job completion counter
- `worker_job_duration_seconds{worker="transform-file"}` - Job duration histogram
- `worker_job_chunks{worker="transform-file"}` - Chunks per job histogram
- `worker_job_file_size_bytes{worker="transform-file"}` - File size histogram
- `collection_transform_jobs_total{transform_id, status}` - Transform-specific counter
- `collection_transform_files_processed{transform_id}` - Files processed counter
- `collection_transform_items_created{transform_id}` - Items created counter

### Tracing

Jobs are traced with the following span attributes:
- `job_id` - Unique job identifier
- `collection_transform_id` - Transform pipeline ID
- `file` - Source file key

### Error Categories

| Status | Description |
|--------|-------------|
| `success` | Job completed successfully |
| `failed_download` | Could not download file from S3 |
| `failed_file_too_large` | File exceeds size limit |
| `failed_config_parse` | Invalid extraction/chunking config |
| `failed_extraction` | Text extraction failed |
| `failed_chunking` | Chunking operation failed |
| `failed_empty_chunks` | No chunks produced (text too short) |
| `failed_upload` | Could not upload results to S3 |

## Running

### Development

```bash
# Set environment variables
export NATS_URL="nats://localhost:4222"
export AWS_REGION="us-east-1"
export AWS_ACCESS_KEY_ID="minioadmin"
export AWS_SECRET_ACCESS_KEY="minioadmin"
export AWS_ENDPOINT_URL="http://localhost:9000"

# Run the worker
cargo run --bin worker-collections
```

### Docker

```bash
docker run \
  -e NATS_URL="nats://nats:4222" \
  -e AWS_REGION="us-east-1" \
  -e AWS_ACCESS_KEY_ID="..." \
  -e AWS_SECRET_ACCESS_KEY="..." \
  -e AWS_ENDPOINT_URL="http://minio:9000" \
  ghcr.io/your-org/worker-collections:latest
```

### Kubernetes

Deploy as part of the Helm chart with horizontal pod autoscaling based on NATS queue depth.

## Concurrency Configuration

### MAX_CONCURRENT_JOBS

Controls the maximum number of jobs processed simultaneously by a single worker instance:

```bash
# Default: 10 jobs per worker
export MAX_CONCURRENT_JOBS=10

# For resource-limited environments (e.g., development)
export MAX_CONCURRENT_JOBS=3

# For powerful servers (but consider backpressure)
export MAX_CONCURRENT_JOBS=20
```

**Recommendation**: Start with the default (10) and adjust based on:
- Available CPU cores (PDF extraction is CPU-intensive)
- Available memory (files can be large)
- NATS backpressure tuning
- Monitor memory usage and adjust if needed

## Scaling Considerations

- **Horizontal Scaling**: Multiple worker replicas can process jobs in parallel
- **Backpressure**: Controlled via `max_ack_pending` in NATS consumer config
- **Memory**: Large files are streamed; peak memory depends on document complexity
- **CPU**: PDF extraction is CPU-intensive; size pods accordingly
- **Retries**: Failed jobs are retried up to 5 times with exponential backoff
- **Concurrency**: Use `MAX_CONCURRENT_JOBS` to prevent resource exhaustion per replica

## License

See LICENSE file in repository root.
