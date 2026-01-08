# HDBSCAN Clustering Fix - "Always 2 Clusters" Issue

**Created:** 2026-01-07  
**Updated:** 2026-01-07  
**Status:** ✅ FULLY IMPLEMENTED - Ready for Testing  
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

### Primary Causes

1. **Missing `min_samples` Parameter** - The config value was not passed through

2. **Using EOM Instead of LEAF Cluster Selection** - EOM (Excess of Mass) tends to produce fewer, larger clusters by selecting clusters with the most excess mass. LEAF method selects clusters at the leaves of the condensed tree, producing more fine-grained clusters.

---

## Implemented Fixes

### Fix 1: Pass `min_samples` Parameter
- Added `min_samples` parameter to `identify_topic_clusters()` and `topic_modeling_pipeline()` in cuml-wrapper-rs
- worker-visualizations now extracts and passes `min_samples` from config

### Fix 2: Add `cluster_selection_method` Parameter (LEAF)
- Added `cluster_selection_method` parameter to cuml_wrapper.h/.cpp
- Updated HDBSCAN::fit() to pass cluster_selection_method to cuML  
- Set `cluster_selection_method=1` (LEAF) in worker-visualizations
- LEAF produces more fine-grained clusters than EOM (value=0)

---

## Files Modified

| File | Repository | Status |
|------|------------|--------|
| `wrapper/cuml_wrapper.h` | cuml-wrapper-rs-1 | ✅ Added cluster_selection_method |
| `wrapper/cuml_wrapper.cpp` | cuml-wrapper-rs-1 | ✅ Added cluster_selection_method to params |
| `src/lib.rs` | cuml-wrapper-rs-1 | ✅ Added min_samples + cluster_selection_method |
| `crates/worker-visualizations/src/job.rs` | semantic-explorer | ✅ Uses LEAF (cluster_selection_method=1) |

---

## Testing Required

After rebuild, test with:
- Default settings (min_cluster_size=15, LEAF)
- Various min_cluster_size values (5, 10, 15, 25)
- Various min_samples values (3, 5, 10)

Expected: More than 2 clusters with LEAF method.

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
