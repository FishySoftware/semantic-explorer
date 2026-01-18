# Collections Worker

Document extraction and intelligent chunking worker for Semantic Explorer. Processes files from collections, extracts text from 10+ file formats, and chunks content using configurable strategies including code-aware chunking with tree-sitter.

## Overview

The collections worker consumes jobs from the `COLLECTION_TRANSFORMS` NATS stream, downloads files from S3, extracts text using format-specific parsers, chunks the content into semantically meaningful segments, and stores the results in PostgreSQL datasets.

## Architecture

```mermaid
graph TB
    subgraph "Job Processing"
        NATS[NATS JetStream<br/>COLLECTION_TRANSFORMS]
        WORKER[Collections Worker<br/>Pull Consumer]
    end

    subgraph "Extraction Pipeline"
        DOWNLOAD[Download from S3]
        DETECT[MIME Type Detection]
        EXTRACT[Format-Specific<br/>Extraction]
    end

    subgraph "Extractors"
        PDF[PDF<br/>pdf-extract]
        OFFICE[Office<br/>DOCX/XLSX/PPTX]
        LEGACY[Legacy Office<br/>DOC/XLS/PPT]
        MD[Markdown]
        HTML[HTML<br/>scraper]
        CODE[Code Files<br/>Tree-sitter]
        ARCHIVE[Archives<br/>ZIP/TAR/GZ]
        EMAIL[Email<br/>EML/MSG]
        OTHER[JSON/XML/RTF/EPUB<br/>Plain Text/Logs]
    end

    subgraph "Chunking Pipeline"
        STRATEGY[Chunking Strategy<br/>Selection]
        CHUNK[Chunk Generation]
        META[Metadata Extraction]
    end

    subgraph "Chunking Strategies"
        FIXED[Fixed Size]
        SENTENCE[Sentence-based]
        TOKEN[Token-based<br/>tiktoken]
        MARKDOWN[Markdown-aware<br/>Heading hierarchy]
        CODE_AWARE[Code-aware<br/>tree-sitter AST]
        TABLE[Table-aware]
        SEMANTIC[Semantic<br/>Similarity-based]
        RECURSIVE[Recursive Character]
        OVERLAP[Overlap]
    end

    subgraph "Storage"
        DB[(PostgreSQL<br/>Dataset Items)]
        S3[(S3<br/>Source Files)]
    end

    NATS --> WORKER
    WORKER --> DOWNLOAD
    DOWNLOAD --> S3
    DOWNLOAD --> DETECT
    DETECT --> EXTRACT

    EXTRACT --> PDF
    EXTRACT --> OFFICE
    EXTRACT --> LEGACY
    EXTRACT --> MD
    EXTRACT --> HTML
    EXTRACT --> CODE
    EXTRACT --> ARCHIVE
    EXTRACT --> EMAIL
    EXTRACT --> OTHER

    PDF --> STRATEGY
    OFFICE --> STRATEGY
    LEGACY --> STRATEGY
    MD --> STRATEGY
    HTML --> STRATEGY
    CODE --> STRATEGY
    ARCHIVE --> STRATEGY
    EMAIL --> STRATEGY
    OTHER --> STRATEGY

    STRATEGY --> FIXED
    STRATEGY --> SENTENCE
    STRATEGY --> TOKEN
    STRATEGY --> MARKDOWN
    STRATEGY --> CODE_AWARE
    STRATEGY --> TABLE
    STRATEGY --> SEMANTIC
    STRATEGY --> RECURSIVE
    STRATEGY --> OVERLAP

    FIXED --> CHUNK
    SENTENCE --> CHUNK
    TOKEN --> CHUNK
    MARKDOWN --> CHUNK
    CODE_AWARE --> CHUNK
    TABLE --> CHUNK
    SEMANTIC --> CHUNK
    RECURSIVE --> CHUNK
    OVERLAP --> CHUNK

    CHUNK --> META
    META --> DB
```

## Features

### Supported File Formats

#### Document Formats

- **PDF** (`.pdf`) - Text extraction with page metadata
- **Microsoft Word** (`.docx`) - XML-based Office format
- **Legacy Word** (`.doc`) - Binary Office format (CFB)
- **RTF** (`.rtf`) - Rich Text Format
- **EPUB** (`.epub`) - Electronic publication format
- **OpenDocument Text** (`.odt`) - OpenDocument text format

#### Spreadsheet Formats

