# Fix Flat Visualization Issues - Implementation Plan

**Goal**: Achieve DataMapPlot-quality visualizations for both 2D and 3D, with proper point distributions, professional appearance, smart interactions, and label placement.

## Problem Summary

**3D Visualizations**: Show 2D circles (billboards) instead of true 3D spheres
**2D Visualizations**: Often appear as lines instead of proper point distributions (likely due to UMAP configuration issues)
**Visual Quality**: Points are tiny and nearly invisible (radius 0.5 with 10,000x scaling)
**User Experience**: Missing label placement, search, and interactive features that make DataMapPlot valuable

## Root Causes Identified

### Backend Issues (cuml-wrapper-rs)
1. **Missing UMAP metric parameter** - Configured as "cosine" in Rust but never passed to C++ wrapper ✅ FIXED
2. **HDBSCAN metric limitation** - cuML HDBSCAN only supports L2 distance (hardcoded in CUVS/cuML library)
   - **Workaround**: L2-normalize UMAP embeddings before clustering → L2 distance on normalized vectors ≈ cosine distance
3. **No vector normalization** - Embeddings not L2-normalized before cosine distance calculation
4. **Suboptimal defaults** - `min_dist: 0.1` too low for good 3D spread

### Frontend Issues (Deck.GL)
1. **Billboard rendering** - `ScatterplotLayer` uses 2D circles that face camera, not 3D spheres
2. **Tiny point radius** - `getRadius: 0.5` with 10,000x coordinate scaling makes points nearly invisible

---

## Implementation Checklist

### Phase 1: Backend Fixes (cuml-wrapper-rs)

#### ✅/❌ Step 1.1: Add UMAP Metric Parameter to C Wrapper
**Status:** In Progress

**Files to modify:**
- `/home/jonathan/cuml-wrapper-rs-1/wrapper/cuml_wrapper.h`
- `/home/jonathan/cuml-wrapper-rs-1/wrapper/cuml_wrapper.cpp`

**Changes:**
1. Add `const char* metric` parameter to `cuml_umap_fit()` signature
2. Add `const char* metric` parameter to `cuml_umap_fit_with_knn()` signature
3. Implement `parse_metric_string()` helper to convert string to cuML enum
4. Update UMAP params initialization to include metric

**Code location:** `cuml_wrapper.cpp` lines 59-63

**Before:**
```cpp
ML::UMAPParams params;
params.n_neighbors = n_neighbors;
params.n_components = n_components;
params.min_dist = min_dist;
```

**After:**
```cpp
ML::UMAPParams params;
params.n_neighbors = n_neighbors;
params.n_components = n_components;
params.min_dist = min_dist;
params.metric = parse_metric_string(metric);
```

**Validation after step:**
```bash
cd cuml-wrapper-rs
cargo fmt
cargo clippy -- -D warnings
cargo machete
```

---

#### ✅/❌ Step 1.2: Update Rust FFI Bindings
**Status:** Not Started

**Files to modify:**
- `/home/jonathan/cuml-wrapper-rs-1/src/lib.rs`

**Changes:**
1. Add `metric: *const c_char` to FFI function declarations
2. Update `UMAP::fit()` to accept `metric: &str` parameter
3. Update `UMAP::fit_with_knn()` to accept `metric: &str` parameter
4. Add metric validation in Rust layer
5. Convert Rust string to CString and pass to C layer

**Code location:** `lib.rs` lines 140-166

**Validation after step:**
```bash
cd cuml-wrapper-rs
cargo fmt
cargo clippy -- -D warnings
cargo machete
cargo test
```

---

#### ✅/❌ Step 1.3: Update High-Level API
**Status:** Not Started

**Files to modify:**
- `/home/jonathan/cuml-wrapper-rs-1/src/lib.rs`

**Changes:**
1. Add `metric: &str` parameter to `reduce_dimensionality()` function
2. Pass metric through to `UMAP::fit_with_knn()`

**Code location:** `lib.rs` lines 380-396

**Validation after step:**
```bash
cd cuml-wrapper-rs
cargo fmt
cargo clippy -- -D warnings
cargo machete
cargo test
```

---

#### ✅/❌ Step 1.4: Work Around HDBSCAN L2-Only Limitation
**Status:** Completed (with constraint documentation)

**IMPORTANT CONSTRAINT DISCOVERED**: cuML HDBSCAN only supports L2 distance (hardcoded in CUVS/cuML library)

