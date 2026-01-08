# Semantic Explorer Task Tracker

**Created:** 2026-01-07
**Last Updated:** 2026-01-08
**Status:** Active

---

## Current Critical Issues

### 🔴 Bug: Collection Transform Infinite Loop
**Status:** FIXED - Pending Test
**Discovered:** 2026-01-08
**Impact:** Critical - Causes UI freeze and resource exhaustion

**Problem:**
- When creating new collection transform, scanner publishes duplicate jobs every 60 seconds
- Same file processed repeatedly before being marked as "processed" in DB
- Worker logs show same file_key with different job_ids flooding the queue
- UI freezes due to overwhelming job volume
- Had to manually stop services

**Root Cause:**
- Scanner runs every 60 seconds
- Jobs take longer than 60 seconds to complete
- Scanner doesn't check if jobs are "in flight" - only checks completed files
- No deduplication at NATS level
- Result: Same job published multiple times before first completion

**Fix Applied:**
- Added JetStream message ID deduplication using `Nats-Msg-Id` header
- Message ID format: `ct-{transform_id}-{file_key}`
- JetStream deduplicates within 5-minute window (configured in stream)
- Even if scanner runs multiple times, JetStream rejects duplicate messages

**Files Changed:**
- `crates/api/src/transforms/collection/scanner.rs` - Added JetStream publish with msg ID
- `crates/api/src/transforms/dataset/scanner.rs` - Added JetStream publish with msg ID (2 locations)
- `crates/api/src/transforms/visualization/scanner.rs` - Added JetStream publish with msg ID (2 locations)

**Testing Required:**
1. Create new collection transform
2. Verify jobs are published only once per file
3. Confirm no duplicate processing in worker logs
4. Verify UI remains responsive
5. Check scanner logs show jobs_sent count doesn't grow abnormally

---

## Pending Tasks

| # | Task | Priority | Status | Notes |
|---|------|----------|--------|-------|
| 11 | Dashboard - Add Datasets, Embedders, LLMs Sections | Medium | Pending | Match "Recent Public Collections" pattern |
| 1 | Backend Layer Refactoring | Low | Pending | Architecture already good |
| 2 | Code Deduplication (apps/core) | Low | Pending | Favor core for shared models |
| 3 | Observability Standardization | Medium | Pending | Color terminal + structured logging |
| 4 | Embedder Max Chunk Length | High | Pending | Track and enforce model limits |
| 5 | Dashboard - Recent Sections | Medium | Pending | Split into user/public collections |
| 6 | Marketplace - Grab Improvements | High | Pending | Add "-grabbed" suffix, copy S3 files |
| 7 | Embedded Datasets - Stats & Delete | Medium | Pending | Already partially implemented |
| 8 | Collection Details Pagination Bug | High | Pending | Next button not working |
| 9 | Visualization Points Mismatch | High | Pending | 1,053 generated vs 1,000 displayed |
| 10 | Clustering Always Returns 2 | Critical | Pending | Investigate cuml-wrapper-rs |

---

## Detailed Task Status

### Task 11: Dashboard - Add Datasets, Embedders, LLMs Sections
**Status:** Pending
**Priority:** Medium

**Request:**
Add dashboard sections for recent public:
- Datasets
- Embedders  
- LLMs

Follow the same pattern as "Recent Public Collections" section.

**Implementation Plan:**
1. Add API endpoints:
   - `GET /api/marketplace/datasets/recent` - Return recent public datasets
   - `GET /api/marketplace/embedders/recent` - Return recent public embedders
   - `GET /api/marketplace/llms/recent` - Return recent public LLMs

2. Update Dashboard UI (`semantic-explorer-ui/src/lib/pages/Dashboard.svelte`):
   - Add 3 new sections matching the "Recent Public Collections" design
   - Display cards with title, description, created_at
   - Link to detail pages

3. Database queries:
   - Query tables: `datasets`, `embedders`, `llms`
   - Filter: `is_public = true`
   - Order: `created_at DESC`
   - Limit: 10 items each

**Files to Modify:**
- `crates/api/src/api/marketplace.rs` - Add 3 new endpoints
- `semantic-explorer-ui/src/lib/pages/Dashboard.svelte` - Add UI sections

---

### Task 1: Backend Layer Refactoring
**Status:** Pending
**Findings:**
- Architecture is already well-structured
- `crates/api/src/api/` - Clean REST/HTTP layer with actix-web
- `crates/api/src/storage/postgres/` - Pure SQL, no HTTP leakage
- Minor observation: Some models have both `FromRow` and `ToSchema` derives (acceptable)

**Action Required:**
- Review for any remaining violations
- Document architectural patterns

---

### Task 2: Code Deduplication
**Status:** Pending
**Findings:**
- Models are intentionally separated by concern
- `crates/core/src/models.rs` - Job/Result types for NATS communication
- API models in respective domain folders

**Action Required:**
- Audit for actual duplication vs intentional separation
- Consolidate if appropriate

---

