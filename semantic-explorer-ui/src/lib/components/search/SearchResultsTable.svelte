<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import type {
		EmbeddedDatasetSearchResults,
		DocumentResult,
		SearchMatch,
	} from '../../types/models';
	import ResultCell from './ResultCell.svelte';
	import ExpandedRowContent from './ExpandedRowContent.svelte';

	let {
		results,
		searchMode,
		onViewDataset,
		onViewEmbedder,
		onViewEmbeddedDataset,
	}: {
		results: EmbeddedDatasetSearchResults[];
		searchMode: 'documents' | 'chunks';
		onViewDataset?: (_id: number) => void;
		onViewEmbedder?: (_id: number) => void;
		onViewEmbeddedDataset?: (_id: number) => void;
	} = $props();

	// Track expanded rows
	let expandedRows = new SvelteSet<number>();

	// Calculate the maximum number of results across all embedded datasets
	let maxResults = $derived.by(() => {
		if (searchMode === 'documents') {
			return Math.max(...results.map((r) => r.documents?.length || 0), 0);
		} else {
			return Math.max(...results.map((r) => r.matches?.length || 0), 0);
		}
	});

	// Get result at a specific position for an embedded dataset
	function getResultAtPosition(
		embeddedDataset: EmbeddedDatasetSearchResults,
		position: number
	): DocumentResult | SearchMatch | null {
		if (searchMode === 'documents') {
			return embeddedDataset.documents?.[position] ?? null;
		} else {
			return embeddedDataset.matches?.[position] ?? null;
		}
	}

	function toggleRow(rowIndex: number) {
		if (expandedRows.has(rowIndex)) {
			expandedRows.delete(rowIndex);
		} else {
			expandedRows.add(rowIndex);
		}
	}
</script>