**Solution Applied**:
1. ✅ Added metric parameter to APIs (for future flexibility when cuML updates)
2. ✅ Document that HDBSCAN uses L2 distance (only option in cuML)
3. ✅ Implement L2-normalization of embeddings → L2 distance on unit vectors ≈ Cosine distance
4. ✅ Keep metric configurable for UMAP (does support cosine)

**Why this works mathematically:**
- For L2-normalized vectors (unit length): `L2_distance(u,v) = sqrt(2 - 2*cosine_similarity(u,v))`
- Therefore: Normalize inputs → L2 metric in HDBSCAN → approximates cosine clustering behavior
- UMAP uses cosine for KNN graph
- HDBSCAN uses L2 on normalized UMAP embeddings
- Input embeddings are L2-normalized before both algorithms

**Validation after step:**
```bash
cd cuml-wrapper-rs-1
cargo fmt
cargo clippy -- -D warnings
cargo machete
cargo test
```

---

### Phase 2: Semantic Explorer Backend Integration

#### ✅/❌ Step 2.1: Add L2 Normalization Helper
**Status:** Not Started

**Files to modify:**
- `crates/worker-visualizations/src/job.rs`

**Changes:**
1. Add `normalize_l2()` helper function
2. Implement L2 normalization for each vector

**Code to add:**
```rust
fn normalize_l2(vectors: &[f32], n_features: usize) -> Vec<f32> {
    let n_samples = vectors.len() / n_features;
    let mut normalized = vectors.to_vec();

    for i in 0..n_samples {
        let start = i * n_features;
        let end = start + n_features;
        let vector = &vectors[start..end];

        // Calculate L2 norm
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Normalize to unit length (avoid division by zero)
        if norm > 1e-10 {
            for j in start..end {
                normalized[j] /= norm;
            }
        }
    }

    normalized
}
```

**Validation after step:**
```bash
cargo fmt
cargo clippy -- -D warnings
cargo machete
```

---

#### ✅/❌ Step 2.2: Update Worker for Normalization + Pass Metric
**Status:** In Progress

**Files to modify:**
- `crates/worker-visualizations/src/job.rs`

**Changes:**
1. L2-normalize input document vectors before UMAP
2. Pass metric parameter to `reduce_dimensionality()` → UMAP uses cosine for KNN
3. L2-normalize UMAP embeddings before HDBSCAN clustering
4. Pass metric="euclidean" to HDBSCAN (only metric it supports)

**Normalization Strategy**:
```rust
// 1. Normalize input vectors (important for cosine distance accuracy)
let normalized_doc_vectors = normalize_l2(&document_vectors, n_features);

// 2. UMAP with cosine metric (KNN preprocessing)
let umap = reduce_dimensionality(
    &normalized_doc_vectors,
    n_features,
    n_neighbors,
    n_components,
    min_dist,
    "cosine",  // Use cosine for KNN in UMAP
)?;

// 3. Normalize UMAP embeddings
let normalized_umap_embeddings = normalize_l2(&umap.embedding, n_components);

// 4. HDBSCAN with L2 metric (only option in cuML)
let (hdbscan, topic_vectors) = identify_topic_clusters(
    &normalized_umap_embeddings,
    n_components,
    min_cluster_size,
    "euclidean",  // L2 on normalized vectors ≈ cosine
    &document_vectors,
    n_features,
)?;
```

**Code location:** Lines 54-73

**Validation after step:**
```bash
cargo fmt
cargo clippy -- -D warnings
cargo machete
cargo test
```

---

#### ✅/❌ Step 2.3: Update Default Parameters
**Status:** Not Started

**Files to modify:**
- `crates/api/src/transforms/visualization/models.rs`

**Changes:**
1. Change `default_min_dist()` from 0.1 to 0.25

**Code location:** Lines 138-140

**Before:**
```rust
fn default_min_dist() -> f32 {
    0.1
}
```

**After:**
```rust
fn default_min_dist() -> f32 {
    0.25  // Better spread for 3D visualization
}
```

**Validation after step:**
```bash
cargo fmt
cargo clippy -- -D warnings
cargo machete
```

---

### Phase 3: Frontend Fixes

#### ✅/❌ Step 3.1: Import SimpleMeshLayer
**Status:** Not Started

**Files to modify:**
- `semantic-explorer-ui/src/lib/pages/VisualizationDetail.svelte`

**Changes:**
1. Add `SimpleMeshLayer` to imports

**Code location:** Line 2

