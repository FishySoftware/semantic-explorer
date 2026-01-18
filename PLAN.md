# Implementation Plan: LLM Inference API + Rename to Embedding Inference API

## Overview

This document tracks the implementation of two major tasks:
1. **Create `llm-inference-api`** - New crate for LLM text generation using mistral.rs
2. **Rename `inference-api` → `embedding-inference-api`** - Clean break rename with full environment variable updates

## Progress Tracking

- ✅ = Completed
- 🚧 = In Progress
- ⏳ = Pending

---

## TASK 1: Create llm-inference-api Crate

### Core Implementation

- [x] ✅ Step 1: Add `llm-inference-api` to workspace members in Cargo.toml
- [x] ✅ Step 2: Create `crates/llm-inference-api/Cargo.toml` with dependencies
- [x] ✅ Step 3: Create `crates/llm-inference-api/src/config.rs` (configuration layer)
- [x] ✅ Step 4: Create `crates/llm-inference-api/src/errors.rs` (error handling)
- [x] ✅ Step 5: Create `crates/llm-inference-api/src/llm.rs` (model cache & generation)
- [x] ✅ Step 6: Create `crates/llm-inference-api/src/models.rs` (model discovery)
- [x] ✅ Step 7: Create `crates/llm-inference-api/src/observability.rs` (telemetry)

### API Endpoints

- [x] ✅ Step 8: Create `crates/llm-inference-api/src/api/mod.rs`
- [x] ✅ Step 9: Create `crates/llm-inference-api/src/api/generation.rs` (basic generation endpoint)
- [x] ✅ Step 10: Create `crates/llm-inference-api/src/api/streaming.rs` (streaming endpoint)
- [x] ✅ Step 11: Create `crates/llm-inference-api/src/api/chat.rs` (chat completion endpoint)
- [x] ✅ Step 12: Create `crates/llm-inference-api/src/api/health.rs` (health checks)

### Server & Configuration

- [x] ✅ Step 13: Create `crates/llm-inference-api/src/main.rs` (server setup)
- [ ] 🚧 Step 14: Create `crates/llm-inference-api/.env.example`
- [x] ✅ Step 15: Create `crates/llm-inference-api/README.md`

### Docker & Build

- [x] ✅ Step 16: Create `crates/llm-inference-api/Dockerfile`

### Kubernetes/Helm Templates

- [x] ✅ Step 17: Create `deployment/helm/semantic-explorer/templates/llm-inference-api-service.yaml`
- [x] ✅ Step 18: Create `deployment/helm/semantic-explorer/templates/llm-inference-api-statefulset.yaml`
- [x] ✅ Step 19: Create `deployment/helm/semantic-explorer/templates/llm-inference-api-serviceaccount.yaml`
- [x] ✅ Step 20: Update `deployment/helm/semantic-explorer/templates/_helpers.tpl` with llmInferenceApi functions
- [x] ✅ Step 21: Add llmInferenceApi section to `deployment/helm/semantic-explorer/values.yaml`
- [x] ✅ Step 22: Update Helm HPA, NetworkPolicy, PDB, ServiceMonitor for llm-inference-api

### Docker Compose

- [x] ✅ Step 23: Add llm-inference-api service to `deployment/compose/compose.yaml`
- [x] ✅ Step 24: Add llm-inference-api service to `deployment/compose/compose.dev.yaml`

### CI/CD Pipeline

- [x] ✅ Step 25: Add llm-inference-api build job to `.github/workflows/ci.yaml`
- [x] ✅ Step 26: Add Dockerfile check to `.github/workflows/pr.yaml`

### Testing

- [x] ✅ Step 27: Test llm-inference-api locally (cargo build, run, API tests)

---

## TASK 2: Rename inference-api → embedding-inference-api

### Git Operations

- [x] ✅ Step 28: Rename directory with `git mv crates/inference-api crates/embedding-inference-api`

### Rust Code Updates