{#if results.length === 0}
	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
		<p class="text-gray-500 dark:text-gray-400">No results to display</p>
	</div>
{:else if maxResults === 0}
	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
		<p class="text-gray-500 dark:text-gray-400">No matching results found</p>
	</div>
{:else}
	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
		<!-- Table container with horizontal scroll -->
		<div class="overflow-x-auto">
			<table class="w-full border-collapse min-w-max">
				<!-- Header row with embedded dataset names -->
				<thead class="bg-gray-50 dark:bg-gray-900 sticky top-0 z-10">
					<tr>
						<!-- Row number column (sticky) -->
						<th
							class="sticky left-0 z-20 bg-gray-100 dark:bg-gray-800 px-4 py-3 text-left text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider border-r border-gray-200 dark:border-gray-700 min-w-15"
						>
							#
						</th>
						<!-- Embedded dataset columns -->
						{#each results as embeddedDataset (embeddedDataset.embedded_dataset_id)}
							<th
								class="px-4 py-3 text-left border-r border-gray-200 dark:border-gray-700 last:border-r-0 min-w-70 max-w-100"
							>
								<div class="space-y-1">
									<button
										type="button"
										onclick={() => onViewEmbeddedDataset?.(embeddedDataset.embedded_dataset_id)}
										class="text-sm font-semibold text-blue-600 dark:text-blue-400 hover:underline text-left block truncate max-w-full"
										title={embeddedDataset.embedded_dataset_title}
									>
										{embeddedDataset.embedded_dataset_title}
									</button>
									<div class="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
										<button
											type="button"
											onclick={() => onViewDataset?.(embeddedDataset.source_dataset_id)}
											class="hover:text-blue-600 dark:hover:text-blue-400 hover:underline truncate"
											title={embeddedDataset.source_dataset_title}
										>
											📊 {embeddedDataset.source_dataset_title}
										</button>
										<span class="text-gray-300 dark:text-gray-600">|</span>
										<button
											type="button"
											onclick={() => onViewEmbedder?.(embeddedDataset.embedder_id)}
											class="hover:text-blue-600 dark:hover:text-blue-400 hover:underline truncate"
											title={embeddedDataset.embedder_name}
										>
											🧠 {embeddedDataset.embedder_name}
										</button>
									</div>
									{#if embeddedDataset.error}
										<div class="text-xs text-red-500 dark:text-red-400 mt-1">
											⚠️ {embeddedDataset.error}
										</div>
									{/if}
								</div>
							</th>
						{/each}
					</tr>
				</thead>

				<!-- Result rows -->
				<tbody class="divide-y divide-gray-200 dark:divide-gray-700">
					{#each { length: maxResults } as _, rowIndex (rowIndex)}
						{@const isExpanded = expandedRows.has(rowIndex)}
						<!-- Compact row -->
						<tr
							class="hover:bg-gray-50 dark:hover:bg-gray-700/50 cursor-pointer transition-colors {isExpanded
								? 'bg-blue-50 dark:bg-blue-900/20'
								: ''}"
							onclick={() => toggleRow(rowIndex)}
						>
							<!-- Row number (sticky) -->
							<td
								class="sticky left-0 z-10 bg-gray-50 dark:bg-gray-800 px-4 py-3 text-sm font-medium text-gray-600 dark:text-gray-400 border-r border-gray-200 dark:border-gray-700 {isExpanded
									? 'bg-blue-100 dark:bg-blue-900/30'
									: ''}"
							>
								<div class="flex items-center gap-1">
									<span class="text-gray-400 dark:text-gray-500 text-xs">
										{isExpanded ? '▼' : '▶'}
									</span>
									{rowIndex + 1}
								</div>
							</td>
							<!-- Result cells -->
							{#each results as embeddedDataset (embeddedDataset.embedded_dataset_id)}
								{@const result = getResultAtPosition(embeddedDataset, rowIndex)}
								<td
									class="px-4 py-3 border-r border-gray-200 dark:border-gray-700 last:border-r-0 align-top"
								>
									{#if result}
										<ResultCell
											{result}
											{searchMode}
											{isExpanded}
											onToggleExpand={() => toggleRow(rowIndex)}
										/>
									{:else}
										<div class="text-sm text-gray-400 dark:text-gray-500 italic py-2 text-center">
											—
										</div>
									{/if}
								</td>
							{/each}
						</tr>

						<!-- Expanded row content -->
						{#if isExpanded}
							<tr class="bg-gray-50 dark:bg-gray-900/50">
								<td
									class="sticky left-0 z-10 bg-gray-100 dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700"
								></td>
								{#each results as embeddedDataset (embeddedDataset.embedded_dataset_id)}
									{@const result = getResultAtPosition(embeddedDataset, rowIndex)}
									<td
										class="px-4 py-4 border-r border-gray-200 dark:border-gray-700 last:border-r-0 align-top min-w-70 max-w-100"
									>
										{#if result}
											<ExpandedRowContent {result} {embeddedDataset} {searchMode} />
										{:else}
											<div class="text-sm text-gray-400 dark:text-gray-500 italic text-center">
												No result at this position
											</div>
										{/if}
									</td>
								{/each}
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</div>

		<!-- Legend / Footer -->
		<div
			class="px-4 py-3 bg-gray-50 dark:bg-gray-900 border-t border-gray-200 dark:border-gray-700"
		>
			<div class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
				<div class="flex items-center gap-4">
					<span>Score Legend:</span>
					<span class="flex items-center gap-1">
						<span class="w-3 h-3 rounded-full bg-green-500"></span>
						High (≥0.8)
					</span>
					<span class="flex items-center gap-1">
						<span class="w-3 h-3 rounded-full bg-yellow-500"></span>
						Medium (0.5-0.79)
					</span>
					<span class="flex items-center gap-1">
						<span class="w-3 h-3 rounded-full bg-red-500"></span>
						Low (&lt;0.5)
					</span>
				</div>
				<span>Click a row to expand details</span>
			</div>
		</div>
	</div>
{/if}