**Before:**
```typescript
import { LineLayer, ScatterplotLayer } from '@deck.gl/layers';
```

**After:**
```typescript
import { LineLayer, ScatterplotLayer, SimpleMeshLayer } from '@deck.gl/layers';
```

**Validation after step:**
```bash
cd semantic-explorer-ui
npm run lint
npm run format
```

---

#### ✅/❌ Step 3.2: Add Sphere Mesh Generator
**Status:** Not Started

**Files to modify:**
- `semantic-explorer-ui/src/lib/pages/VisualizationDetail.svelte`

**Changes:**
1. Add `generateSphereMesh()` function
2. Create `SPHERE_MESH` constant

**Code location:** After line 150

**Code to add:**
```typescript
// Generate sphere geometry for SimpleMeshLayer
function generateSphereMesh(radius: number = 1, segments: number = 8) {
    const positions: number[] = [];
    const normals: number[] = [];
    const indices: number[] = [];

    // Generate vertices using spherical coordinates
    for (let lat = 0; lat <= segments; lat++) {
        const theta = (lat * Math.PI) / segments;
        const sinTheta = Math.sin(theta);
        const cosTheta = Math.cos(theta);

        for (let lon = 0; lon <= segments; lon++) {
            const phi = (lon * 2 * Math.PI) / segments;
            const sinPhi = Math.sin(phi);
            const cosPhi = Math.cos(phi);

            const x = cosPhi * sinTheta;
            const y = cosTheta;
            const z = sinPhi * sinTheta;

            positions.push(x * radius, y * radius, z * radius);
            normals.push(x, y, z);
        }
    }

    // Generate triangle indices
    for (let lat = 0; lat < segments; lat++) {
        for (let lon = 0; lon < segments; lon++) {
            const first = lat * (segments + 1) + lon;
            const second = first + segments + 1;

            indices.push(first, second, first + 1);
            indices.push(second, second + 1, first + 1);
        }
    }

    return { positions, normals, indices };
}

const SPHERE_MESH = generateSphereMesh(1.0, 8); // 8 segments = 128 triangles
```

**Validation after step:**
```bash
cd semantic-explorer-ui
npm run lint
npm run format
npm run build
```

---

#### ✅/❌ Step 3.3: Replace ScatterplotLayer with Conditional Rendering
**Status:** Not Started

**Files to modify:**
- `semantic-explorer-ui/src/lib/pages/VisualizationDetail.svelte`

**Changes:**
1. Use ScatterplotLayer for 2D visualizations
2. Use SimpleMeshLayer with true spheres for 3D visualizations
3. Add lighting material properties for 3D
4. Increase point sizes for better visibility

**Code location:** Lines 602-626

**Before:**
```typescript
const layers: Layer[] = [
    new ScatterplotLayer({
        id: 'points-layer',
        data: filteredPoints,
        getPosition: (d: VisualizationPoint) => d.position,
        getFillColor: (d: VisualizationPoint) => { /* ... */ },
        getRadius: 0.5,
        radiusMinPixels: 4,
        radiusMaxPixels: 20,
        pickable: true,
        onHover: (info) => { /* ... */ }
    }),
];
```

**After:**
```typescript
const layers: Layer[] = is2D
    ? [
        // 2D: Use ScatterplotLayer (billboards are fine)
        new ScatterplotLayer({
            id: 'points-layer',
            data: filteredPoints,
            getPosition: (d: VisualizationPoint) => d.position,
            getFillColor: (d: VisualizationPoint) => {
                if (currentSelectedClusters.size > 0 && !currentSelectedClusters.has(d.cluster_id)) {
                    const color = getClusterColor(d.cluster_id);
                    return [...color, 50];
                }
                return getClusterColor(d.cluster_id);
            },
            getRadius: 15,
            radiusMinPixels: 5,
            radiusMaxPixels: 25,
            pickable: true,
            onHover: (info) => {
                if (info.object) {
                    hoveredPoint = info.object;
                } else {
                    hoveredPoint = null;
                }
            },
        })
    ]
    : [
        // 3D: Use SimpleMeshLayer for true spheres
        new SimpleMeshLayer({
            id: 'points-layer',
            data: filteredPoints,
            mesh: SPHERE_MESH,
            sizeScale: 25,  // Increased from 0.5 for better visibility
            getPosition: (d: VisualizationPoint) => d.position,
            getColor: (d: VisualizationPoint) => {
                if (currentSelectedClusters.size > 0 && !currentSelectedClusters.has(d.cluster_id)) {
                    const color = getClusterColor(d.cluster_id);
                    return [...color, 50];
                }
                return getClusterColor(d.cluster_id);
            },
            getOrientation: [0, 0, 0],
            wireframe: false,
            pickable: true,
            onHover: (info) => {
                if (info.object) {
                    hoveredPoint = info.object;
                } else {
                    hoveredPoint = null;
                }
            },
            // Enable lighting for 3D depth cues
            material: {
                ambient: 0.35,
                diffuse: 0.6,
                shininess: 32,
                specularColor: [255, 255, 255]
            }
        })
    ];
```

