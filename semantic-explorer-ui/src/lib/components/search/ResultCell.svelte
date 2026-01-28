<script lang="ts">
	import type { DocumentResult, SearchMatch } from '../../types/models';
	import { getScoreBadgeClass } from '../../utils/scoreColors';

	let {
		result,
		searchMode,
		isExpanded: _isExpanded,
		onToggleExpand,
	}: {
		result: DocumentResult | SearchMatch;
		searchMode: 'documents' | 'chunks';
		isExpanded: boolean;
		onToggleExpand: () => void;
	} = $props();

	// Type guards
	function isDocumentResult(r: DocumentResult | SearchMatch): r is DocumentResult {
		return 'item_id' in r && 'best_score' in r;
	}

	// Get the score to display
	let score = $derived.by(() => {
		if (isDocumentResult(result)) {
			return result.best_score;
		} else {
			return result.score;
		}
	});

	// Get the title to display
	let title = $derived.by(() => {
		if (isDocumentResult(result)) {
			return result.item_title;
		} else {
			// For chunks, use item_title from metadata if available
			return result.metadata?.item_title || `Chunk ${result.metadata?.chunk_index ?? '?'}`;
		}
	});
</script>

<div
	class="group"
	role="button"
	tabindex="0"
	onclick={(e) => {
		e.stopPropagation();
		onToggleExpand();
	}}
	onkeydown={(e) => {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onToggleExpand();
		}
	}}
>
	<div class="flex items-start justify-between gap-2">
		<!-- Title -->
		<div class="flex-1 min-w-0">
			<div
				class="text-sm font-medium text-gray-900 dark:text-white truncate group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors"
				{title}
			>
				{#if searchMode === 'documents'}
					📄
				{:else}
					📝
				{/if}
				{title}
			</div>

			{#if searchMode === 'documents' && isDocumentResult(result)}
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">
					<span
						class="inline-block px-1.5 py-0.5 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded"
					>
						{result.chunk_count} chunk{result.chunk_count !== 1 ? 's' : ''}
					</span>
				</div>
			{:else if !isDocumentResult(result) && result.metadata?.chunk_index !== undefined}
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">
					Chunk #{result.metadata.chunk_index}
				</div>
			{/if}
		</div>

		<!-- Score badge -->
		<div class="shrink-0">
			<span
				class="inline-flex items-center px-2 py-1 rounded-full text-xs font-bold {getScoreBadgeClass(
					score
				)}"
				title="Similarity Score: {score.toFixed(6)}"
			>
				{score.toFixed(3)}
			</span>
		</div>
	</div>
</div>
