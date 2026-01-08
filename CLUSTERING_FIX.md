# HDBSCAN Clustering Fix - "Always 2 Clusters" Issue

**Created:** 2026-01-07  
**Status:** ✅ IMPLEMENTED - Ready for Testing  
**Issue:** Visualization clustering always returns 2 clusters regardless of UMAP/HDBSCAN settings

---

## Problem Summary

When running visualization transforms, HDBSCAN consistently produces only 2 clusters (plus noise labeled -1) regardless of:
- `min_cluster_size` setting (tested: 5, 10, 15, 25, 50)
- `n_components` setting (2D and 3D both affected)
- Dataset size or content

**Evidence from logs:**
```
Identified 2 clusters in X.XXs (cluster labels range: Some((-1, 1)))
```

---

## Root Cause Analysis

### Primary Cause: Missing `min_samples` Parameter

The `VisualizationConfig` includes `min_samples: Option<i32>` but this value is **never passed** to the `identify_topic_clusters()` function in cuml-wrapper-rs.

**Current call in `worker-visualizations/src/job.rs`:**
```rust
let (hdbscan, topic_vectors) = identify_topic_clusters(
    &normalized_embeddings,
    job.visualization_config.n_components as usize,
    job.visualization_config.min_cluster_size as usize,
    "euclidean",
    &document_vectors,
    n_features,
)?;
// NOTE: min_samples is NOT passed - cuML uses default (likely 5)
```

**cuML HDBSCAN Parameters (from cuml/cluster/hdbscan.hpp):**
```cpp
class RobustSingleLinkageParams {
  int min_samples      = 5;           // ← NOT EXPOSED in cuml-wrapper-rs
  int min_cluster_size = 5;           // ← Passed correctly
  int max_cluster_size = 0;
  float cluster_selection_epsilon = 0.0;
  bool allow_single_cluster = false;
};
```

### Why This Causes 2 Clusters

- With default `min_samples=5` and `min_cluster_size=15`, HDBSCAN is conservative
- Most points may be classified as noise (-1) with remaining points forming just 1-2 core clusters
- The `min_samples` parameter controls the "core distance" calculation - too high means fewer core points

---

## Implementation Plan

### Step 1: Update cuml-wrapper-rs C++ Wrapper
**Location:** `/home/jonathan/cuml-wrapper-rs-1/wrapper/`
- [x] `cuml_wrapper.h` already had `min_samples` parameter ✅
- [x] `cuml_wrapper.cpp` already passes `min_samples` to cuML ✅

### Step 2: Update cuml-wrapper-rs Rust FFI
**Location:** `/home/jonathan/cuml-wrapper-rs-1/src/lib.rs`
- [x] `HDBSCAN::fit()` already accepts `min_samples` ✅
- [x] Added `min_samples` to `identify_topic_clusters()` function signature
- [x] Added `min_samples` to `topic_modeling_pipeline()` function signature
- [x] Pass `min_samples` (instead of hardcoded `min_cluster_size`) to `HDBSCAN::fit()`

### Step 3: Update worker-visualizations
**Location:** `crates/worker-visualizations/src/job.rs`
- [x] Extract `min_samples` from config with fallback to `min_cluster_size`
- [x] Pass `min_samples` to `identify_topic_clusters()`
- [x] Updated logging to show `min_samples` value

### Step 4: Build and Test
- [x] Rebuild cuml-wrapper-rs: `cd /home/jonathan/cuml-wrapper-rs-1 && cargo build --release` ✅
- [x] Rebuild semantic-explorer: `cargo check -p worker-visualizations` ✅
- [ ] Test with various `min_samples` values (user testing required)

---

## Files to Modify

| File | Repository | Status |
|------|------------|--------|
| `wrapper/cuml_wrapper.h` | cuml-wrapper-rs-1 | ✅ Already had min_samples |
| `wrapper/cuml_wrapper.cpp` | cuml-wrapper-rs-1 | ✅ Already passes min_samples |
| `src/lib.rs` | cuml-wrapper-rs-1 | ✅ Updated identify_topic_clusters |
| `crates/worker-visualizations/src/job.rs` | semantic-explorer | ✅ Now passes min_samples |

---

## Current State (What's Already Working)

### ✅ UMAP Metric Parameter
- cuml-wrapper-rs accepts `metric` parameter for UMAP
- worker-visualizations passes `"cosine"` to `reduce_dimensionality()`

### ✅ L2 Normalization
- Input vectors are L2-normalized before UMAP
- UMAP embeddings are L2-normalized before HDBSCAN
- This approximates cosine distance using L2 (since cuML HDBSCAN only supports L2)

### ✅ Frontend Rendering
- 3D: Uses PointCloudLayer with proper depth rendering
- 2D: Uses ScatterplotLayer with appropriate radius
- Pagination implemented for large point sets

### ⚠️ HDBSCAN Limitation
- cuML HDBSCAN only supports L2/Euclidean distance (hardcoded in CUVS library)
- Workaround: L2-normalize vectors → L2 distance ≈ cosine distance on unit vectors

---

## Testing Protocol

### After Implementation
1. Run visualization with default settings → expect more than 2 clusters
2. Test parameter sensitivity:
   - `min_samples=1` → should produce more clusters
   - `min_samples=5` → moderate clustering
   - `min_samples=15` → conservative clustering
3. Test with known datasets (if available)

### Expected Log Output
```
Running HDBSCAN with min_cluster_size=15, min_samples=5, n_samples=1000
Identified N clusters in X.XXs (cluster labels range: Some((-1, N-1)))
```
Where N > 2 for datasets with clear structure.

---

## Previous Tracking Documents (Consolidated)

This document replaces:
- `CLUSTERING_INVESTIGATION.md` - Root cause analysis
- `TASK_TRACKER.md` - General task tracking
- `VISUALIZATION_FIX_PLAN.md` - Implementation phases

All relevant information has been consolidated here.