- **Microsoft Excel** (`.xlsx`) - XML-based Office format
- **Legacy Excel** (`.xls`) - Binary Office format (CFB)
- **OpenDocument Spreadsheet** (`.ods`)

#### Presentation Formats

- **Microsoft PowerPoint** (`.pptx`) - XML-based Office format
- **Legacy PowerPoint** (`.ppt`) - Binary Office format (CFB)
- **OpenDocument Presentation** (`.odp`)

#### Markup & Web Formats

- **Markdown** (`.md`, `.markdown`) - With metadata preservation
- **HTML** (`.html`, `.htm`) - Text extraction with scraper
- **XML** (`.xml`) - Generic XML text extraction

#### Code & Data Formats

- **Source Code** - All major programming languages (see Code-Aware Chunking)
- **JSON** (`.json`) - Structured data
- **NDJSON** (`.ndjson`, `.jsonl`) - Line-delimited JSON
- **YAML** (`.yaml`, `.yml`) - Configuration files
- **TOML** (`.toml`) - Configuration files
- **Log Files** (`.log`) - Line-based logs
- **CSV** (`.csv`) - Comma-separated values

#### Archive Formats

- **ZIP** (`.zip`) - Recursive extraction
- **TAR** (`.tar`) - Tape archive
- **GZIP** (`.tar.gz`, `.tgz`) - Compressed tar
- **BZIP2** (`.tar.bz2`) - Compressed tar

#### Email Formats

- **EML** (`.eml`) - Email message format
- **MSG** (`.msg`) - Outlook message format (partial support)

#### Plain Text

- **Text Files** (`.txt`) - UTF-8 encoded text

### Chunking Strategies

#### Fixed Size

Simple character-based chunking with configurable chunk size:

```json
{
  "type": "fixed_size",
  "chunk_size": 1000,
  "overlap": 200
}
```

**Use Cases**: Simple documents, when consistency is important

#### Sentence-Based

Chunks text by sentence boundaries, respecting natural language structure:

```json
{
  "type": "sentence",
  "sentences_per_chunk": 5,
  "overlap_sentences": 1
}
```

**Use Cases**: Narrative text, articles, documentation

#### Token-Based

Chunks by token count using `tiktoken` (OpenAI tokenizer):

```json
{
  "type": "token_based",
  "max_tokens": 512,
  "overlap_tokens": 50,
  "encoding": "cl100k_base"
}
```

**Supported Encodings**:
- `cl100k_base` - GPT-4, GPT-3.5-turbo, text-embedding-ada-002
- `p50k_base` - Codex models
- `r50k_base` - GPT-3 models (davinci, curie, babbage, ada)

**Use Cases**: Optimizing for embedding model token limits

#### Markdown-Aware

Preserves Markdown structure and heading hierarchy:

```json
{
  "type": "markdown_aware",
  "max_chunk_size": 1500,
  "respect_headings": true,
  "include_heading_hierarchy": true
}
```

**Use Cases**: Markdown documentation, READMEs, technical docs

**Features**:
- Preserves heading context in metadata
- Respects code blocks
- Maintains list structure
- Preserves links and images

#### Code-Aware

Tree-sitter AST-based chunking for source code:

```json
{
  "type": "code_aware",
  "max_chunk_size": 2000,
  "chunk_by": "function",
  "include_docstrings": true
}
```

**Supported Languages** (via tree-sitter):
- **Rust** (`.rs`)
- **Python** (`.py`)
- **JavaScript** (`.js`, `.mjs`)
- **TypeScript** (`.ts`, `.tsx`)
- **Go** (`.go`)
- **Java** (`.java`)
- **C** (`.c`, `.h`)
- **C++** (`.cpp`, `.hpp`, `.cc`, `.cxx`)
- **Bash** (`.sh`, `.bash`)
- **HTML** (`.html`, `.htm`)
- **CSS** (`.css`, `.scss`)
- **JSON** (`.json`)
- **YAML** (`.yaml`, `.yml`)
- **TOML** (`.toml`)

**Chunking Modes**:
- `function` - Chunk by function/method definitions
- `class` - Chunk by class definitions
- `module` - Chunk by module/file
- `statement` - Chunk by top-level statements

**Use Cases**: Code repositories, API documentation, code search

#### Table-Aware

Detects and preserves table structures:

```json
{
  "type": "table_aware",
  "max_chunk_size": 1000,
  "keep_tables_together": true
}
```

**Use Cases**: Spreadsheets, structured documents, CSV files

#### Semantic

Similarity-based chunking using embeddings:

```json
{
  "type": "semantic",
  "similarity_threshold": 0.75,
  "embedder_id": "embed-1"
}
```

**Use Cases**: When semantic coherence is critical

**Note**: Requires embedder configuration to be passed in job

#### Recursive Character

Hierarchical splitting with multiple separators:

```json
{
  "type": "recursive_character",
  "chunk_size": 1000,
  "overlap": 200,
  "separators": ["\n\n", "\n", ". ", " "]
}
```

**Use Cases**: General-purpose chunking

#### Overlap

Adds configurable overlap between chunks:

```json
{
  "type": "overlap",
  "base_strategy": "sentence",
  "overlap_size": 200
}
```

**Use Cases**: Improving context continuity for search

### Extraction Configuration

All extractors support structure-preserving extraction:

```json
{
  "strategy": "structure_preserving",
  "options": {
    "preserve_formatting": false,
    "extract_tables": true,
    "table_format": "plain_text",
    "preserve_headings": true,
    "heading_format": "plain_text",
    "preserve_lists": true,
    "preserve_code_blocks": true,
    "include_metadata": true,
    "append_metadata_to_text": true
  }
}
```

**Extraction Strategies**:
- `plain_text` - Simple text extraction
- `structure_preserving` - Preserves document structure
- `markdown` - Converts to Markdown format

**Options**:
- `preserve_formatting` - Keep whitespace and formatting
- `extract_tables` - Extract table content
- `table_format` - Table output format: `plain_text`, `markdown`, `csv`
- `preserve_headings` - Preserve heading structure
- `heading_format` - Heading format: `plain_text`, `markdown`
- `preserve_lists` - Preserve list formatting
- `preserve_code_blocks` - Preserve code block formatting
- `include_metadata` - Extract document metadata (author, title, dates)
- `append_metadata_to_text` - Append metadata as text for chunking

## Configuration

All configuration via environment variables:

### Worker Configuration

```bash
# NATS connection (inherited from core)
NATS_URL=nats://localhost:4222

# Database connection (inherited from core)
DATABASE_URL=postgresql://localhost/semantic_explorer

# S3 configuration (inherited from core)
S3_ENDPOINT=http://localhost:9000
S3_ACCESS_KEY=minioadmin
S3_SECRET_KEY=minioadmin
S3_BUCKET=semantic-explorer

# Worker concurrency
WORKER_CONCURRENCY=4

# Max retries for failed jobs
MAX_RETRIES=3

# Job processing timeout (seconds)
JOB_TIMEOUT=300
```

### Extraction Configuration

```bash
# Max file size to process (MB)
MAX_FILE_SIZE_MB=100

# PDF extraction timeout (seconds)
PDF_TIMEOUT=60

# Archive extraction depth (prevent zip bombs)
MAX_ARCHIVE_DEPTH=3

# Max archive extracted size (MB)
MAX_ARCHIVE_SIZE_MB=500

# Enable/disable specific extractors
ENABLE_PDF_EXTRACTION=true
ENABLE_OFFICE_EXTRACTION=true
ENABLE_CODE_EXTRACTION=true
ENABLE_ARCHIVE_EXTRACTION=true
```

### Chunking Configuration

```bash
# Default chunk size (characters)
DEFAULT_CHUNK_SIZE=1000

# Default overlap (characters)
DEFAULT_OVERLAP=200

# Minimum chunk size (characters)
MIN_CHUNK_SIZE=100

# Maximum chunk size (characters)
MAX_CHUNK_SIZE=5000

# Token encoding for token-based chunking
TOKEN_ENCODING=cl100k_base

# Enable code-aware chunking
ENABLE_CODE_AWARE_CHUNKING=true
```

### Observability Configuration

```bash
# OpenTelemetry
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=worker-collections
OTEL_SERVICE_VERSION=1.0.0

# Logging
RUST_LOG=info,worker_collections=debug
LOG_FORMAT=json
```

## Building

### Debug Build

```bash
cargo build -p worker-collections
```

### Release Build

```bash
cargo build -p worker-collections --release
```

Binary location: `target/release/worker-collections`

### Docker Build

```bash
# Build from repository root
docker build -f crates/worker-collections/Dockerfile -t worker-collections:latest .
```

## Running

### Local Development

```bash
# Set required environment variables
export DATABASE_URL=postgresql://localhost/semantic_explorer
export NATS_URL=nats://localhost:4222
export S3_ENDPOINT=http://localhost:9000
export S3_ACCESS_KEY=minioadmin
export S3_SECRET_KEY=minioadmin
export S3_BUCKET=semantic-explorer

# Run with cargo
cargo run -p worker-collections

# Or run the compiled binary
./target/release/worker-collections
```

