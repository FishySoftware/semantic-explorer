# Collections Worker

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Document extraction and chunking worker for Semantic Explorer**

</div>

Processes files from the `COLLECTION_TRANSFORMS` NATS stream.

---

## Overview

The collections worker:

1. Consumes jobs from `COLLECTION_TRANSFORMS` NATS stream
2. Downloads files from S3
3. Extracts text using format-specific parsers (runs in blocking thread pool)
4. Chunks content using configurable strategies
5. Uploads results back to S3 as JSON documents

### Performance Features

- **Non-blocking extraction**: CPU-intensive extraction runs in `spawn_blocking` thread pool
- **Circuit breakers**: S3 operations protected by circuit breaker pattern
- **Configurable retries**: Exponential backoff for transient failures
- **Tunable NATS consumers**: Configurable ack pending and wait times

---

## Supported File Formats

<details>
<summary><strong>Documents</strong></summary>

| Format | Extensions |
|--------|------------|
| PDF | `.pdf` |
| Word | `.docx` |
| Legacy Word | `.doc` |
| RTF | `.rtf` |
| EPUB | `.epub` |
| OpenDocument | `.odt` |

</details>

<details>
<summary><strong>Spreadsheets</strong></summary>

| Format | Extensions |
|--------|------------|
| Excel | `.xlsx` |
| Legacy Excel | `.xls` |
| OpenDocument | `.ods` |

</details>

<details>
<summary><strong>Presentations</strong></summary>

| Format | Extensions |
|--------|------------|
| PowerPoint | `.pptx` |
| Legacy PowerPoint | `.ppt` |
| OpenDocument | `.odp` |

</details>

<details>
<summary><strong>Markup & Web</strong></summary>

| Format | Extensions |
|--------|------------|
| Markdown | `.md`, `.markdown` |
| HTML | `.html`, `.htm` |
| XML | `.xml` |

</details>

<details>
<summary><strong>Code & Data</strong></summary>

| Format | Extensions |
|--------|------------|
| JSON | `.json` |
| Log | `.log` |
| Plain Text | `.txt` |

</details>

<details>
<summary><strong>Archives</strong></summary>

| Format | Extensions |
|--------|------------|
| ZIP | `.zip` |
| TAR | `.tar` |
| GZIP | `.tar.gz`, `.tgz` |

</details>

<details>
<summary><strong>Email</strong></summary>

| Format | Extensions |
|--------|------------|
| EML | `.eml` |
| MSG | `.msg` |

</details>

---

## Chunking Strategies

| Strategy | Description |
|----------|-------------|
| `sentence` | Sentence-based boundaries (default) |
| `fixed_size` | Character-based chunks |
| `token_based` | Token count using tiktoken |
| `markdown_aware` | Preserves Markdown structure |
| `code_aware` | AST-based via tree-sitter |
| `table_aware` | Preserves table structures |
| `semantic` | Similarity-based grouping |
| `recursive_character` | Hierarchical separators |

### Code-Aware Chunking

Tree-sitter support for:

Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Bash, HTML, CSS, JSON, YAML, TOML

---

## Extraction Strategies

| Strategy | Description |
|----------|-------------|
| `plain_text` | Simple text extraction (default) |
| `structure_preserving` | Preserves document structure |
| `markdown` | Converts to Markdown format |

### Extraction Options

- `preserve_formatting` - Keep whitespace
- `extract_tables` - Extract table content
- `table_format` - `plain_text`, `markdown`, or `csv`
- `preserve_headings` - Keep heading structure
- `heading_format` - `plain_text` or `markdown`
- `preserve_lists` - Keep list formatting
- `preserve_code_blocks` - Keep code blocks
- `include_metadata` - Extract document metadata
- `append_metadata_to_text` - Append metadata for chunking

---

## Environment Variables

### Worker-Specific

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVICE_NAME` | `worker-collections` | Service name for telemetry |
| `NATS_URL` | `nats://localhost:4222` | NATS server URL |
| `MAX_CONCURRENT_JOBS` | `10` | Concurrent job limit |

### S3 Storage (from core)

| Variable | Required | Description |
|----------|----------|-------------|
| `AWS_REGION` | Yes | S3 region |
| `AWS_ENDPOINT_URL` | Yes | S3 endpoint URL |
| `AWS_ACCESS_KEY_ID` | No* | S3 access key |
| `AWS_SECRET_ACCESS_KEY` | No* | S3 secret key |
| `S3_FORCE_PATH_STYLE` | No | Use path-style URLs (for MinIO) |
| `MAX_FILE_SIZE_MB` | `100` | Max file size to process |

*Uses AWS default credential chain if not set

### Observability

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP collector |
| `RUST_LOG` | `info` | Log level |

### NATS Consumer Tuning

| Variable | Default | Description |
|----------|---------|-------------|
| `NATS_MAX_ACK_PENDING` | `100` | Max unacknowledged messages per consumer |
| `NATS_COLLECTION_ACK_WAIT_SECS` | `600` | Ack wait for collection transforms |

### Circuit Breaker Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `S3_CIRCUIT_BREAKER_FAILURE_THRESHOLD` | `5` | Failures before circuit opens |
| `S3_CIRCUIT_BREAKER_SUCCESS_THRESHOLD` | `3` | Successes to close from half-open |
| `S3_CIRCUIT_BREAKER_TIMEOUT_SECS` | `30` | How long circuit stays open |

### Retry Policy Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RETRY_MAX_ATTEMPTS` | `3` | Maximum retry attempts |
| `RETRY_INITIAL_DELAY_MS` | `100` | Initial delay between retries (ms) |
| `S3_RETRY_MAX_ATTEMPTS` | `3` | S3-specific retry attempts |

---

## Building

```bash
# Debug build
cargo build -p worker-collections

# Release build
cargo build -p worker-collections --release
```

### Docker

```bash
docker build -f crates/worker-collections/Dockerfile -t worker-collections:latest .
```

---

## Running

```bash
# Set required environment variables
export AWS_REGION=us-east-1
export AWS_ENDPOINT_URL=http://localhost:9000
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
export NATS_URL=nats://localhost:4222

cargo run -p worker-collections
```

---

## Metrics

Worker metrics via `record_worker_job`:

| Metric | Labels | Description |
|--------|--------|-------------|
| `worker_job_duration_seconds` | `job_type`, `status` | Job processing time |
| `worker_job_total` | `job_type`, `status` | Job count |

Status values: `success`, `failed_validation`, `failed_download`, `failed_file_too_large`, `failed_extraction`, `failed_config_parse`, `failed_chunking`, `failed_empty_chunks`, `failed_upload`

---

## Job Payload

Jobs from the NATS stream:

```json
{
  "job_id": "uuid",
  "collection_transform_id": 123,
  "collection_id": 456,
  "bucket": "semantic-explorer",
  "source_file_key": "document.pdf",
  "extraction_config": {
    "strategy": "plain_text",
    "options": {}
  },
  "chunking_config": {
    "strategy": "sentence",
    "chunk_size": 1000,
    "overlap": 200
  }
}
```

---

## License

Apache License 2.0
