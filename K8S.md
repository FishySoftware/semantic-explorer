# Kubernetes Helm Chart - Tracking Document

This document tracks the alignment between the Helm chart and `docker-compose.dev.yaml` deployment.

## Overview

- **Helm Chart Location**: `deployment/helm/semantic-explorer/`
- **Docker Compose Reference**: `deployment/compose/compose.dev.yaml`
- **Chart Version**: 1.0.0
- **App Version**: 1.0.0

---

## Findings Summary

### Application Services Status

| # | Service | docker-compose | Helm Template | Status |
|---|---------|----------------|---------------|--------|
| 1 | semantic-explorer (API) | ✅ | ✅ api-statefulset.yaml | ✅ Complete |
| 2 | worker-collections | ✅ | ✅ worker-collections-statefulset.yaml | ✅ Complete |
| 3 | worker-datasets | ✅ | ✅ worker-datasets-statefulset.yaml | ✅ Complete |
| 4 | worker-visualizations-py | ✅ | ✅ worker-visualizations-py-statefulset.yaml | ✅ Complete |
| 5 | embedding-inference-api | ✅ | ✅ embedding-inference-api-statefulset.yaml | ✅ Complete |
| 6 | llm-inference-api | ✅ | ✅ llm-inference-api-statefulset.yaml | ✅ Complete |

### Infrastructure Services Status

| # | Service | docker-compose | Helm Chart | Status |
|---|---------|----------------|------------|--------|
| 7 | PostgreSQL | ✅ postgres | ✅ postgresql.yaml | ✅ Complete |
| 8 | NATS (3-node cluster) | ✅ nats-1,2,3 | ✅ nats subchart | ✅ Complete |
| 9 | Qdrant | ✅ qdrant | ✅ qdrant subchart | ✅ Complete (GPU documented) |
| 10 | MinIO (4-node distributed) | ✅ minio-1,2,3,4 | ✅ minio subchart | ✅ Complete |
| 11 | Dex (OIDC) | ✅ dex | ✅ dex-*.yaml | ✅ Complete (NEW) |
| 12 | Prometheus | ✅ prometheus | ✅ prometheus subchart | ✅ Complete |
| 13 | Grafana | ✅ grafana | ✅ grafana subchart | ✅ Complete |
| 14 | OpenTelemetry Collector | ✅ otel-collector | ✅ otel-collector.yaml | ✅ Complete |
| 15 | Quickwit | ✅ quickwit | ✅ quickwit-*.yaml | ✅ Complete |
| 16 | postgres-exporter | ✅ | ✅ postgresql.yaml metrics | ✅ Complete |
| 17 | minio-init | ✅ | ✅ minio-init-job.yaml | ✅ Complete (NEW) |

---

## Detailed Findings & Fixes

### Issue #1: Missing Dex Template
**Status**: ✅ Complete  
**Priority**: High  
**Description**: Dex configuration exists in `values.yaml` but there's no corresponding template to deploy it.

**Created Files**:
- [x] `templates/dex-deployment.yaml`
- [x] `templates/dex-service.yaml`
- [x] `templates/dex-configmap.yaml`
- [x] `templates/dex-serviceaccount.yaml` (includes RBAC for kubernetes storage)
- [x] `templates/dex-ingress.yaml` (optional)

**Updated Files**:
- [x] `templates/_helpers.tpl` - Added Dex helper functions
- [x] `values.yaml` - Added missing fields (env, envFrom, nodeSelector, tolerations, affinity, serviceAccount)

**docker-compose reference**:
```yaml
dex:
  image: dexidp/dex:latest
  command: dex serve /etc/dex/config.yaml
  environment:
    GITHUB_CLIENT_ID: ${GITHUB_CLIENT_ID}
    GITHUB_CLIENT_SECRET: ${GITHUB_CLIENT_SECRET}
  volumes:
    - ./dex.yaml:/etc/dex/config.yaml:ro
    - dex_data:/var/dex:rw
  ports:
    - "5556:5556"
    - "5557:5557"
    - "5558:5558"
```

---

### Issue #2: Missing MinIO Init Job
**Status**: ✅ Complete  
**Priority**: Medium  
**Description**: docker-compose has `minio-init` service that creates buckets. Helm chart needs equivalent.

**Resolution**:
- MinIO subchart already supports bucket provisioning via `buckets` configuration in values.yaml
- Created optional `templates/minio-init-job.yaml` as fallback for custom configurations
- Added `minio.initJob.enabled` flag to values.yaml (disabled by default, subchart handles it)

**Created Files**:
- [x] `templates/minio-init-job.yaml` - Helm hook job for custom bucket init

**Existing Configuration** (already working):
```yaml
minio:
  buckets:
    - name: semantic-explorer-files
      policy: none
      purge: false
    - name: quickwit
      policy: none
      purge: false
```

---

### Issue #3: Environment Variable Alignment
**Status**: ✅ Complete  
**Priority**: Medium  
**Description**: Verify all environment variables from docker-compose are represented in Helm values.