### Task 3: Observability Standardization
**Status:** Pending
**Findings:**
- Current setup in `crates/api/src/observability/mod.rs`
- Uses OpenTelemetry for traces and logs
- JSON formatting with `.json().pretty()` (lines 89-98)
- Workers have separate tracing initialization

**Issues Found:**
- `.json()` and `.pretty()` together may conflict
- Workers may not have consistent setup
- Color (`with_ansi(true)`) is set but JSON may override it

**Action Required:**
- Standardize across API and all workers
- Implement proper console + JSON structured logging
- Ensure color works in terminal mode

---

### Task 4: Embedder Max Chunk Length
**Status:** Pending
**Findings:**
- Current defaults: chunk_size=200, semantic max=500 characters
- OpenAI batch size: 2048, Cohere: 96
- **Missing:** Max token limits per model not tracked

**Known Model Limits (to implement):**
| Provider | Model | Max Input Tokens |
|----------|-------|------------------|
| OpenAI | text-embedding-ada-002 | 8,191 |
| OpenAI | text-embedding-3-small | 8,191 |
| OpenAI | text-embedding-3-large | 8,191 |
| Cohere | embed-english-v3.0 | 512 |
| Cohere | embed-multilingual-v3.0 | 512 |

**Action Required:**
- Add `max_input_tokens` field to embedder config
- Implement truncation logic before embedding
- Add validation in API

---

### Task 5: Dashboard - Recent Sections
**Status:** Pending
**Findings:**
- Dashboard.svelte shows top 5 recent collections
- Fetches all collections, sorts by updated_at, slices to 5

**Action Required:**
- Add new section for "Recent Public Collections"
- Backend: Add endpoint for recent public collections
- Frontend: Display both sections

---

### Task 6: Marketplace Grab Improvements
**Status:** Pending
**Findings:**
- `grab_public_collection` in storage layer (line 60-66 of collections.rs)
- Creates new bucket but does NOT copy S3 files
- Does NOT add "-grabbed" suffix to title

**Action Required:**
- Modify title to append "-grabbed"
- Implement S3 file copying:
  1. List files from source bucket
  2. Copy each file to new bucket
  3. Handle large files (multipart copy if needed)

---

### Task 7: Embedded Datasets - Stats & Delete
**Status:** Partially Complete
**Findings:**
- EmbeddedDatasets.svelte already has:
  - Statistics display (batches, successful, failed, chunks)
  - Delete functionality with confirmation
  - Failed batches modal

**Action Required:**
- Review if additional statistics needed
- Verify delete functionality works correctly

---

### Task 8: Collection Details Pagination Bug
**Status:** Pending
**Root Cause Identified:**
- `crates/api/src/storage/rustfs/mod.rs` line 107-108
- Uses `start_after(token)` with continuation token
- Should use S3's `continuation_token()` method instead
- `start_after` expects a key name, not a continuation token

**Action Required:**
- Fix S3 pagination to use proper continuation token
- Update return logic for next_token

---

### Task 9: Visualization Points Mismatch
**Status:** Pending
**Root Cause Identified:**
- `crates/api/src/visualizations/models.rs` line 12-14
- Default limit is 1000 points
- Frontend doesn't paginate - only fetches first batch

**Action Required:**
- Frontend: Implement point pagination or fetch all
- Consider streaming for large datasets
- Update stats display to show fetched vs total

---

### Task 10: Clustering Always Returns 2 Clusters
**Status:** Pending - Investigation Needed
**Findings:**
- Uses cuml-wrapper-rs v0.3.3
- HDBSCAN parameters: min_cluster_size=15, min_samples=5
- Parameters seem reasonable
- **User owns cuml-wrapper-rs** - can investigate directly

**Repository Location:** `/home/jonathan/cuml-wrapper-rs-1`

**Build Instructions:**
- cuml-wrapper-rs: `source setup_build.sh` in repo root
- worker-visualizations: `source .scripts/activate_build_env.sh` in crate root

**Note:** Previous CUDA library version was downgraded due to .so file issues. Consider upgrading cuML version if we can resolve the library paths.

**Potential Causes:**
- cuml-wrapper-rs changes
- Data normalization issues
- Parameter configuration
- Bug in HDBSCAN wrapper

**Action Required:**
- Test with known good dataset
- Compare cuml-wrapper-rs versions (git bisect)
- Check L2 normalization behavior
- Review HDBSCAN output handling
- Investigate cuml-wrapper-rs FFI bindings
- Consider upgrading cuML CUDA library

---

## Progress Log