### Docker

```bash
docker run -d \
  --name worker-collections \
  -e DATABASE_URL=postgresql://postgres:password@postgres:5432/semantic_explorer \
  -e NATS_URL=nats://nats:4222 \
  -e S3_ENDPOINT=http://minio:9000 \
  -e S3_ACCESS_KEY=minioadmin \
  -e S3_SECRET_KEY=minioadmin \
  -e S3_BUCKET=semantic-explorer \
  worker-collections:latest
```

### Health Check

Worker health can be monitored via:
- NATS consumer health (check pending messages)
- Prometheus metrics
- Structured logs

```bash
# Check NATS consumer status
nats consumer info COLLECTION_TRANSFORMS worker-collections

# Check metrics
curl http://localhost:9090/metrics | grep worker_collections
```

## Deployment

### Docker Compose

```yaml
services:
  worker-collections:
    image: worker-collections:latest
    environment:
      DATABASE_URL: postgresql://postgres:password@postgres:5432/semantic_explorer
      NATS_URL: nats://nats:4222
      S3_ENDPOINT: http://minio:9000
      S3_ACCESS_KEY: minioadmin
      S3_SECRET_KEY: minioadmin
      S3_BUCKET: semantic-explorer
      RUST_LOG: info
      WORKER_CONCURRENCY: 4
    depends_on:
      - postgres
      - nats
      - minio
    restart: unless-stopped
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: worker-collections
spec:
  serviceName: worker-collections
  replicas: 3
  selector:
    matchLabels:
      app: worker-collections
  template:
    metadata:
      labels:
        app: worker-collections
    spec:
      containers:
      - name: worker-collections
        image: worker-collections:latest
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: semantic-explorer-secrets
              key: database-url
        - name: NATS_URL
          value: nats://nats:4222
        - name: S3_ENDPOINT
          value: http://minio:9000
        - name: S3_ACCESS_KEY
          valueFrom:
            secretKeyRef:
              name: semantic-explorer-secrets
              key: s3-access-key
        - name: S3_SECRET_KEY
          valueFrom:
            secretKeyRef:
              name: semantic-explorer-secrets
              key: s3-secret-key
        - name: WORKER_CONCURRENCY
          value: "4"
        resources:
          requests:
            memory: 1Gi
            cpu: 500m
          limits:
            memory: 2Gi
            cpu: 2000m
```

## Job Processing Flow

```mermaid
sequenceDiagram
    participant API
    participant NATS
    participant Worker
    participant S3
    participant DB

    API->>NATS: Publish CollectionTransform job
    Worker->>NATS: Pull job (with ack)
    Worker->>DB: Fetch transform config
    Worker->>S3: List files in collection

    loop For each file
        Worker->>S3: Download file
        Worker->>Worker: Detect MIME type
        Worker->>Worker: Extract text
        Worker->>Worker: Chunk text
        Worker->>DB: Store chunks as DatasetItems
        Worker->>NATS: Publish progress update
    end

    Worker->>DB: Update transform status
    Worker->>NATS: ACK job
    Worker->>NATS: Publish completion status
```

## Metrics

Prometheus metrics exposed on port 9090 (configurable):

### Job Metrics

```
# Total jobs processed
collection_transform_jobs_total{status="success|failure"}

# Job processing duration
collection_transform_duration_seconds

# Active jobs currently processing
collection_transform_active_jobs

# Files processed per job
collection_transform_files_processed

# Chunks created per job
collection_transform_items_created
```

### Extraction Metrics

```
# Extraction count by format
file_extraction_total{format="pdf|docx|md|..."}

# Extraction duration by format
file_extraction_duration_seconds{format="pdf|docx|md|..."}

# Extraction errors
file_extraction_errors_total{format="pdf|docx|md|...",error_type="..."}

# Bytes extracted
file_extraction_bytes{format="pdf|docx|md|..."}
```

### Chunking Metrics

```
# Chunks created by strategy
chunks_created_total{strategy="sentence|code_aware|..."}

# Chunking duration
chunking_duration_seconds{strategy="sentence|code_aware|..."}

# Average chunk size
chunk_size_bytes{strategy="sentence|code_aware|..."}

# Chunk count distribution
chunk_count_per_document{strategy="sentence|code_aware|..."}
```

### Worker Metrics

