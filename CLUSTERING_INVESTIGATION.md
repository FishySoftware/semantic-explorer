# HDBSCAN Clustering Investigation

**Date:** 2026-01-07  
**Issue:** Visualization clustering always returns 2 clusters regardless of data

---

## Current Implementation

### Location
`crates/worker-visualizations/src/job.rs`

### Parameters Used
```rust
identify_topic_clusters(
    &normalized_embeddings,          // L2-normalized UMAP output
    n_components as usize,            // Typically 2 or 3
    min_cluster_size as usize,        // Default: 15 (from config)
    "euclidean",                      // Distance metric
    &document_vectors,                // Original high-dimensional vectors
    n_features,                       // Original vector dimensionality
)
```

### Configuration
- **min_cluster_size**: 15 (from `VisualizationConfig`)
- **min_samples**: 5 (in config but NOT passed to function)
- **metric**: "euclidean" (hardcoded)

---

## Identified Issues

### 1. ⚠️ Missing `min_samples` Parameter
**Problem:** The `VisualizationConfig` has an optional `min_samples` field, but it's never passed to `identify_topic_clusters()`.

**Impact:** HDBSCAN defaults may be too restrictive, causing over-clustering.

**Fix Required:** Update `cuml-wrapper-rs` to accept `min_samples` parameter.

### 2. 🔍 Hardcoded "euclidean" Metric
**Problem:** While the config has a `metric` field (used by UMAP), HDBSCAN always uses "euclidean".

**Status:** This is likely intentional since cuML's HDBSCAN only supports Euclidean/L2 distance. The code compensates by L2-normalizing embeddings first (L2 distance on normalized vectors ≈ cosine distance).

### 3. 📊 Insufficient Diagnostics
**Problem:** Limited logging makes it hard to debug clustering behavior.

**Fixed:** Added comprehensive logging:
- UMAP parameters and output shape
- HDBSCAN parameters and input size
- Cluster count and label range
- Processing times

---

## Diagnostic Steps Added

### Enhanced Logging
```rust
// Before UMAP
info!("Running UMAP with n_neighbors={}, n_components={}, min_dist={}, metric={}", ...);

// After UMAP
info!("Reduced dimensionality in {:.2}s (output shape: {}x{})", ...);

// Before HDBSCAN
info!("Running HDBSCAN with min_cluster_size={}, n_samples={}, n_components={}", ...);

// After HDBSCAN
info!("Identified {} clusters in {:.2}s (cluster labels range: {:?})", ...);
```

### What to Check
1. **Label distribution**: Are all points assigned to just 2 clusters, or are most marked as noise (-1)?
2. **Input data quality**: Check UMAP output shape and verify normalization
3. **Parameter sensitivity**: Test with different min_cluster_size values (5, 10, 20, 30)

---

## Root Cause Hypotheses

### Hypothesis 1: cuml-wrapper-rs Issue (Most Likely)
- The wrapper may have a bug in parameter handling
- Default `min_samples` in cuML might be too high
- Investigate cuML version and wrapper implementation
- the `cuml-wrapper-rs` repo is owned by me at `/home/jonathan/cuml-wrapper-rs-1`
- source build environment: `source setup_build.sh` in the repo root

### Hypothesis 2: Data Characteristics
- If documents are very similar, UMAP might produce tight clusters
- High-dimensional curse: original vectors may not have clear structure
- L2 normalization might be removing important magnitude information

### Hypothesis 3: HDBSCAN Configuration
- `min_cluster_size=15` might be too large for the dataset
- Need to expose and tune `min_samples` parameter
- HDBSCAN's hierarchical nature might need different parameters

---

## Recommended Actions

### Immediate (Done ✅)
- [x] Add diagnostic logging for UMAP and HDBSCAN
- [x] Log cluster label ranges and distributions
- [x] Document the missing `min_samples` parameter

### Short-term (Action Required)
- [ ] Check cuml-wrapper-rs repository for:
  - HDBSCAN function signature
  - How parameters are passed to cuML
  - Any known issues or recent changes
- [ ] Test with known good dataset (e.g., sklearn digits, iris)
- [ ] Try different min_cluster_size values: [5, 10, 20, 30, 50]
- [ ] Examine actual cluster labels from HDBSCAN output

### Medium-term
- [ ] Update cuml-wrapper-rs to accept `min_samples` parameter
- [ ] Add cluster quality metrics (silhouette score, etc.)
- [ ] Consider alternative clustering algorithms (DBSCAN, K-means) for comparison
- [ ] Add visualization of UMAP output to verify dimensionality reduction

### Long-term
- [ ] Investigate upgrading cuML CUDA library (per TASK_TRACKER notes)
- [ ] Add automated tests with synthetic datasets
- [ ] Create cluster quality dashboard

---

## Testing Protocol

### Step 1: Verify UMAP Output
```bash
# Set LOG_FORMAT=human for readable output
export LOG_FORMAT=human
# Run a visualization job and check logs
```

Expected output:
```
Running UMAP with n_neighbors=15, n_components=3, min_dist=0.1, metric=euclidean
Reduced dimensionality in 2.34s (output shape: 1053x3)
```

### Step 2: Examine HDBSCAN Input
Look for:
- Number of samples (should match UMAP output)
- n_components (2 or 3)
- min_cluster_size value

### Step 3: Analyze Cluster Labels
```
Identified 2 clusters in 1.23s (cluster labels range: Some((-1, 1)))
```

Questions:
- What's the distribution? (e.g., 50 noise points, 500 in cluster 0, 503 in cluster 1)
- Are there outliers (label = -1)?
- Is the split balanced or heavily skewed?

### Step 4: Test cuml-wrapper-rs Directly
In the cuml-wrapper-rs-1 repository:
1. Create a simple test with known data
2. Verify HDBSCAN returns expected cluster count
3. Check if min_samples parameter exists

---

## Additional Notes

### cuML Version Issues
From TASK_TRACKER.md:
> "Previous CUDA library version was downgraded due to .so file issues. Consider upgrading cuML version if we can resolve the library paths."

This suggests there may be underlying library compatibility issues that could affect clustering behavior.

### Build Instructions
- cuml-wrapper-rs: `source setup_build.sh` (in repo root)
- worker-visualizations: `source .scripts/activate_build_env.sh` (in crate root)

---

## Related Files
- `crates/worker-visualizations/src/job.rs` - Clustering implementation
- `crates/core/src/models.rs` - VisualizationConfig definition
- External: `/home/jonathan/cuml-wrapper-rs-1` - Wrapper library
