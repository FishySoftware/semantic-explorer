# Search Page Revamp - Implementation Plan

## Overview
Revamp the Search page in semantic-explorer-ui to use a table-based comparison view that allows users to visually inspect search results across multiple embedded datasets without overflow issues.

## Current Issues
- Side-by-side column layout causes overflow when using more than 3 embedded datasets
- Difficult to compare results across different embedders at a glance
- No clear visual indication of ranking differences between embedders

## Design Requirements
- **Table-based approach**: Rows represent result positions (1st, 2nd, 3rd, etc.), columns represent each embedded dataset
- **Compact view**: Show only document/chunk title and similarity score in table cells
- **Expandable rows**: Click to expand and show detailed content
- **Separate table structures**: Different layouts for Documents mode vs Chunks mode
- **Responsive design**: Handle varying numbers of embedded datasets gracefully

---

## Table Structure Design

### Documents Mode Table

| Row # | Embedded Dataset 1 | Embedded Dataset 2 | Embedded Dataset 3 | ... |
|-------|-------------------|-------------------|-------------------|-----|
| 1 | Title + Score | Title + Score | Title + Score | ... |
| 2 | Title + Score | Title + Score | Title + Score | ... |
| 3 | Title + Score | Title + Score | Title + Score | ... |
| ... | ... | ... | ... | ... |

**Table Cell Content (Compact View):**
- Document title (truncated if too long)
- Similarity score (best_score)
- Chunk count badge
- Click to expand for details

**Expanded Row Content:**
- Full document title
- Best matching chunk text
- Chunk metadata (chunk_index, item_id, etc.)
- All chunk scores (if available)

### Chunks Mode Table

| Row # | Embedded Dataset 1 | Embedded Dataset 2 | Embedded Dataset 3 | ... |
|-------|-------------------|-------------------|-------------------|-----|
| 1 | Title + Score | Title + Score | Title + Score | ... |
| 2 | Title + Score | Title + Score | Title + Score | ... |
| 3 | Title + Score | Title + Score | Title + Score | ... |
| ... | ... | ... | ... | ... |

**Table Cell Content (Compact View):**
- Chunk index or document title
- Similarity score
- Click to expand for details

**Expanded Row Content:**
- Full chunk text
- Chunk metadata (chunk_index, item_id, item_title, etc.)
- Document title (if available)

---

## Component Architecture

### 1. SearchResultsTable.svelte (New Component)
Main table component that displays the comparison view.

**Props:**
- `results: EmbeddedDatasetSearchResults[]` - Search results from API
- `searchMode: 'documents' | 'chunks'` - Current search mode
- `onViewDataset?: (id: number) => void` - Callback to view dataset
- `onViewEmbedder?: (id: number) => void` - Callback to view embedder
- `onViewEmbeddedDataset?: (id: number) => void` - Callback to view embedded dataset

**State:**
- `expandedRows: Set<string>` - Track which rows are expanded
- `sortColumn: string | null` - Current sort column
- `sortDirection: 'asc' | 'desc'` - Sort direction

**Features:**
- Horizontal scroll for many embedded datasets
- Sticky first column (row numbers)
- Sticky header row
- Expandable rows for details
- Score color coding

### 2. ResultCell.svelte (New Component)
Individual table cell component for displaying a single result.

**Props:**
- `result: DocumentResult | SearchMatch` - The result to display
- `embeddedDataset: EmbeddedDatasetSearchResults` - Parent embedded dataset info
- `searchMode: 'documents' | 'chunks'` - Current search mode
- `isExpanded: boolean` - Whether this row is expanded
- `onToggleExpand: () => void` - Callback to toggle expansion

**Features:**
- Compact display (title + score)
- Score color coding
- Hover effects
- Click to expand

### 3. ExpandedRowContent.svelte (New Component)
Component for displaying detailed content when a row is expanded.

**Props:**
- `result: DocumentResult | SearchMatch` - The result to display
- `embeddedDataset: EmbeddedDatasetSearchResults` - Parent embedded dataset info
- `searchMode: 'documents' | 'chunks'` - Current search mode

**Features:**
- Full content display
- Metadata display
- Syntax highlighting for code (if applicable)
- Copy to clipboard button

---

## Visual Design