```
# Worker health
worker_ready{worker="worker-collections"}

# Active worker goroutines/tasks
worker_active_jobs{worker="worker-collections"}

# Job retries
worker_job_retries_total{worker="worker-collections"}

# Worker failures
worker_job_failures_total{worker="worker-collections",error_type="..."}
```

## Error Handling

### Retry Logic

Failed jobs are automatically retried with exponential backoff:

1. **First retry**: 30 seconds
2. **Second retry**: 2 minutes
3. **Third retry**: 5 minutes
4. **Fourth retry**: 15 minutes
5. **Fifth retry**: 1 hour
6. **After max attempts**: Move to Dead Letter Queue (DLQ)

### Dead Letter Queue

Jobs that exceed max delivery attempts are routed to `DLQ_COLLECTION_TRANSFORMS` stream:

```bash
# Inspect DLQ
nats stream info DLQ_COLLECTION_TRANSFORMS

# Get DLQ messages
nats consumer next DLQ_COLLECTION_TRANSFORMS worker-collections-dlq --count 10

# Replay DLQ message
nats pub COLLECTION_TRANSFORMS "$(nats consumer next DLQ_COLLECTION_TRANSFORMS worker-collections-dlq --count 1 --no-ack)"
```

### Common Errors

#### File too large

```
Error: File size 150MB exceeds MAX_FILE_SIZE_MB limit of 100MB
```

**Solution**: Increase `MAX_FILE_SIZE_MB` or split the file

#### Unsupported format

```
Error: No extractor available for MIME type: video/mp4
```

**Solution**: Convert to supported format or add custom extractor

#### Extraction timeout

```
Error: PDF extraction timeout after 60s
```

**Solution**: Increase `PDF_TIMEOUT` or optimize PDF (remove images, compress)

#### Archive too deep

```
Error: Archive nesting depth 5 exceeds MAX_ARCHIVE_DEPTH of 3
```

**Solution**: Extract archive manually or increase `MAX_ARCHIVE_DEPTH`

#### Archive bomb detected

```
Error: Extracted archive size 600MB exceeds MAX_ARCHIVE_SIZE_MB of 500MB
```

**Solution**: Increase `MAX_ARCHIVE_SIZE_MB` or extract selectively

## Development

### Project Structure

```
src/
├── main.rs                   # Worker initialization
├── job.rs                    # Job processing logic
├── extract/                  # Text extraction
│   ├── mod.rs
│   ├── config.rs             # Extraction configuration
│   ├── error.rs              # Extraction errors
│   ├── service.rs            # Extraction orchestration
│   ├── plain_text/           # Plain text files
│   ├── pdf/                  # PDF extraction
│   ├── office/               # Modern Office (DOCX/XLSX/PPTX)
│   │   ├── document.rs
│   │   ├── spreadsheet.rs
│   │   └── presentation.rs
│   ├── open_office/          # OpenDocument formats
│   │   ├── open_document.rs
│   │   ├── open_spreadsheet.rs
│   │   └── open_presentation.rs
│   ├── legacy_doc/           # Legacy .doc files
│   ├── legacy_xls/           # Legacy .xls files
│   ├── legacy_ppt/           # Legacy .ppt files
│   ├── epub/                 # EPUB ebooks
│   ├── rtf/                  # RTF documents
│   ├── html/                 # HTML documents
│   ├── xml/                  # XML documents
│   ├── json/                 # JSON/NDJSON
│   ├── email/                # Email (.eml, .msg)
│   ├── log/                  # Log files
│   ├── markdown/             # Markdown files
│   └── archive/              # ZIP/TAR/GZ archives
└── chunk/                    # Text chunking
    ├── mod.rs
    ├── config.rs             # Chunking configuration
    ├── service.rs            # Chunking orchestration
    ├── metadata.rs           # Chunk metadata
    └── strategies/           # Chunking strategies
        ├── fixed_size.rs
        ├── sentence.rs
        ├── token_based.rs
        ├── markdown_aware.rs
        ├── code_aware.rs
        ├── table_aware.rs
        ├── semantic.rs
        ├── recursive_character.rs
        └── overlap.rs
```

### Adding New File Format

1. Create extractor module in `src/extract/<format>/mod.rs`
2. Implement extraction logic:
   ```rust
   pub fn extract(content: &[u8]) -> Result<String, ExtractError> {
       // Implementation...
   }
   ```
3. Register in `src/extract/service.rs`:
   ```rust
   match mime_type {
       "application/myformat" => my_format::extract(content),
       // ...
   }
   ```