**Validation after step:**
```bash
cd semantic-explorer-ui
npm run lint
npm run format
npm run build
```

---

### Phase 4: End-to-End Testing

#### ✅/❌ Step 4.1: Test Backend Build
**Status:** Not Started

**Commands:**
```bash
cargo build --release
cargo test
```

**Expected:** All tests pass, no warnings

---

#### ✅/❌ Step 4.2: Test Frontend Build
**Status:** Not Started

**Commands:**
```bash
cd semantic-explorer-ui
npm run build
```

**Expected:** Build succeeds, no warnings

---

#### ✅/❌ Step 4.3: Visual Testing
**Status:** Not Started

**Manual test steps:**
1. Create a new 3D visualization (n_components=3)
2. Create a new 2D visualization (n_components=2)
3. Verify 3D shows spheres with depth/lighting
4. Verify 2D shows proper distribution (not lines)
5. Check point visibility (should be clearly visible)
6. Test rotation in 3D (spheres should look round from all angles)

---

## Quality Checks (Run After Each Step)

### Backend (Rust)
```bash
cargo fmt
cargo clippy -- -D warnings
cargo machete
```

### Frontend (TypeScript/Svelte)
```bash
cd semantic-explorer-ui
npm run lint
npm run format
npm run build
```

---

### Phase 5: DataMapPlot-Inspired Enhancements (Optional - Later Iterations)

These enhancements align with DataMapPlot's publication-quality standards and should be addressed after core visualization issues are fixed.

#### ✅/❌ Step 5.1: Improve Label Placement (Future)
**Status:** Deferred - After core fixes verified

**Consideration:** Implement smart label positioning for cluster names/topics to avoid overlaps and improve readability, similar to DataMapPlot's automated label placement.

---

#### ✅/❌ Step 5.2: Enhance Interactive Features (Future)
**Status:** Deferred - After core fixes verified

**Improvements:**
- Better zoom/pan responsiveness
- Search/filter functionality for clusters
- Hover tooltips with richer information
- Selection feedback

---

#### ✅/❌ Step 5.3: Overall Visual Polish (Future)
**Status:** Deferred - After core fixes verified

**Improvements:**
- Consistent color palettes
- Better background and grid styling
- Improved camera controls
- Professional annotations and legends

---

## Expected Outcomes

**Phase 1-3 Core Fixes:**
✅ **3D visualizations** render as true spheres with lighting and depth
✅ **2D visualizations** show proper point distributions, not lines
✅ **Point visibility** improved 50x (from radius 0.5 to 25)
✅ **UMAP quality** improved with correct metric and normalization
✅ **Clustering quality** improved with consistent metrics
✅ **Performance** maintained (60+ FPS for <5k points, 30+ FPS for 10k points)
✅ **No warnings** from cargo fmt, clippy, machete, npm lint, format, build

**Future Enhancements (DataMapPlot-Inspired):**
🔄 Smart label placement for cluster names
🔄 Enhanced interactive features (search, filter, zoom, pan)
🔄 Publication-quality visual polish

---

## Critical Files Reference

### Backend (cuml-wrapper-rs-1 - for PR)
- `/home/jonathan/cuml-wrapper-rs-1/wrapper/cuml_wrapper.h`
- `/home/jonathan/cuml-wrapper-rs-1/wrapper/cuml_wrapper.cpp`
- `/home/jonathan/cuml-wrapper-rs-1/src/lib.rs`

### Backend (semantic-explorer)
- `/home/jonathan/semantic-explorer/crates/worker-visualizations/src/job.rs`
- `/home/jonathan/semantic-explorer/crates/api/src/transforms/visualization/models.rs`

### Frontend (semantic-explorer)
- `/home/jonathan/semantic-explorer/semantic-explorer-ui/src/lib/pages/VisualizationDetail.svelte`