- [x] ✅ Step 29: Update `crates/embedding-inference-api/Cargo.toml` (package name, binary name)
- [x] ✅ Step 30: Update root `Cargo.toml` workspace member
- [x] ✅ Step 31: Update `crates/core/src/config.rs` (environment variable names)
- [x] ✅ Step 32: Update `crates/core/src/embedder.rs` (environment variable usage)
- [x] ✅ Step 33: Update `crates/api/src/api/embedders.rs` (comments/error messages)
- [x] ✅ Step 34: Update `crates/api/src/api/embedding_inference.rs` (HTTP calls)
- [x] ✅ Step 35: Update `crates/api/src/embedding/mod.rs` (comments)

### Docker Updates

- [x] ✅ Step 36: Update `crates/embedding-inference-api/Dockerfile`
- [x] ✅ Step 37: Update `crates/embedding-inference-api/.env.example`

### Kubernetes/Helm Updates

- [x] ✅ Step 38: Rename Helm template files (service, statefulset, serviceaccount)
- [x] ✅ Step 39: Update `deployment/helm/semantic-explorer/templates/_helpers.tpl` (inferenceApi → embeddingInferenceApi)
- [x] ✅ Step 40: Update `deployment/helm/semantic-explorer/values.yaml` (inferenceApi → embeddingInferenceApi)
- [x] ✅ Step 41: Update `deployment/helm/semantic-explorer/templates/embedding-inference-api-service.yaml`
- [x] ✅ Step 42: Update `deployment/helm/semantic-explorer/templates/embedding-inference-api-statefulset.yaml`
- [x] ✅ Step 43: Update `deployment/helm/semantic-explorer/templates/embedding-inference-api-serviceaccount.yaml`
- [x] ✅ Step 44: Update `deployment/helm/semantic-explorer/templates/hpa.yaml`
- [x] ✅ Step 45: Update `deployment/helm/semantic-explorer/templates/networkpolicy.yaml`
- [x] ✅ Step 46: Update `deployment/helm/semantic-explorer/templates/poddisruptionbudget.yaml`
- [x] ✅ Step 47: Update `deployment/helm/semantic-explorer/templates/servicemonitor.yaml`

### Docker Compose Updates

- [x] ✅ Step 48: Update `deployment/compose/compose.yaml`
- [x] ✅ Step 49: Update `deployment/compose/compose.dev.yaml`

### CI/CD Updates

- [x] ✅ Step 50: Update `.github/workflows/ci.yaml` (job names, image tags, paths)
- [x] ✅ Step 51: Update `.github/workflows/pr.yaml` (Dockerfile path references)

### Misc Updates

- [x] ✅ Step 52: Update `.vscode/tasks.json`
- [x] ✅ Step 53: Update `deployment/DEPLOYMENT_GUIDE.md`
- [x] ✅ Step 54: Update `.gitignore` (comment)

### Verification

- [x] ✅ Step 55: Verify rename with cargo build
- [x] ✅ Step 56: Verify rename with grep searches
- [x] ✅ Step 57: Verify rename with Helm validation

---

## Environment Variables Reference

