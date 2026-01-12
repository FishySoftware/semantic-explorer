# Semantic Explorer Deployment Guide

Comprehensive guide for deploying Semantic Explorer in production using Kubernetes and Helm.

## Table of Contents

- [Quick Start](#quick-start)
- [Security Configuration](#security-configuration)
- [Helm Chart Configuration](#helm-chart-configuration)
- [Environment Variables](#environment-variables)
- [Health Checks and Monitoring](#health-checks-and-monitoring)
- [Observability and Metrics](#observability-and-metrics)
- [NATS Stream Configuration](#nats-stream-configuration)
- [Worker Concurrency Tuning](#worker-concurrency-tuning)
- [Production Best Practices](#production-best-practices)

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
curl http://localhost:8081/health

# Readiness probe (is the worker ready to accept jobs?)
curl http://localhost:8081/ready

# Kubernetes probes (in values.yaml)
workerVisualizationsPy:
  livenessProbe:
    httpGet:
      path: /health
      port: 8081
    initialDelaySeconds: 30
    periodSeconds: 10
    failureThreshold: 3

  readinessProbe:
    httpGet:
      path: /ready
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

### Metrics Endpoints

All services export Prometheus metrics:

```bash
# API server metrics
curl http://localhost:8080/metrics

# Available metrics:
# - http_requests_total (by method, path, status)
# - http_request_duration_seconds (request latency)
# - database_query_duration_seconds (query latency)
# - database_query_total (by operation, status)
# - storage_operation_duration_seconds (S3 operations)
# - worker_jobs_total (by worker, status)
# - worker_job_duration_seconds
# - nats_stream_messages (messages in stream)
# - nats_consumer_pending (pending messages)
# - search_request_duration_seconds
# - sse_connections_active
```

### Structured Logging

All services output structured JSON logs compatible with log aggregation:

```json
{
  "timestamp": "2024-01-11T12:00:00Z",
  "level": "INFO",
  "message": "API server started",
  "service": "api",
  "version": "1.0.0",
  "request_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

Configure in Helm:
```yaml
api:
  env:
    LOG_FORMAT: "json"
    RUST_LOG: "info,semantic_explorer=debug"

workerVisualizationsPy:
  env:
    LOG_FORMAT: "json"
    LOG_LEVEL: "INFO"
```

### Distributed Tracing

OpenTelemetry traces exported to Quickwit (logs/traces) or Jaeger:

```yaml
observability:
  otelCollector:
    enabled: true
    config:
      exporters:
        otlp/quickwit:
          endpoint: "quickwit:7281"
```

Each request includes:
- `x-request-id` header with unique request ID
- Trace context propagation across services
- Span attributes for database, storage, API calls

### Grafana Dashboards

Pre-configured dashboards for:
- **Application Metrics**: Request rates, latencies, errors
- **Worker Metrics**: Job processing, concurrency, errors
- **Database/NATS**: Connection pools, stream metrics
- **Infrastructure**: Node resources, disk usage
- **Health Check**: Liveness and readiness probe status

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
