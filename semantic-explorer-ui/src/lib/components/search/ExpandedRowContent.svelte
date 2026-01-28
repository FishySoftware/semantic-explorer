<script lang="ts">
	import type {
		DocumentResult,
		SearchMatch,
		EmbeddedDatasetSearchResults,
	} from '../../types/models';
	import { getScoreBadgeClass, getScoreBorderClass } from '../../utils/scoreColors';

	let {
		result,
		embeddedDataset: _embeddedDataset,
		searchMode,
	}: {
		result: DocumentResult | SearchMatch;
		embeddedDataset: EmbeddedDatasetSearchResults;
		searchMode: 'documents' | 'chunks';
	} = $props();

	// Type guards
	function isDocumentResult(r: DocumentResult | SearchMatch): r is DocumentResult {
		return 'item_id' in r && 'best_score' in r;
	}

	// Copy text to clipboard
	async function copyToClipboard(text: string) {
		try {
			await navigator.clipboard.writeText(text);
		} catch (err) {
			console.error('Failed to copy text:', err);
		}
	}

	// Get the content text to display
	let contentText = $derived.by(() => {
		if (isDocumentResult(result)) {
			return result.best_chunk?.text || '';
		} else {
			return result.text || '';
		}
	});

	// Get score
	let score = $derived.by(() => {
		if (isDocumentResult(result)) {
			return result.best_score;
		} else {
			return result.score;
		}
	});
</script>

<div
	class="rounded-lg border {getScoreBorderClass(
		score
	)} bg-white dark:bg-gray-800 p-3 space-y-3 overflow-hidden"
>
	<!-- Header with title and score -->
	<div class="flex items-start justify-between gap-3">
		<div class="flex-1 min-w-0 overflow-hidden">
			{#if searchMode === 'documents' && isDocumentResult(result)}
				<h4 class="text-sm font-semibold text-gray-900 dark:text-white break-all">
					📄 {result.item_title}
				</h4>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1 space-y-1">
					<div>Item ID: <span class="font-mono">{result.item_id}</span></div>
					<div>
						<span
							class="inline-block px-1.5 py-0.5 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded"
						>
							{result.chunk_count} chunk{result.chunk_count !== 1 ? 's' : ''}
						</span>
					</div>
				</div>
			{:else if !isDocumentResult(result)}
				<h4 class="text-sm font-semibold text-gray-900 dark:text-white break-all">
					📝 {result.metadata?.item_title || 'Chunk'}
				</h4>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1 space-y-1">
					{#if result.metadata?.chunk_index !== undefined}
						<div>Chunk Index: <span class="font-mono">{result.metadata.chunk_index}</span></div>
					{/if}
					{#if result.metadata?.item_id !== undefined}
						<div>Item ID: <span class="font-mono">{result.metadata.item_id}</span></div>
					{/if}
					<div>Point ID: <span class="font-mono text-xs">{result.id}</span></div>
				</div>
			{/if}
		</div>

		<!-- Score -->
		<div class="text-right shrink-0">
			<div class="text-xs text-gray-500 dark:text-gray-400">Score</div>
			<span
				class="inline-flex items-center px-2 py-1 rounded-full text-sm font-bold {getScoreBadgeClass(
					score
				)}"
			>
				{score.toFixed(4)}
			</span>
		</div>
	</div>

	<!-- Content -->
	<div class="relative">
		<div class="text-xs text-gray-500 dark:text-gray-400 mb-1 flex items-center justify-between">
			<span>
				{#if searchMode === 'documents'}
					Best Matching Chunk:
				{:else}
					Content:
				{/if}
			</span>
			<button
				type="button"
				onclick={() => copyToClipboard(contentText)}
				class="text-blue-600 dark:text-blue-400 hover:underline text-xs"
				title="Copy to clipboard"
			>
				📋 Copy
			</button>
		</div>
		<div
			class="bg-gray-50 dark:bg-gray-900 rounded p-3 border border-gray-200 dark:border-gray-700 max-h-64 overflow-y-auto"
		>
			<p
				class="text-sm text-gray-900 dark:text-gray-100 leading-relaxed whitespace-pre-wrap break-all"
			>
				{contentText}
			</p>
		</div>
	</div>

	<!-- Metadata (if available) -->
	{#if !isDocumentResult(result) && result.metadata && Object.keys(result.metadata).length > 0}
		{@const displayMetadata = Object.entries(result.metadata).filter(
			([key]) => !['item_title', 'chunk_index', 'item_id'].includes(key)
		)}
		{#if displayMetadata.length > 0}
			<details class="text-xs">
				<summary
					class="text-gray-500 dark:text-gray-400 cursor-pointer hover:text-gray-700 dark:hover:text-gray-300"
				>
					Additional Metadata ({displayMetadata.length} fields)
				</summary>
				<div
					class="mt-2 bg-gray-50 dark:bg-gray-900 rounded p-2 border border-gray-200 dark:border-gray-700"
				>
					<pre class="text-xs text-gray-700 dark:text-gray-300 overflow-x-auto">{JSON.stringify(
							Object.fromEntries(displayMetadata),
							null,
							2
						)}</pre>
				</div>
			</details>
		{/if}
	{/if}

	{#if searchMode === 'documents' && isDocumentResult(result) && result.best_chunk?.metadata}
		{@const displayMetadata = Object.entries(result.best_chunk.metadata).filter(
			([key]) => !['item_title', 'chunk_index', 'item_id'].includes(key)
		)}
		{#if displayMetadata.length > 0}
			<details class="text-xs">
				<summary
					class="text-gray-500 dark:text-gray-400 cursor-pointer hover:text-gray-700 dark:hover:text-gray-300"
				>
					Chunk Metadata ({displayMetadata.length} fields)
				</summary>
				<div
					class="mt-2 bg-gray-50 dark:bg-gray-900 rounded p-2 border border-gray-200 dark:border-gray-700"
				>
					<pre class="text-xs text-gray-700 dark:text-gray-300 overflow-x-auto">{JSON.stringify(
							Object.fromEntries(displayMetadata),
							null,
							2
						)}</pre>
				</div>
			</details>
		{/if}
	{/if}
</div>
