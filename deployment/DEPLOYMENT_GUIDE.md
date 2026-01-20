# Semantic Explorer Deployment Guide

Comprehensive guide for deploying Semantic Explorer in production using Kubernetes and Helm.

## Table of Contents

- [Quick Start](#quick-start)
- [Security Configuration](#security-configuration)
- [Helm Chart Configuration](#helm-chart-configuration)
- [Helm Chart Dependencies](#helm-chart-dependencies)
- [Example Values Files](#example-values-files)
- [Air-Gapped Deployments](#air-gapped-deployments)
- [Environment Variables](#environment-variables)
- [Health Checks and Monitoring](#health-checks-and-monitoring)
- [Observability and Metrics](#observability-and-metrics)
- [NATS Stream Configuration](#nats-stream-configuration)
- [Worker Concurrency Tuning](#worker-concurrency-tuning)
- [Production Best Practices](#production-best-practices)
- [Helm Chart Reference](#helm-chart-reference)
- [Support and Troubleshooting](#support-and-troubleshooting)

## Quick Start

### 1. Generate Encryption Master Key

Before deploying, generate a secure 256-bit encryption key for API key encryption at rest:

```bash
# Generate the master key
MASTER_KEY=$(openssl rand -hex 32)
echo "ENCRYPTION_MASTER_KEY=$MASTER_KEY"

# Store securely in AWS Secrets Manager or HashiCorp Vault
# DO NOT commit to version control
```

### 2. Create Helm Values Override

```bash
# Create custom values file
cat > custom-values.yaml <<EOF
# Encryption Configuration
encryption:
  masterKey: "$MASTER_KEY"

# Storage Configuration
storage:
  s3:
    endpoint: "https://s3.amazonaws.com"
    region: "us-east-1"
    accessKeyId: "$AWS_ACCESS_KEY_ID"
    secretAccessKey: "$AWS_SECRET_ACCESS_KEY"

# PostgreSQL Configuration
postgresql:
  external:
    enabled: true
    host: "postgres.example.com"
    port: 5432
    database: "semantic_explorer"
    username: "explorer_user"
    password: "$DB_PASSWORD"
    sslMode: "require"

# NATS Configuration
nats:
  external:
    enabled: true
    url: "nats://nats.example.com:4222"

# Qdrant Configuration
qdrant:
  external:
    enabled: true
    url: "http://qdrant.example.com:6334"

# OIDC Configuration
dex:
  external:
    enabled: true
    issuerUrl: "https://auth.example.com"
    clientId: "$OIDC_CLIENT_ID"
    clientSecret: "$OIDC_CLIENT_SECRET"
EOF
```

### 3. Deploy with Helm

```bash
helm install semantic-explorer ./deployment/helm/semantic-explorer \
  -f custom-values.yaml \
  -n semantic-explorer \
  --create-namespace
```

## Security Configuration

### Encryption Master Key Management

The system uses AES-256-GCM encryption for all API keys stored at rest in the database. The encryption master key is critical for security.

**Key Generation:**
```bash
# Generate a new 256-bit (64 hex character) key
openssl rand -hex 32

# Output example:
# a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a
```

**Storage Requirements:**
- Store in AWS Secrets Manager, HashiCorp Vault, or similar
- Never commit to version control
- Restrict access to ops/infrastructure team only
- Enable audit logging for key access

**Helm Configuration:**
```yaml
encryption:
  masterKey: "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a"
```

**Key Rotation:**
To rotate the encryption key:
1. Generate a new key with `openssl rand -hex 32`
2. Create a database migration to re-encrypt all stored secrets
3. Deploy the migration
4. Update `ENCRYPTION_MASTER_KEY` to the new value
5. Verify all keys were rotated successfully
6. Store old key securely for recovery purposes

### Secret Management

**Required Secrets (in Helm values):**
- `encryption.masterKey` - AES-256-GCM master key (64 hex chars)
- `postgresql.external.password` - Database password
- `storage.s3.accessKeyId` - S3 access key
- `storage.s3.secretAccessKey` - S3 secret key
- `dex.external.clientSecret` - OIDC client secret (if external Dex)

**Production Configuration:**

Use external secret management:

```bash
# AWS Secrets Manager
aws secretsmanager create-secret \
  --name semantic-explorer/encryption-key \
  --secret-string "$MASTER_KEY"

# Use with Helm (external secret operator)
apiVersion: external-secrets.io/v1beta1
kind: SecretStore
metadata:
  name: aws-secrets
spec:
  provider:
    aws:
      service: SecretsManager
      region: us-east-1
      auth:
        jwt:
          serviceAccountRef:
            name: external-secrets-sa
---
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: semantic-explorer-secrets
spec:
  secretStoreRef:
    name: aws-secrets
    kind: SecretStore
  target:
    name: semantic-explorer-secrets
    creationPolicy: Owner
  data:
  - secretKey: encryption-master-key
    remoteRef:
      key: semantic-explorer/encryption-key
```

## Helm Chart Configuration

### Minimal Production Deployment

```yaml
# Replicate API across 3 nodes
api:
  replicaCount: 3
  resources:
    requests:
      cpu: 2000m
      memory: 2Gi
    limits:
      cpu: 4000m
      memory: 8Gi
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10
    targetCPUUtilizationPercentage: 70

# 3 worker replicas for file extraction
workerCollections:
  replicaCount: 3
  resources:
    requests:
      cpu: 2000m
      memory: 1Gi
    limits:
      cpu: 4000m
      memory: 2Gi
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10

# 3 worker replicas for embeddings
workerDatasets:
  replicaCount: 3
  resources:
    requests:
      cpu: 2000m
      memory: 2Gi
    limits:
      cpu: 4000m
      memory: 8Gi
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 8

# 2-3 worker replicas for visualizations (GPU recommended)
workerVisualizationsPy:
  replicaCount: 2
  resources:
    requests:
      cpu: 4000m
      memory: 8Gi
    limits:
      cpu: 8000m
      memory: 16Gi
  autoscaling:
    enabled: true
    minReplicas: 2
    maxReplicas: 6

# External services (recommended for production)
postgresql:
  external:
    enabled: true
    sslMode: require

nats:
  external:
    enabled: true

qdrant:
  external:
    enabled: true

# Observability
observability:
  otelCollector:
    enabled: true
  grafana:
    enabled: true
  quickwit:
    enabled: true
```

## Helm Chart Dependencies

The Semantic Explorer Helm chart includes optional dependencies for infrastructure components. These can be enabled for self-contained deployments or disabled when using external managed services.

| Dependency | Chart Version | Repository | Condition |
|------------|---------------|------------|-----------|
| 
| nats | 1.2.x | nats-io/k8s/helm/charts | `nats.enabled` |
| qdrant | 0.8.x | qdrant/qdrant-helm | `qdrant.enabled` |
| minio | 5.2.x | charts.min.io | `minio.enabled` |
| grafana | 8.x | grafana/helm-charts | `grafana.enabled` |
| prometheus | 25.x | prometheus-community/helm-charts | `prometheus.enabled` |

**Additional components (not subcharts, deployed as templates):**
- PostgreSQL (single-node deployment)
- OpenTelemetry Collector
- Quickwit (log/trace aggregation)
- Dex (OIDC provider)

### Updating Dependencies

```bash
# Update chart dependencies
cd deployment/helm/semantic-explorer
helm dependency update
```

## Example Values Files

The chart includes pre-configured values files for common deployment scenarios in `deployment/helm/semantic-explorer/examples/`:

| File | Use Case | Description |
|------|----------|-------------|
| `values-all-included-dev.yaml` | Local Development | All infrastructure enabled, minimal resources (500m CPU, 512Mi per pod). Suitable for local testing and CI/CD. |
| `values-all-included-prod.yaml` | Self-Hosted Production | Full HA deployment with all infrastructure. PostgreSQL HA, NATS JetStream, NATS JetStream, 3+ replicas per service. |
| `values-external-infra-dev.yaml` | Cloud Development | Only app services deployed. Connect to cloud-managed PostgreSQL, NATS, S3, etc. |
| `values-external-infra-prod.yaml` | Cloud Production | HA app services with external managed infrastructure. Extensive docs for AWS/GCP/Azure equivalents. |
| `values-airgapped.yaml` | Air-Gapped (External Infra) | Air-gapped Kubernetes with external infrastructure. No CRDs, no operators, internal registry support. |
| `values-airgapped-all-included.yaml` | Air-Gapped (Self-Contained) | Complete air-gapped deployment. All infrastructure included, internal registry, RKE2-compatible. |

### Install with Example Values

```bash
# Development (all-included)
helm install semantic-explorer ./deployment/helm/semantic-explorer \
  -f ./deployment/helm/semantic-explorer/examples/values-all-included-dev.yaml \
  -n semantic-explorer --create-namespace

# Production with external infrastructure
helm install semantic-explorer ./deployment/helm/semantic-explorer \
  -f ./deployment/helm/semantic-explorer/examples/values-external-infra-prod.yaml \
  --set postgresql.external.host="prod-db.example.com" \
  --set postgresql.external.password="$DB_PASSWORD" \
  --set storage.s3.endpoint="https://s3.amazonaws.com" \
  --set storage.s3.accessKeyId="$AWS_ACCESS_KEY_ID" \
  --set storage.s3.secretAccessKey="$AWS_SECRET_ACCESS_KEY" \
  -n semantic-explorer --create-namespace

# Air-gapped production
helm install semantic-explorer ./deployment/helm/semantic-explorer \
  -f ./deployment/helm/semantic-explorer/examples/values-airgapped-all-included.yaml \
  --set global.imageRegistry="registry.internal.example.com" \
  -n semantic-explorer --create-namespace
```

## Air-Gapped Deployments

For air-gapped Kubernetes clusters (e.g., RKE2, OpenShift in disconnected environments), use the air-gapped values files which:

- **Disable CRDs**: ServiceMonitors and other CRD-based resources are disabled
- **Internal Registry**: Configure `global.imageRegistry` to your internal registry
- **Non-Root Containers**: All containers run as non-root users
- **No External Dependencies**: No internet access required post-deployment

### Prerequisites for Air-Gapped

1. **Push Required Images** to your internal registry:
```bash
# List of required images
IMAGES=(
  "jofish89/semantic-explorer:latest"
  "jofish89/worker-collections:latest"
  "jofish89/worker-datasets:latest"
  "jofish89/worker-visualizations-py:latest"
  "jofish89/embedding-inference-api:cuda"
  "jofish89/llm-inference-api:cuda"
  "postgres:16.3-alpine"
  "nats:2.10-alpine"
  "qdrant/qdrant:v1.16.3"
  "minio/minio:latest"
  "dexidp/dex:latest"
  "otel/opentelemetry-collector-contrib:latest"
  "quickwit/quickwit:latest"
  "grafana/grafana:latest"
  "prom/prometheus:latest"
  "busybox:1.36"
)

# Pull and push to internal registry
for img in "${IMAGES[@]}"; do
  docker pull "docker.io/$img"
  docker tag "docker.io/$img" "registry.internal.example.com/$img"
  docker push "registry.internal.example.com/$img"
done
```

2. **Create Image Pull Secret**:
```bash
kubectl create secret docker-registry registry-credentials \
  --docker-server=registry.internal.example.com \
  --docker-username=<user> \
  --docker-password=<password> \
  -n semantic-explorer
```

3. **Deploy with Air-Gapped Values**:
```bash
helm install semantic-explorer ./deployment/helm/semantic-explorer \
  -f ./deployment/helm/semantic-explorer/examples/values-airgapped-all-included.yaml \
  --set global.imageRegistry="registry.internal.example.com" \
  --set global.storageClass="local-path" \
  -n semantic-explorer --create-namespace
```

### Network Policies

Network policies are enabled by default and restrict traffic:

```yaml
networkPolicy:
  enabled: true
  ingress:
    fromIngress: true  # Allow from ingress controller
  egress:
    toDns: true        # Allow DNS lookups
    toInternet: true   # Allow external API calls
```

## Environment Variables

### API Server

| Variable | Value | Notes |
|----------|-------|-------|
| `RUST_LOG` | `warn,semantic_explorer=info` | Logging configuration |
| `HOSTNAME` | `0.0.0.0` | Bind address |
| `SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown timeout |
| `LOG_FORMAT` | `json` | Structured JSON logging |
| `DB_MAX_CONNECTIONS` | `20` | Connection pool size (per replica) |
| `DB_MIN_CONNECTIONS` | `2` | Minimum pool connections |
| `NATS_REPLICAS` | `3` | Stream replication factor |

Set in Helm values.yaml:
```yaml
api:
  env:
    NATS_REPLICAS: "3"
    DB_MAX_CONNECTIONS: "20"
    RUST_LOG: "warn,semantic_explorer=info"
```

### Worker Rust Services

| Variable | Value | Notes |
|----------|-------|-------|
| `MAX_CONCURRENT_JOBS` | `10` | Concurrent job processing limit |
| `NATS_REPLICAS` | `3` | Stream replication factor |
| `LOG_FORMAT` | `json` | Structured JSON logging |

```yaml
workerCollections:
  env:
    MAX_CONCURRENT_JOBS: "10"
    NATS_REPLICAS: "3"

workerDatasets:
  env:
    MAX_CONCURRENT_JOBS: "10"
    NATS_REPLICAS: "3"
```

### Worker Python (Visualizations)

| Variable | Value | Notes |
|----------|-------|-------|
| `MAX_CONCURRENT_JOBS` | `3` | Concurrent visualization jobs |
| `PROCESSING_TIMEOUT_SECS` | `3600` | Job timeout (1 hour) |
| `LOG_FORMAT` | `json` | Structured JSON logging |
| `HEALTH_CHECK_PORT` | `8081` | Health check HTTP server |

```yaml
workerVisualizationsPy:
  env:
    MAX_CONCURRENT_JOBS: "3"
    LOG_FORMAT: "json"
    HEALTH_CHECK_PORT: "8081"
  terminationGracePeriodSeconds: 300  # 5 minutes
```

### Inference API (Local Embedding/Reranking)

| Variable | Value | Notes |
|----------|-------|-------|
| `INFERENCE_PORT` | `8090` | HTTP server port |
| `INFERENCE_CUDA_ENABLED` | `true` | Enable GPU acceleration |
| `INFERENCE_CUDA_DEVICE_ID` | `0` | GPU device ID |
| `INFERENCE_PRELOAD_MODELS` | `BAAI/bge-small-en-v1.5,BAAI/bge-reranker-base` | Models to load at startup |
| `HF_HOME` | `/models` | HuggingFace cache directory |
| `HF_ENDPOINT` | (optional) | Artifactory/proxy URL for airgapped |

```yaml
inferenceApi:
  enabled: true
  resources:
    limits:
      nvidia.com/gpu: 1
      memory: "16Gi"
    requests:
      nvidia.com/gpu: 1
      memory: "8Gi"
  env:
    INFERENCE_CUDA_ENABLED: "true"
    INFERENCE_PRELOAD_MODELS: "BAAI/bge-small-en-v1.5,BAAI/bge-reranker-base"
    HF_HOME: "/models"
  volumes:
    models:
      enabled: true
      size: "50Gi"
```

## Health Checks and Monitoring

### API Server Probes

Health check endpoints running on port 8080:

```bash
# Liveness probe (is the process running?)
curl http://localhost:8080/health/live

# Readiness probe (is the service ready to accept traffic?)
curl http://localhost:8080/health/ready

# Kubernetes probes (in values.yaml)
api:
  livenessProbe:
    httpGet:
      path: /health/live
      port: 8080
    initialDelaySeconds: 30
    periodSeconds: 10
    failureThreshold: 3

  readinessProbe:
    httpGet:
      path: /health/ready
      port: 8080
    initialDelaySeconds: 10
    periodSeconds: 5
    failureThreshold: 3
```

### Python Visualization Worker Probes

Health check endpoints running on port 8081:

```bash
# Liveness probe (is the worker running?)
curl http://localhost:8081/health/live

# Readiness probe (is the worker ready to accept jobs?)
curl http://localhost:8081/health/ready

# Kubernetes probes (in values.yaml)
workerVisualizationsPy:
  livenessProbe:
    httpGet:
      path: /health/live
      port: 8081
    initialDelaySeconds: 30
    periodSeconds: 10
    failureThreshold: 3

  readinessProbe:
    httpGet:
      path: /health/ready
      port: 8081
    initialDelaySeconds: 15
    periodSeconds: 5
    failureThreshold: 2

  terminationGracePeriodSeconds: 300  # Wait 5 mins for in-flight jobs
```

### Graceful Shutdown

All services handle SIGTERM gracefully:
- API: Stops accepting requests, waits up to 30 seconds
- Workers (Rust): Completes in-flight jobs, waits up to 30 seconds
- Workers (Python): Completes visualization jobs, waits up to 5 minutes

```yaml
# In StatefulSet template
terminationGracePeriodSeconds: 300  # Max wait time
lifecycle:
  preStop:
    exec:
      command: ["/bin/sh", "-c", "sleep 5"]  # Load balancer drain time
```

## Observability and Metrics

### Overview

The observability stack provides comprehensive monitoring of all semantic-explorer components:

- **Prometheus** (port 9090): Metrics scraping and storage
- **Grafana** (port 3000): Dashboards and visualization
- **OTEL Collector** (port 4317, 4318): Trace/log aggregation
- **Jaeger** (port 6831): Distributed tracing backend
- **Quickwit** (port 7280-7281): Log and trace storage with search
- **AlertManager** (port 9093): Alert routing and notifications

All components are pre-configured and automatically scraped. **Deploy with `values-all-included-dev.yaml` or `values-all-included-prod.yaml` for complete observability stack.**

### Metrics Collection

**Prometheus scrape configuration** (auto-configured):
- API server: 15s interval, `/metrics` endpoint on port 8080
- worker-collections: 15s interval, `/metrics` endpoint on port 8080
- worker-datasets: 15s interval, `/metrics` endpoint on port 8080
- worker-visualizations-py: 15s interval, `/metrics` endpoint on port 9090
- Infrastructure (PostgreSQL, NATS, NATS, Qdrant): 30s interval via exporters

### Metrics by Service

#### API Server (`http_*` metrics)

```
http_requests_total{method,path,status}
  - Total HTTP requests across API endpoints
  - Labels: method (GET/POST/etc), path, status (200/404/500/etc)
  - Type: Counter

http_request_duration_seconds{method,path,status}
  - HTTP request latency distribution
  - Use histogram quantiles for p50/p95/p99 latency
  - Type: Histogram with 50ms-10s buckets

http_requests_in_flight
  - Currently processing HTTP requests (gauge)
  - Type: Gauge

sse_connections_active
  - Active Server-Sent Events connections
  - Type: Gauge
```

#### Transform Workers (`*_transform_jobs_*` metrics)

**Collection Transform Worker (worker-collections):**
```
collection_transform_jobs_total{status}
  - Total file extraction/chunk jobs processed
  - Labels: status (success/failed)
  
collection_transform_jobs_duration_seconds
  - Time spent processing extraction jobs
  
collection_transform_items_created
  - Total chunks created from files
  
collection_transform_failures_total{error_type}
  - Job failures by type (parsing/storage/validation/etc)
```

**Dataset Transform Worker (worker-datasets):**
```
dataset_transform_jobs_total{status}
  - Total embedding generation jobs
  
dataset_transform_jobs_duration_seconds
  - Time spent generating embeddings
  
dataset_transform_items_created
  - Total vectors created
  
dataset_transform_failures_total{error_type}
  - Job failures by type (embedder/qdrant/validation/etc)
```

**Visualization Transform Worker (worker-visualizations-py):**
```
visualization_jobs_total{status}
  - Total visualization (UMAP/HDBSCAN) jobs
  
visualization_job_duration_seconds
  - Time spent on clustering/layout computation
  
visualization_points_created
  - Total data points laid out
  
visualization_clusters_created
  - Total clusters identified
  
visualization_job_failures_total{error_type}
  - Job failures by type (processing/s3/validation/etc)
  
visualization_s3_upload_duration_seconds
  - Time to upload visualization results to S3
```

#### Database Metrics (`database_*`)

```
database_connection_pool_active
  - Currently active database connections
  
database_connection_pool_idle
  - Idle connections in pool
  
database_query_duration_seconds{operation,status}
  - Database query latency (SELECT/INSERT/UPDATE/DELETE)
  
database_query_total{operation,status}
  - Total database queries executed
```

#### Storage Metrics (`storage_*`)

```
storage_operation_duration_seconds{operation}
  - S3/storage operation latency (get/put/delete/list)
  
storage_operation_total{operation,status}
  - Total storage operations
```

#### Search Metrics (`search_*`)

```
search_request_total{endpoint}
  - Total search requests (vector search/BM25/etc)
  
search_request_duration_seconds{endpoint}
  - Search latency breakdown:
    - search_embedder_duration_seconds (embedding generation)
    - search_qdrant_query_duration_seconds (vector search query)

search_request_failures_total
  - Failed search requests
```

#### NATS Queue Metrics (`nats_*`)

```
nats_stream_messages{stream}
  - Message count in each JetStream stream
  - Streams: COLLECTION_TRANSFORMS, DATASET_TRANSFORMS, 
    VISUALIZATION_TRANSFORMS, DLQ_TRANSFORMS, TRANSFORM_STATUS
  
nats_consumer_pending{stream,consumer}
  - Pending (unacked) messages per consumer
  - High values indicate backlog/slow processing
  
nats_message_latency_seconds{direction}
  - Message publish/subscribe latency
  - Directions: publish, subscribe
```

#### Worker Health Metrics

```
worker_job_retries_total{worker}
  - Total job retries due to transient failures
  
worker_active_jobs{worker}
  - Currently processing jobs (gauge)
  
worker_ready
  - Worker readiness status (1=ready, 0=not ready)
  - Used by health checks
```

### Grafana Dashboards

Grafana is pre-configured with datasources for observability:

**Datasource UIDs**
- `prometheus` — primary metrics store for services and infrastructure panels
- `quickwit-logs` — Loki-compatible endpoint for log rate stats and streams
- `quickwit-traces` — Jaeger-compatible endpoint for dependency and trace panels
- `postgres-audit` — Postgres datasource for querying the audit_events table

> **Note:** Pre-built dashboards are not currently included. You can create custom dashboards using the available datasources and metrics exposed by the services.

**Available Metrics:**
The following OpenTelemetry metrics are exported via Prometheus:
- `database_query_*` — Database query counts and latencies
- `storage_operations_*` — S3/MinIO storage operation metrics
- `worker_jobs_*` — Worker job processing counts and durations
- `collection_transform_*` — Collection transform pipeline metrics
- `dataset_transform_*` — Dataset embedding pipeline metrics
- `visualization_transform_*` — Visualization generation metrics
- `nats_stream_*` / `nats_consumer_*` — NATS JetStream queue metrics
- `search_request_*` — Search request latencies and counts
- `inference_embed_*` / `inference_rerank_*` / `inference_llm_*` — Inference API metrics
- `sse_*` — Server-Sent Events connection metrics
- `{prefix}_http_requests_*` — HTTP request metrics from actix-web-prom (prefix varies by service)

**Access Grafana:**

Docker Compose:
```bash
# After `docker-compose up -d`, visit:
http://localhost:3000
# Default: admin / admin
# (change password immediately!)
```

Kubernetes:
```bash
# Port forward to Grafana
kubectl port-forward -n semantic-explorer svc/semantic-explorer-grafana 3000:3000

# Visit http://localhost:3000
```

### Alert Rules

Alert rules are configured in `deployment/alert_rules.yml` and cover:

**API/HTTP Alerts:**
- High error rate (>1% HTTP 5xx)
- High latency (p99 > 1 second)
- Slow database queries (>500ms)

**Worker Alerts:**
- High job failure rate (>5% failures)
- Transform-specific failure rates (collections/datasets/visualizations)
- Job processing timeout (jobs stuck for >1 hour)

**Infrastructure Alerts:**
- Database connection pool exhaustion
- NATS queue backlog (>1000 pending messages)
- Search latency high (>500ms)
- SSE connection errors

**View Alerts:**

Docker Compose:
```bash
# AlertManager UI (if enabled)
http://localhost:9093

# Prometheus alerts
http://localhost:9090/alerts
```

Kubernetes:
```bash
# Prometheus alerts
kubectl port-forward -n semantic-explorer svc/prometheus 9090:9090
http://localhost:9090/alerts
```

### Structured Logging

All services output structured JSON logs for log aggregation:

```json
{
  "timestamp": "2024-01-11T12:00:00Z",
  "level": "INFO",
  "message": "Processing visualization job",
  "service": "worker-visualizations-py",
  "version": "1.0.0",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "job_id": "viz-123",
  "duration_ms": 2500
}
```

**Configure logging:**

```yaml
api:
  env:
    LOG_FORMAT: "json"
    RUST_LOG: "info,semantic_explorer=debug"

workerCollections:
  env:
    LOG_FORMAT: "json"
    RUST_LOG: "info,semantic_explorer_core=debug"

workerDatasets:
  env:
    LOG_FORMAT: "json"
    RUST_LOG: "info,semantic_explorer_core=debug"

workerVisualizationsPy:
  env:
    LOG_FORMAT: "json"
    LOG_LEVEL: "INFO"
```

### Distributed Tracing

OpenTelemetry traces are exported to Jaeger and Quickwit:

```yaml
observability:
  otelCollector:
    enabled: true
    env:
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://localhost:4317"
```

Each request includes:
- `traceparent` header (W3C Trace Context)
- Span attributes for database, storage, API calls
- Trace context propagation across services

**Access Traces:**

Docker Compose:
```bash
# Jaeger UI (if enabled)
http://localhost:16686

# Quickwit UI (if enabled)
http://localhost:7280
```

Kubernetes:
```bash
# Quickwit search
kubectl port-forward -n semantic-explorer svc/quickwit 7280:7280
http://localhost:7280
```

### Deployment Options

Choose the appropriate values file for your deployment:

**All-Included (Complete observability stack):**
- `values-all-included-dev.yaml`: Local development (minimal resources)
  - PostgreSQL 10Gi, NATS 3 nodes, NATS, Qdrant, MinIO, OTEL, Grafana, Quickwit
  - 1 replica per service, 500m CPU, 512Mi memory per pod
  - Use for: Local dev, testing, CI/CD pipelines

- `values-all-included-prod.yaml`: Production with all services
  - PostgreSQL 100Gi HA (3 replicas), NATS JetStream, NATS with JetStream
  - 4 replicas per service, generous resources (2-4 CPU, 2-8Gi memory)
  - Pod Disruption Budgets, anti-affinity, autoscaling enabled
  - Use for: Self-hosted production deployments

**External Infrastructure (Managed services):**
- `values-external-infra-dev.yaml`: Development with external services
  - Only API + workers + Dex deployed
  - Environment variables for: PostgreSQL, NATS, NATS, Qdrant, S3, OTEL endpoint
  - Use for: Cloud dev environments (GCP CloudSQL, AWS RDS, etc.)

- `values-external-infra-prod.yaml`: Production with external services
  - 4 replicas, HA configurations, PDBs
  - Extensive documentation for AWS/GCP/Azure equivalents
  - Use for: Cloud production (managed PostgreSQL, NATS, S3, etc.)

**Install with specific values:**

```bash
# Deploy all-included dev
helm install semantic-explorer ./helm/semantic-explorer \
  -f helm/semantic-explorer/examples/values-all-included-dev.yaml

# Deploy external infra prod
helm install semantic-explorer ./helm/semantic-explorer \
  -f helm/semantic-explorer/examples/values-external-infra-prod.yaml \
  --set externalInfra.databaseUrl="postgresql://user:pass@prod-db.com/dbname" \
  --set externalInfra.s3Bucket="my-prod-bucket" \
  --set externalInfra.natsUrl="nats://nats-prod.example.com:4222"
```

### Python Worker Metrics Specifics

The `worker-visualizations-py` service exports Prometheus metrics on **port 9090**:

```bash
# Scrape metrics from Python worker
curl http://worker-visualizations-py:9090/metrics

# Includes all visualization_* metrics plus:
# - OpenTelemetry instrumentation for requests library
# - Python runtime metrics (gc, memory, thread count)
# - Custom initialization timings (S3/LLM setup duration)
```

**Python worker OTEL configuration:**

```python
# Automatically initialized in src/observability.py
# Sends traces to: http://localhost:6831 (Jaeger UDP)
# Sends metrics to: Prometheus scrape on :9090
# Sends logs to: stdout (JSON format)
```

### Performance Tuning

**For high-throughput scenarios:**

1. Increase Prometheus scrape interval (reduces cardinality):
```yaml
prometheus:
  scrapeInterval: "30s"  # Reduce from default 15s
```

2. Configure metric retention:
```yaml
prometheus:
  retention: "15d"  # Keep metrics for 15 days
```

3. Adjust worker concurrency (see Worker Concurrency Tuning section above)

4. Monitor metric cardinality:
```bash
# Query Prometheus for metric cardinality
curl 'http://prometheus:9090/api/v1/query?query=count({__name__=~".+"})'
```

### Troubleshooting Observability

**Check if metrics are being scraped:**
```bash
# Prometheus targets page
http://localhost:9090/targets

# All targets should show "UP" with recent scrape time
```

**Verify dashboards are provisioned:**
```bash
# Docker Compose
ls -la deployment/compose/grafana/provisioning/dashboards/

# Kubernetes
kubectl get configmap -n semantic-explorer | grep dashboard
```

**Check Python worker metrics:**
```bash
# Port forward to Python worker metrics
kubectl port-forward -n semantic-explorer deployment/worker-visualizations-py 9090:9090

# Scrape metrics
curl http://localhost:9090/metrics | grep visualization_
```

**Validate alert rules:**
```bash
# Check Prometheus alert evaluation
http://prometheus:9090/alerts

# Look for your configured alerts; they should show pending/firing status
```

**Check OTEL connectivity:**
```bash
# View OTEL Collector logs
kubectl logs -n semantic-explorer deployment/otel-collector | grep -i error

# Check if traces are reaching Quickwit
kubectl port-forward -n semantic-explorer svc/quickwit 7280:7280
# Visit http://localhost:7280 and search for traces
```

## NATS Stream Configuration

### Stream Replication

All JetStream streams are configured with 3-replica replication for production:

```yaml
# Configured via NATS_REPLICAS environment variable
api:
  env:
    NATS_REPLICAS: "3"
```

**Streams:**
- `COLLECTION_TRANSFORMS` - File extraction jobs (WorkQueue retention, 7 days)
- `DATASET_TRANSFORMS` - Embedding generation jobs (WorkQueue retention, 7 days)
- `VISUALIZATION_TRANSFORMS` - Visualization jobs (WorkQueue retention, 7 days)
- `DLQ_TRANSFORMS` - Failed jobs for investigation (Limits retention, 30 days)
- `TRANSFORM_STATUS` - Real-time status updates (Limits retention, 1 hour, 100K messages)

**Consumer Configuration:**
- `max_ack_pending`: 100 (backpressure limit)
- `max_deliver`: 5 (retry attempts)
- `ack_wait`: 10-30 minutes (processing time allowance)
- Exponential backoff: 30s, 60s, 120s, 300s

**Production Setup:**

```yaml
nats:
  enabled: false
  external:
    enabled: true
    url: "nats://nats.example.com:4222"
  # NATS cluster must have:
  # - At least 3 replicas for HA
  # - Persistent storage for JetStream
  # - Adequate disk space (10GB+ recommended)
```

## Worker Concurrency Tuning

### MAX_CONCURRENT_JOBS Configuration

Controls maximum parallel job processing per worker replica:

**File Extraction (worker-collections):**
```yaml
workerCollections:
  env:
    MAX_CONCURRENT_JOBS: "10"  # Default
  # Adjust based on:
  # - CPU cores available (PDF extraction is CPU-intensive)
  # - Memory available (large files need memory)
  # - Average file size
```

**Embedding Generation (worker-datasets):**
```yaml
workerDatasets:
  env:
    MAX_CONCURRENT_JOBS: "10"  # Default
  # Adjust based on:
  # - Embedder API rate limits
  # - Available memory (batch size × vector dimensions)
  # - Qdrant connection pool capacity
  # - Network bandwidth
```

**Visualization (worker-visualizations-py):**
```yaml
workerVisualizationsPy:
  env:
    MAX_CONCURRENT_JOBS: "3"   # Lower for memory/CPU intensive
  # Adjust based on:
  # - Dataset size (vectors to process)
  # - Available memory (UMAP/HDBSCAN working memory)
  # - GPU availability and VRAM
  # - Processing timeout requirements
```

**Backpressure Configuration:**

NATS automatically applies backpressure via `max_ack_pending`:

```yaml
# In worker configuration
# Consumer won't deliver more than max_ack_pending messages
# until previous messages are acked

# Example: max_ack_pending=100, max_concurrent_jobs=10
# NATS will buffer up to 10 messages, backpressure prevents overflow
```

## Production Best Practices

### Pod Disruption Budgets

Ensure minimum availability during upgrades/maintenance:

```yaml
api:
  podDisruptionBudget:
    enabled: true
    minAvailable: 1  # Keep at least 1 pod running

workerCollections:
  podDisruptionBudget:
    enabled: true
    minAvailable: 1

workerDatasets:
  podDisruptionBudget:
    enabled: true
    minAvailable: 1
```

### Pod Anti-Affinity

Spread pods across different nodes for resilience:

```yaml
api:
  affinity:
    podAntiAffinity:
      preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchExpressions:
            - key: app.kubernetes.io/name
              operator: In
              values:
              - api
          topologyKey: kubernetes.io/hostname
```

### Resource Requests and Limits

Set appropriate resource requests for scheduling:

```yaml
api:
  resources:
    requests:
      cpu: 2000m
      memory: 2Gi
    limits:
      cpu: 4000m
      memory: 8Gi
```

### Monitoring and Alerting

Configure alerts for:
- API server down (liveness probe failures)
- High error rates (HTTP 5xx > 1%)
- High latencies (p99 request time > 1s)
- Worker job failures (failed status > 5%)
- Database connection pool exhaustion
- NATS stream message backlog
- Disk space (especially for NATS JetStream)

### Backup and Recovery

- **Database**: Regular automated backups with point-in-time recovery
- **NATS JetStream**: Persistent storage with replicated disks
- **S3 Storage**: Versioning enabled, lifecycle policies
- **Encryption Keys**: Secure backup separate from primary storage

### Logging and Audit

- Central log aggregation (ELK, Loki, CloudWatch)
- Audit events logged for security tracking
- Encryption key rotation audited
- API authentication and errors tracked

### TLS and Network Security

```yaml
global:
  securityContext:
    runAsNonRoot: true
    runAsUser: 10001
    fsGroup: 10001
    fsGroupChangePolicy: "OnRootMismatch"
    seccompProfile:
      type: RuntimeDefault

# Container security context
podSecurityContext:
  allowPrivilegeEscalation: false
  capabilities:
    drop:
    - ALL
  readOnlyRootFilesystem: true  # Most services can have read-only root
```

### Database Connection Pooling

Optimize for multi-replica deployments:

```yaml
api:
  env:
    DB_MAX_CONNECTIONS: "20"      # Per API pod (3 pods = 60 total)
    DB_MIN_CONNECTIONS: "2"       # Minimum connections
    DB_ACQUIRE_TIMEOUT_SECS: "30" # Timeout waiting for connection
    DB_IDLE_TIMEOUT_SECS: "300"   # Close idle connections after 5 mins
    DB_MAX_LIFETIME_SECS: "1800"  # Recycle connections every 30 mins
```

For 3 API replicas with 20 max each, total DB connections = 60.
Configure PostgreSQL to allow at least this many:

```sql
-- In PostgreSQL
ALTER SYSTEM SET max_connections = 200;  -- Leave headroom
SELECT pg_reload_conf();
```

## Helm Chart Reference

Full values.yaml documentation:
- See `/deployment/helm/semantic-explorer/values.yaml` for all configuration options
- Each section has comments explaining available settings
- Defaults are production-safe for small to medium deployments

## Support and Troubleshooting

### Verify Encryption Setup

```bash
# Check if encryption is properly configured
kubectl logs -l app.kubernetes.io/name=api -n semantic-explorer | grep -i encryption

# Should see log entries confirming encryption service initialized
# If ENCRYPTION_MASTER_KEY not set, you'll see a warning
```

### Monitor Health Checks

```bash
# Check API readiness
kubectl get pods -n semantic-explorer -w

# All pods should show Ready 1/1
# If not ready, check probes with:
kubectl describe pod <pod-name> -n semantic-explorer
```

### Check Worker Logs

```bash
# File extraction worker logs
kubectl logs -l app.kubernetes.io/name=worker-collections -n semantic-explorer

# Embedding worker logs
kubectl logs -l app.kubernetes.io/name=worker-datasets -n semantic-explorer

# Visualization worker logs (with health check)
kubectl logs -l app.kubernetes.io/name=worker-visualizations-py -n semantic-explorer
```

### Verify NATS Stream Replication

```bash
# Port forward to NATS
kubectl port-forward -n semantic-explorer svc/semantic-explorer-nats 4222:4222 &

# Check stream info
nats stream info COLLECTION_TRANSFORMS
# Should show: Cluster Nodes: [node-0, node-1, node-2] (3 replicas)
```

## License

See LICENSE file in repository root.