**Changes Made**:
- [x] Added `EMBEDDING_INFERENCE_API_URL` to API statefulset (auto-generated from service)
- [x] Added `EMBEDDING_INFERENCE_API_TIMEOUT_SECS` to API statefulset
- [x] Added `LLM_INFERENCE_API_URL` to API statefulset (auto-generated from service)
- [x] Added `LLM_INFERENCE_API_TIMEOUT_SECS` to API statefulset
- [x] Added `EMBEDDING_INFERENCE_API_URL` to worker-datasets statefulset
- [x] Added `EMBEDDING_INFERENCE_API_TIMEOUT_SECS` to worker-datasets statefulset
- [x] Added helper functions for inference API URLs in `_helpers.tpl`

**Note**: Environment variables like `DATABASE_URL`, `NATS_URL`, `QDRANT_URL`, `AWS_*`, `S3_*`, `OIDC_*`, `OTEL_*`, and `ENCRYPTION_MASTER_KEY` are already properly templated from secrets and helpers.

---

### Issue #4: Service Dependencies / Init Containers
**Status**: ✅ Complete  
**Priority**: Medium  
**Description**: docker-compose uses `depends_on` with health checks. Helm chart should have init containers or proper probes.

**Changes Made**:
- [x] Added `wait-for-embedding-api` init container to API statefulset
- [x] Added `wait-for-llm-api` init container to API statefulset  
- [x] Added `wait-for-embedding-api` init container to worker-datasets statefulset

**Existing Init Containers** (already present):
- API: `wait-for-postgresql`, `wait-for-nats`, `wait-for-qdrant`
- worker-collections: `wait-for-nats`, `wait-for-minio`
- worker-datasets: `wait-for-nats`, `wait-for-qdrant`
- worker-visualizations-py: `wait-for-nats`, `wait-for-qdrant`, `wait-for-minio`

---

### Issue #5: Volume Mounts Alignment
**Status**: ✅ Complete  
**Priority**: Low  
**Description**: Verify persistent volume claims match docker-compose volumes.

**Volume Mapping Analysis**:

| docker-compose volume | Helm Chart Implementation | Notes |
|----------------------|---------------------------|-------|
| `qdrant_storage` | qdrant subchart persistence | ✅ Handled by subchart |
| `minio_data_1-4` | minio subchart persistence | ✅ Handled by subchart (distributed mode) |
| `postgres_data` | postgresql.primary.persistence | ✅ 100Gi default |
| `dex_data` | emptyDir (sqlite3 storage) | ✅ No persistence needed (k8s storage backend) |
| `nats_data_1-3` | nats subchart fileStorage | ✅ Handled by subchart |
| `quickwit_data` | quickwit persistence | ✅ 50Gi default |
| `prometheus_data` | prometheus subchart | ✅ Handled by subchart |
| `grafana_data` | grafana subchart | ✅ Handled by subchart |
| `inference_models` | embeddingInferenceApi persistence | ✅ 50Gi PVC |
| `llm_models` | llmInferenceApi persistence | ✅ 100Gi PVC |
| `llm_hf_cache` | emptyDir (1Gi) | ✅ Container-local cache OK |
| `tensorrt_cache` | embeddingInferenceApi.tensorrtCache | ✅ Optional 10Gi PVC |
| `api_tmp` | emptyDir (tmp) | ✅ Ephemeral is appropriate |

**Fix Applied**:
- [x] Added `ENCRYPTION_MASTER_KEY` env var to worker-visualizations-py (was missing)

---

### Issue #6: GPU Configuration
**Status**: ✅ Complete  
**Priority**: Medium  
**Description**: Verify GPU configuration for qdrant, embedding-inference-api, llm-inference-api.

**Analysis**:

| Component | docker-compose GPU Config | Helm Chart Config | Status |
|-----------|--------------------------|-------------------|--------|
| Qdrant | nvidia driver, all GPUs, `gpu-nvidia-latest` image | `gpu.enabled`, `gpu.nvidia`, image configurable | ✅ Documented |
| Embedding Inference API | nvidia driver, 1 GPU | `nvidia.com/gpu: 1` in resources, GPU nodeSelector + tolerations | ✅ Complete |
| LLM Inference API | nvidia driver, 1 GPU | `nvidia.com/gpu: 1` in resources, GPU nodeSelector + tolerations | ✅ Complete |

**Helm Chart GPU Features**:

1. **Embedding/LLM Inference APIs** (production-ready):
   - GPU resources in requests/limits
   - GPU node selector: `nvidia.com/gpu: "true"`
   - GPU tolerations for tainted nodes
   - Security context allows SYS_ADMIN for GPU access

2. **Qdrant** (subchart - configurable):
   - Updated values.yaml with GPU documentation
   - Instructions to use `gpu-nvidia-latest` image tag
   - GPU resource configuration documented as comments

**Updated Files**:
- [x] Updated `values.yaml` with better Qdrant GPU documentation

---

## Progress Log