### LLM Inference API (New Service)

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_INFERENCE_HOSTNAME` | 0.0.0.0 | Bind address |
| `LLM_INFERENCE_PORT` | 8091 | HTTP port |
| `CORS_ALLOWED_ORIGINS` | * | CORS origins |
| `LLM_ALLOWED_MODELS` | - | Comma-separated model IDs |
| `LLM_DEFAULT_MODEL` | mistralai/Mistral-7B-Instruct-v0.2 | Default model |
| `LLM_MAX_CONCURRENT_REQUESTS` | 10 | Max concurrent requests |
| `LLM_DEFAULT_TEMPERATURE` | 0.7 | Default temperature |
| `LLM_DEFAULT_TOP_P` | 0.9 | Default top_p |
| `LLM_DEFAULT_MAX_TOKENS` | 512 | Default max tokens |
| `LLM_MAX_TOKENS_LIMIT` | 4096 | Hard limit on tokens |
| `HF_HOME` | - | HuggingFace cache directory |
| `HF_ENDPOINT` | - | HF mirror/proxy URL |
| `SERVICE_NAME` | llm-inference-api | Service identifier |

### Embedding Inference API (Renamed from inference-api)

**Changed:**
- `INFERENCE_API_URL` → `EMBEDDING_INFERENCE_API_URL`
- `INFERENCE_API_TIMEOUT_SECS` → `EMBEDDING_INFERENCE_API_TIMEOUT_SECS`

**Service-specific (unchanged):**
- `INFERENCE_HOSTNAME` (internal to service)
- `INFERENCE_PORT` (internal to service)
- `INFERENCE_ALLOWED_EMBEDDING_MODELS`
- `INFERENCE_ALLOWED_RERANK_MODELS`
- `INFERENCE_MAX_BATCH_SIZE`

---

## Critical Files

### Task 1: llm-inference-api (New Files)

**Rust Source:**
- `crates/llm-inference-api/Cargo.toml`
- `crates/llm-inference-api/src/config.rs`
- `crates/llm-inference-api/src/errors.rs`
- `crates/llm-inference-api/src/llm.rs`
- `crates/llm-inference-api/src/models.rs`
- `crates/llm-inference-api/src/observability.rs`
- `crates/llm-inference-api/src/api/mod.rs`
- `crates/llm-inference-api/src/api/generation.rs`
- `crates/llm-inference-api/src/api/streaming.rs`
- `crates/llm-inference-api/src/api/chat.rs`
- `crates/llm-inference-api/src/api/health.rs`
- `crates/llm-inference-api/src/main.rs`

**Configuration:**
- `crates/llm-inference-api/.env.example`
- `crates/llm-inference-api/README.md`

**Docker:**
- `crates/llm-inference-api/Dockerfile`

**Helm Templates:**
- `deployment/helm/semantic-explorer/templates/llm-inference-api-service.yaml`
- `deployment/helm/semantic-explorer/templates/llm-inference-api-statefulset.yaml`
- `deployment/helm/semantic-explorer/templates/llm-inference-api-serviceaccount.yaml`

### Task 2: embedding-inference-api (Modified Files)

**Core Changes:**
- `Cargo.toml` - workspace member
- `crates/embedding-inference-api/Cargo.toml` - package name
- `crates/core/src/config.rs` - env var names
- `crates/core/src/embedder.rs` - env var usage

**Helm Templates (12+ files):**
- `deployment/helm/semantic-explorer/templates/_helpers.tpl`
- `deployment/helm/semantic-explorer/values.yaml`
- All embedding-inference-api template files

**CI/CD:**
- `.github/workflows/ci.yaml`
- `.github/workflows/pr.yaml`

---

## Testing Checklist

### LLM Inference API

- [ ] Build: `cargo build -p llm-inference-api`
- [ ] Run: `cargo run -p llm-inference-api`
- [ ] Swagger UI: http://localhost:8091/swagger-ui/
- [ ] Health check: `curl http://localhost:8091/health/ready`
- [ ] List models: `curl http://localhost:8091/api/models`
- [ ] Generate text: `curl -X POST http://localhost:8091/api/generate -H "Content-Type: application/json" -d '{"model": "mistralai/Mistral-7B-Instruct-v0.2", "prompt": "Hello"}'`
- [ ] Stream text: `curl -X POST http://localhost:8091/api/generate/stream`
- [ ] Chat completion: `curl -X POST http://localhost:8091/api/chat`
- [ ] Docker build: `docker build -f crates/llm-inference-api/Dockerfile .`

### Embedding Inference API Rename

- [ ] Build workspace: `cargo build --all`
- [ ] Run tests: `cargo test --all`
- [ ] Check references: `grep -r "inference-api" --exclude-dir=target`
- [ ] Helm lint: `helm lint deployment/helm/semantic-explorer`
- [ ] Helm template: `helm template semantic-explorer deployment/helm/semantic-explorer`

---

## Notes

- Created: 2026-01-17
- User Decisions:
  - ✅ Clean break for environment variable renaming (no backward compatibility)
  - ✅ Create llm-inference-api first, then rename existing service
  - ✅ LLM API features: Basic text generation + Streaming + Chat completions
  - ✅ mistral.rs integration: Direct Rust library dependency