4. Add MIME type detection
5. Add tests in `tests/<format>_test.rs`

### Adding New Chunking Strategy

1. Create strategy module in `src/chunk/strategies/<strategy>.rs`
2. Implement `ChunkStrategy` trait:
   ```rust
   pub struct MyStrategy {
       config: MyStrategyConfig,
   }

   impl ChunkStrategy for MyStrategy {
       fn chunk(&self, text: &str, metadata: &ChunkMetadata) -> Result<Vec<Chunk>> {
           // Implementation...
       }
   }
   ```
3. Register in `src/chunk/service.rs`
4. Add configuration in `src/chunk/config.rs`
5. Add tests

### Testing

```bash
# Run all tests
cargo test -p worker-collections

# Run with logging
RUST_LOG=debug cargo test -p worker-collections -- --nocapture

# Test specific extractor
cargo test -p worker-collections pdf_extraction

# Test chunking strategy
cargo test -p worker-collections code_aware_chunking

# Integration tests (requires dependencies)
cargo test -p worker-collections --test '*' -- --ignored
```

**Test Coverage**: 211+ tests

## Troubleshooting

### Worker not consuming jobs

```bash
# Check NATS consumer status
nats consumer info COLLECTION_TRANSFORMS worker-collections

# Check consumer lag
nats stream info COLLECTION_TRANSFORMS

# Check worker logs
docker logs worker-collections -f --tail 100
```

### Extraction failures

```bash
# Enable debug logging
export RUST_LOG=worker_collections=debug,worker_collections::extract=trace

# Check specific file
curl -o test.pdf http://example.com/sample.pdf
# Manual extraction test (if implemented)
```

### Memory issues

```bash
# Reduce worker concurrency
export WORKER_CONCURRENCY=2

# Limit file size
export MAX_FILE_SIZE_MB=50

# Monitor memory usage
docker stats worker-collections

# Check for memory leaks
RUST_LOG=debug cargo run -p worker-collections
```

### Code-aware chunking not working

```bash
# Verify tree-sitter grammars are compiled
cargo tree -p worker-collections | grep tree-sitter

# Enable debug logging for code chunking
export RUST_LOG=worker_collections::chunk::strategies::code_aware=debug

# Test specific language
cargo test -p worker-collections code_aware_rust
```

### High error rates

```bash
# Check Prometheus metrics
curl http://localhost:9090/metrics | grep file_extraction_errors

# Review failed jobs
nats consumer info COLLECTION_TRANSFORMS worker-collections | grep "Num Pending"

# Check DLQ
nats stream info DLQ_COLLECTION_TRANSFORMS
```

## Performance Tuning

### Throughput Optimization

```bash
# Increase concurrency (requires more CPU/memory)
export WORKER_CONCURRENCY=8

# Enable parallel file processing within jobs
export PARALLEL_FILE_PROCESSING=true

# Increase batch size for database inserts
export DB_BATCH_SIZE=500

# Disable unnecessary extractors
export ENABLE_LEGACY_OFFICE=false
```

### Memory Optimization

```bash
# Reduce max file size
export MAX_FILE_SIZE_MB=50

# Reduce chunk size
export DEFAULT_CHUNK_SIZE=500

# Limit archive depth (prevent zip bombs)
export MAX_ARCHIVE_DEPTH=2

# Disable archive extraction if not needed
export ENABLE_ARCHIVE_EXTRACTION=false
```

### Latency Optimization

```bash
# Increase timeouts for large files
export PDF_TIMEOUT=120
export JOB_TIMEOUT=600

# Reduce chunk overlap (faster chunking)
export DEFAULT_OVERLAP=50
```

## Dependencies

Key dependencies:

- **pdf-extract** - PDF text extraction
- **lopdf** - PDF structure parsing
- **quick-xml** - XML parsing for Office formats
- **zip** - ZIP archive extraction
- **tar** - TAR archive extraction
- **flate2** - GZIP compression
- **scraper** - HTML text extraction
- **tree-sitter** + language grammars - Code parsing for AST-based chunking
- **tiktoken-rs** - OpenAI tokenizer for token-based chunking
- **mail-parser** - Email parsing
- **rtf-parser** - RTF text extraction
- **epub** - EPUB ebook format
- **cfb** - Compound File Binary (legacy Office)
- **unicode-segmentation** - Text boundary detection
- **unicode-normalization** - Text normalization
- **regex** - Pattern matching
- **mime_guess** - MIME type detection

See [Cargo.toml](Cargo.toml) for complete list.

## License

Apache License 2.0