| Date | Issue | Action | Result |
|------|-------|--------|--------|
| 2026-01-18 | Initial | Created tracking document | ✅ |
| 2026-01-18 | #1 | Created Dex templates (deployment, service, configmap, serviceaccount, ingress) | ✅ |
| 2026-01-18 | #1 | Added Dex helper functions to _helpers.tpl | ✅ |
| 2026-01-18 | #1 | Added missing Dex config fields to values.yaml | ✅ |
| 2026-01-18 | #2 | Created minio-init-job.yaml for custom bucket provisioning | ✅ |
| 2026-01-18 | #2 | Added minio.initJob.enabled flag to values.yaml | ✅ |
| 2026-01-18 | #3 | Added inference API URL helpers to _helpers.tpl | ✅ |
| 2026-01-18 | #3 | Added EMBEDDING_INFERENCE_API_URL env to API statefulset | ✅ |
| 2026-01-18 | #3 | Added LLM_INFERENCE_API_URL env to API statefulset | ✅ |
| 2026-01-18 | #3 | Added EMBEDDING_INFERENCE_API_URL env to worker-datasets | ✅ |
| 2026-01-18 | #4 | Added wait-for-embedding-api init container to API | ✅ |
| 2026-01-18 | #4 | Added wait-for-llm-api init container to API | ✅ |
| 2026-01-18 | #4 | Added wait-for-embedding-api init container to worker-datasets | ✅ |
| 2026-01-18 | #5 | Added ENCRYPTION_MASTER_KEY env to worker-visualizations-py | ✅ |
| 2026-01-18 | #6 | Updated Qdrant GPU documentation in values.yaml | ✅ |
| 2026-01-18 | #7 | Security: Changed GPU pods to run as non-root (user 1000) | ✅ |
| 2026-01-18 | #7 | Security: Removed SYS_ADMIN capability from GPU pods | ✅ |
| 2026-01-18 | #7 | Security: Changed Dex storage from kubernetes (needs CRDs) to sqlite3 | ✅ |
| 2026-01-18 | #7 | Security: Removed CRD creation RBAC from Dex serviceaccount | ✅ |
| 2026-01-18 | #7 | Security: Fixed Qdrant podSecurityContext to runAsNonRoot: true | ✅ |
| 2026-01-18 | #7 | Security: Changed Grafana sidecar to namespace-only search | ✅ |
| 2026-01-18 | #7 | Security: Disabled Prometheus subchart RBAC (uses our Role/RoleBinding) | ✅ |

---

## Issue #7: Security Hardening (Namespace-Only RBAC, Non-Root)
**Status**: ✅ Complete  
**Priority**: High  
**Description**: Enforce security constraints - only Role/RoleBinding allowed (no ClusterRole), no root user, no operators.

### Changes Made

#### 1. GPU Inference APIs - Non-Root Execution
**Files**: `templates/embedding-inference-api-statefulset.yaml`, `templates/llm-inference-api-statefulset.yaml`
- Changed `runAsNonRoot: false` → `runAsNonRoot: true`
- Changed `runAsUser: 0` → `runAsUser: 1000` (configurable via values)
- Removed `SYS_ADMIN` capability
- Changed `allowPrivilegeEscalation: true` → `allowPrivilegeEscalation: false`
- Added `seccompProfile: RuntimeDefault`

#### 2. Dex Storage Backend
**Files**: `templates/dex-serviceaccount.yaml`, `values.yaml`
- Changed storage type from `kubernetes` (requires CRDs - cluster-wide) to `sqlite3`
- Removed RBAC rules for CRD creation
- For production HA, use external database (postgres/mysql) instead

#### 3. Qdrant Security Context
**File**: `values.yaml`
- Changed `runAsNonRoot: false` → `runAsNonRoot: true`
- Added `runAsGroup: 1000`, `fsGroupChangePolicy`, `seccompProfile`

#### 4. Grafana Sidecar Namespace Scope
**File**: `values.yaml`
- Changed `sidecar.dashboards.searchNamespace: ALL` → `null`
- This limits sidecar to release namespace only (no ClusterRole needed)

#### 5. Prometheus RBAC
**File**: `values.yaml`
- Added `rbac.create: false` to disable subchart ClusterRole creation
- Our custom `templates/prometheus-rbac.yaml` uses Role/RoleBinding (namespace-scoped)

### Validation Results
```bash
# No ClusterRole or ClusterRoleBinding resources
helm template test . | grep -E "^kind: (ClusterRole|ClusterRoleBinding)"
# Result: No matches

# All pods run as non-root
helm template test . | grep -E "runAsUser: 0|runAsNonRoot: false"
# Result: No matches
```

### Notes
- GPU container images must be built to run as non-root user (uid 1000)
- If kubernetes storage for Dex is required, CRDs must be pre-installed by cluster admin
- Prometheus uses static scrape configs with `kubernetes_sd_configs` limited to `own_namespace`

---

## Summary

All identified gaps between `docker-compose.dev.yaml` and the Helm chart have been addressed:

1. **Dex OIDC Provider**: Complete template set created
2. **MinIO Bucket Init**: Subchart handles it; optional custom job available
3. **Environment Variables**: Added missing inference API URLs and timeouts
4. **Init Containers**: Added dependency waits for inference APIs
5. **Volume Mounts**: All properly configured
6. **GPU Configuration**: Fully documented and configured
7. **Security Hardening**: Namespace-only RBAC (Role/RoleBinding), non-root execution, no operators