### Score Color Coding
- **High score (≥ 0.8)**: Green background/badge
- **Medium score (0.5 - 0.79)**: Yellow/amber background/badge
- **Low score (< 0.5)**: Red/orange background/badge
- **No result**: Gray/empty cell

### Table Styling
- Sticky header row with embedded dataset names
- Sticky first column with row numbers
- Alternating row colors for readability
- Hover effects on cells
- Border between columns
- Horizontal scroll for many columns

### Responsive Design
- On mobile: Stack embedded datasets vertically or use horizontal scroll
- On tablet: Show up to 3-4 columns, scroll for more
- On desktop: Show as many columns as fit, scroll for more

---

## Implementation Steps

### Phase 1: Core Table Structure
1. Create `SearchResultsTable.svelte` component
2. Implement basic table structure with embedded dataset columns
3. Add sticky header and first column
4. Implement horizontal scrolling

### Phase 2: Documents Mode
1. Implement Documents mode table structure
2. Create `ResultCell.svelte` for document results
3. Add document title, score, and chunk count display
4. Implement expandable row functionality
5. Create `ExpandedRowContent.svelte` for document details

### Phase 3: Chunks Mode
1. Implement Chunks mode table structure
2. Reuse `ResultCell.svelte` with chunk-specific display
3. Add chunk index and document title display
4. Implement expandable row for chunk details

### Phase 4: Visual Enhancements
1. Add score color coding
2. Implement hover effects
3. Add sorting functionality
4. Add filtering options (by score threshold)

### Phase 5: Responsive Design
1. Test on different screen sizes
2. Implement mobile-friendly layout
3. Ensure horizontal scrolling works smoothly

### Phase 6: Integration
1. Replace current side-by-side view in `Search.svelte`
2. Test with various numbers of embedded datasets
3. Test both search modes
4. Verify error handling

---

## Data Flow

```
Search.svelte
  ├─ performSearch()
  │   └─ API call to /api/search
  │       └─ Returns SearchResponse
  │           └─ results: EmbeddedDatasetSearchResults[]
  │
  └─ SearchResultsTable
      ├─ Process results into table rows
      │   ├─ Documents mode: Group by document position
      │   └─ Chunks mode: Group by chunk position
      │
      ├─ Render table header (embedded dataset names)
      ├─ Render table rows
      │   ├─ Row number (sticky)
      │   └─ ResultCell for each embedded dataset
      │       ├─ Compact view (title + score)
      │       └─ ExpandedRowContent (when expanded)
      │
      └─ Handle user interactions
          ├─ Click to expand/collapse
          ├─ Click to view dataset/embedder
          └─ Sort/filter
```

---

## Edge Cases to Handle

1. **Different result counts per embedded dataset**: Show empty cells or "No result" for missing positions
2. **Error in embedded dataset search**: Display error message in column header or cell
3. **Very long titles**: Truncate with ellipsis, show full on hover or expand
4. **Very long content**: Truncate in expanded view, show full on click
5. **No results at all**: Show empty state message
6. **Single embedded dataset**: Still use table format for consistency
7. **Many embedded datasets (10+)**: Ensure horizontal scrolling works well

---

## API Considerations

The existing API structure is sufficient for the new design:
- `SearchResponse.results` contains all embedded dataset results
- Each `EmbeddedDatasetSearchResults` has `matches` (chunks) or `documents` (documents mode)
- No API changes required

---

## Testing Checklist

- [ ] Table displays correctly with 1 embedded dataset
- [ ] Table displays correctly with 3 embedded datasets
- [ ] Table displays correctly with 10+ embedded datasets
- [ ] Horizontal scrolling works smoothly
- [ ] Sticky header and first column work correctly
- [ ] Expandable rows work in Documents mode
- [ ] Expandable rows work in Chunks mode
- [ ] Score color coding is accurate
- [ ] Sorting works correctly
- [ ] Filtering works correctly
- [ ] Responsive design works on mobile/tablet/desktop
- [ ] Error states display correctly
- [ ] Empty states display correctly
- [ ] Clicking dataset/embedder links works correctly
- [ ] Dark mode styling works correctly

---

## Future Enhancements (Out of Scope)

- Export comparison results to CSV
- Visual heatmap of score differences
- Statistical comparison (average score, variance, etc.)
- Side-by-side diff view for same document across embedders
- Save comparison queries for later