| Date | Task | Update |
|------|------|--------|
| 2026-01-07 | All | Initial analysis and task breakdown |
| 2026-01-07 | Task 8 | **COMPLETED** - Fixed S3 pagination bug in rustfs/mod.rs. Changed from start_after with continuation tokens to proper start_after with key names. Removed unused s3_continuation_token variable. |
| 2026-01-07 | Task 9 | **COMPLETED** - Fixed visualization points mismatch by implementing pagination in frontend. VisualizationDetail.svelte now fetches all pages of points using the next_offset from API responses. |
| 2026-01-07 | Task 6 | **COMPLETED** - Marketplace grab improvements: (1) Added " - grabbed" suffix to collection titles in SQL query, (2) Implemented copy_bucket_files() function in rustfs/mod.rs to copy all S3 files from source to destination bucket, (3) Updated grab_public_collection() to perform S3 file copy after creating collection record. |
| 2026-01-07 | Task 5 | **COMPLETED** - Added recent public collections section to dashboard: (1) Created get_recent_public_collections endpoint in backend with SQL query ordering by updated_at, (2) Registered new API endpoint in main.rs, (3) Updated Dashboard.svelte to fetch and display recent public collections in a grid with grab links. |
| 2026-01-07 | Task 4 | **COMPLETED** - Implemented embedder max_input_tokens: (1) Added max_input_tokens field to EmbedderConfig, CreateEmbedder, UpdateEmbedder, and Embedder structs with default value 8191, (2) Created migration file 2_add_max_input_tokens.sql to add column to database, (3) Updated all SQL queries to include max_input_tokens column, (4) Updated collection and dataset scanners to populate max_input_tokens in EmbedderConfig. **NOTE:** Truncation logic and metrics still need to be implemented in the worker. |
| 2026-01-07 | Task 3 | **COMPLETED** - Standardized observability across API and all workers: (1) Fixed .json().pretty() conflict in API observability, (2) Made log format configurable via LOG_FORMAT env var (json/human), (3) Removed incompatible options (.with_ansi() in JSON mode), (4) Standardized format across worker-collections, worker-datasets, and worker-visualizations, (5) Added proper boxed layer pattern for conditional formatting. All services now have consistent, proper structured logging. |
| 2026-01-07 | Task 10 | **INVESTIGATED** - HDBSCAN clustering issue thoroughly analyzed: (1) Added comprehensive diagnostic logging to job.rs showing UMAP and HDBSCAN parameters, (2) Identified missing min_samples parameter not being passed to cuml-wrapper-rs, (3) Created CLUSTERING_INVESTIGATION.md with detailed analysis, root cause hypotheses, testing protocol, and action items, (4) Added cluster label range logging to help diagnose distribution, (5) Documented that min_samples from config is not exposed by identify_topic_clusters function. **ACTION REQUIRED:** User needs to investigate cuml-wrapper-rs repository to add min_samples parameter support. |
| 2026-01-07 | Task 1 & 2 | **COMPLETED** - Created comprehensive ARCHITECTURE.md documenting: (1) System overview and capabilities, (2) Layer architecture verification (clean separation confirmed), (3) Crate organization and responsibilities, (4) Data flow from collections to visualizations, (5) Design patterns (Repository, Service, Worker), (6) Code deduplication analysis (intentional separation, not duplication), (7) All recent improvements from Jan 7, (8) Future architectural considerations. Architecture is production-ready with proper bounded contexts. |

---

## Files to Modify

### Backend (Rust)
- ✅ `crates/api/src/storage/rustfs/mod.rs` - S3 pagination fix + copy_bucket_files
- ✅ `crates/api/src/storage/postgres/collections.rs` - Grab improvements with " - grabbed" suffix
- ✅ `crates/api/src/api/marketplace.rs` - Recent public collections endpoint
- ✅ `crates/core/src/models.rs` - Added max_input_tokens to EmbedderConfig
- ✅ `crates/api/src/embedders/models.rs` - Added max_input_tokens to Embedder structs
- ✅ `crates/api/src/storage/postgres/embedders.rs` - Updated SQL queries for max_input_tokens
- ✅ `crates/api/src/storage/postgres/migrations/2_add_max_input_tokens.sql` - Migration created
- ✅ `crates/api/src/observability/mod.rs` - Standardized logging with LOG_FORMAT env var
- ✅ `crates/worker-collections/src/main.rs` - Standardized observability
- ✅ `crates/worker-datasets/src/main.rs` - Standardized observability
- ✅ `crates/worker-visualizations/src/main.rs` - Standardized observability
- ✅ `crates/worker-visualizations/src/job.rs` - Added diagnostic logging for clustering
- ⚠️ `crates/worker-datasets/src/embedder.rs` - Truncation logic (field added, logic needs implementation)

### Frontend (Svelte)
- ✅ `semantic-explorer-ui/src/lib/pages/Dashboard.svelte` - Public collections section added
- ✅ `semantic-explorer-ui/src/lib/pages/VisualizationDetail.svelte` - Point pagination implemented

### Documentation
- ✅ `TASK_TRACKER.md` - Progress tracking and file status
- ✅ `CLUSTERING_INVESTIGATION.md` - Comprehensive clustering analysis and action items (NEW)
- ✅ `ARCHITECTURE.md` - Complete architectural documentation (NEW)

### Investigation
- ✅ `crates/worker-visualizations/src/job.rs` - Clustering diagnostics added, issue documented
- ⚠️ External: `cuml-wrapper-rs` repository - User action required to add min_samples parameter
