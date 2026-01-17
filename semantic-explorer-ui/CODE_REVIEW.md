## Overview
The Svelte UI renders a large set of management screens. The component layout is tidy, but data fetching is ad-hoc and easily overwhelms the API under user interaction.

## High
- **Search inputs spam the API on every keystroke** – `Collections.svelte` calls `fetchCollections()` inside a reactive effect tied directly to `searchQuery`, with no debounce or in-flight request cancellation. See [semantic-explorer-ui/src/lib/pages/Collections.svelte#L75-L199](semantic-explorer-ui/src/lib/pages/Collections.svelte#L75-L199). Similar patterns exist across other list pages. Add debouncing (e.g., `setTimeout`) or a derived store so typing doesn’t trigger dozens of parallel fetches.

## Medium
- **Each page reimplements fetch logic instead of using the shared helper** – `Collections.svelte` constructs URLs manually, handles errors, and calls `fetch` directly, ignoring the `apiCall` and `handleApiResponse` helpers. See [semantic-explorer-ui/src/lib/pages/Collections.svelte#L75-L148](semantic-explorer-ui/src/lib/pages/Collections.svelte#L75-L148) compared with the unused utilities in [semantic-explorer-ui/src/lib/utils/api.ts#L6-L35](semantic-explorer-ui/src/lib/utils/api.ts#L6-L35). Consolidate request/notification patterns to ensure consistent headers, auth, and error handling.
- **No abort handling for component unmounts** – Long-running fetches continue even if the user navigates away. Introduce `AbortController` support so disposals (or new keystrokes) cancel stale calls.

## Low
- **State duplication between `$state` and derived stores** – Components use `$state`/`$derived` locally, but the same pagination/search state is recomputed per page. Consider extracting reusable list stores to cut repetition and make testing easier.
