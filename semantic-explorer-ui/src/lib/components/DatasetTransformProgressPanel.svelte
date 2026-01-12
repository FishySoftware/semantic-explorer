<script lang="ts">
	import { onMount } from 'svelte';

	interface EmbedderProgress {
		embedder_id: number;
		embedder_name: string;
		status: 'pending' | 'processing' | 'completed' | 'failed';
		items_processed: number;
		total_items: number;
		embeddings_count: number;
		error?: string;
	}

	interface Props {
		datasetTransformId: number;
		title: string;
		sourceDatasetTitle: string;
		embedderProgresses?: EmbedderProgress[];
		overallStatus: 'processing' | 'completed' | 'failed';
		totalItemsProcessed?: number;
		totalItems?: number;
		startedAt?: string;
	}

	let {
		datasetTransformId,
		title,
		sourceDatasetTitle,
		embedderProgresses = [],
		overallStatus,
		totalItemsProcessed = 0,
		totalItems = 0,
		startedAt,
	}: Props = $props();

	// Reactive time for elapsed/estimated calculations
	let now = $state(Date.now());

	onMount(() => {
		// Update time every second to trigger re-calculations
		const interval = setInterval(() => {
			now = Date.now();
		}, 1000);

		return () => clearInterval(interval);
	});

	// Calculate overall progress
	const overallProgress = $derived(
		totalItems && totalItemsProcessed ? Math.round((totalItemsProcessed / totalItems) * 100) : 0
	);

	// Reference datasetTransformId to satisfy linter
	const transformKey = $derived(`${datasetTransformId}-${title}`);

	// Calculate elapsed and estimated time remaining
	const elapsedTime = $derived.by(() => {
		if (!startedAt) return 0;
		const start = new Date(startedAt).getTime();
		return Math.floor((now - start) / 1000); // seconds
	});

	const estimatedTimeRemaining = $derived.by(() => {
		if (totalItemsProcessed === 0 || totalItems === 0) return 0;
		const progressRate = totalItemsProcessed / (elapsedTime || 1);
		const remainingItems = totalItems - totalItemsProcessed;
		return Math.max(0, Math.floor(remainingItems / progressRate));
	});

	const formatTime = (seconds: number): string => {
		if (seconds < 60) return `${seconds}s`;
		if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
		const hours = Math.floor(seconds / 3600);
		const minutes = Math.floor((seconds % 3600) / 60);
		return `${hours}h ${minutes}m`;
	};

	const completedEmbedders = $derived(
		embedderProgresses.filter((e) => e.status === 'completed').length
	);
	const processingEmbedders = $derived(
		embedderProgresses.filter((e) => e.status === 'processing').length
	);
	const failedEmbedders = $derived(embedderProgresses.filter((e) => e.status === 'failed').length);
</script>

<div
	class="mt-4 p-4 bg-linear-to-br from-purple-50 to-purple-25 dark:from-purple-900/20 dark:to-purple-900/10 border border-purple-200 dark:border-purple-800 rounded-lg"
	data-transform-key={transformKey}
>
	<!-- Header with title and overall progress -->
	<div class="flex items-center justify-between mb-4">
		<div class="flex items-center gap-3">
			<div class="shrink-0">
				{#if overallStatus === 'processing'}
					<div class="relative inline-flex">
						<svg
							class="animate-spin h-6 w-6 text-purple-600"
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
						>
							<circle
								class="opacity-25"
								cx="12"
								cy="12"
								r="10"
								stroke="currentColor"
								stroke-width="4"
							></circle>
							<path
								class="opacity-75"
								fill="currentColor"
								d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
							></path>
						</svg>
					</div>
				{:else if overallStatus === 'completed'}
					<svg class="h-6 w-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"
						></path>
					</svg>
				{:else}
					<svg class="h-6 w-6 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M6 18L18 6M6 6l12 12"
						></path>
					</svg>
				{/if}
			</div>
			<div>
				<h3 class="text-sm font-semibold text-purple-900 dark:text-purple-100">{title}</h3>
				<p class="text-xs text-purple-700 dark:text-purple-300">from {sourceDatasetTitle}</p>
			</div>
		</div>

		<!-- Summary stats -->
		<div class="text-right">
			<p class="text-sm font-medium text-purple-900 dark:text-purple-100">
				{totalItemsProcessed} / {totalItems} items
			</p>
			<p class="text-xs text-purple-700 dark:text-purple-300">
				{overallProgress}% complete
				{#if overallStatus === 'processing' && estimatedTimeRemaining > 0}
					· ~{formatTime(estimatedTimeRemaining)} remaining
				{/if}
			</p>
		</div>
	</div>

	<!-- Main progress bar -->
	<div class="mb-4">
		<div class="w-full bg-purple-200 dark:bg-purple-900 rounded-full h-3 overflow-hidden">
			<div
				class="bg-linear-to-r from-purple-500 to-purple-600 h-3 rounded-full transition-all duration-300 ease-out shadow-lg"
				style="width: {overallProgress}%"
			></div>
		</div>
		<div class="mt-1 flex justify-between text-xs text-purple-700 dark:text-purple-300">
			<span>{overallProgress}% complete</span>
			<span>Elapsed: {formatTime(elapsedTime)}</span>
		</div>
	</div>

	<!-- Per-embedder progress -->
	{#if embedderProgresses.length > 0}
		<div
			class="mb-3 max-h-64 overflow-y-auto border border-purple-200 dark:border-purple-800 rounded bg-white dark:bg-gray-800/50"
		>
			<div class="divide-y divide-purple-100 dark:divide-purple-900">
				{#each embedderProgresses as embedder (embedder.embedder_id)}
					<div class="px-4 py-3 text-xs">
						<!-- Embedder header -->
						<div class="flex items-center justify-between mb-2">
							<div class="flex items-center gap-2">
								{#if embedder.status === 'processing'}
									<svg
										class="h-4 w-4 text-purple-600 animate-spin"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
									>
										<circle
											cx="12"
											cy="12"
											r="10"
											stroke="currentColor"
											stroke-width="2"
											fill="none"
										></circle>
										<path stroke-linecap="round" d="M12 2a10 10 0 010 20"></path>
									</svg>
								{:else if embedder.status === 'completed'}
									<svg
										class="h-4 w-4 text-green-600"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
									>
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M5 13l4 4L19 7"
										></path>
									</svg>
								{:else if embedder.status === 'failed'}
									<svg
										class="h-4 w-4 text-red-600"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
									>
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M6 18L18 6M6 6l12 12"
										></path>
									</svg>
								{:else}
									<svg
										class="h-4 w-4 text-gray-400"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
									>
										<circle
											cx="12"
											cy="12"
											r="10"
											stroke="currentColor"
											stroke-width="2"
											fill="none"
										></circle>
									</svg>
								{/if}
								<span class="font-medium text-gray-900 dark:text-white"
									>{embedder.embedder_name}</span
								>
							</div>
							<span
								class="px-2 py-0.5 rounded text-xs font-medium {embedder.status === 'processing'
									? 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300'
									: embedder.status === 'completed'
										? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
										: embedder.status === 'failed'
											? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
											: 'bg-gray-100 dark:bg-gray-900/30 text-gray-700 dark:text-gray-300'}"
							>
								{embedder.status}
							</span>
						</div>

						<!-- Progress details -->
						<div
							class="flex items-center justify-between text-xs mb-1 text-gray-600 dark:text-gray-400"
						>
							<span>{embedder.items_processed} / {embedder.total_items} items</span>
							<span>{embedder.embeddings_count} embeddings generated</span>
						</div>

						<!-- Progress bar -->
						<div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5 overflow-hidden">
							<div
								class="bg-linear-to-r from-purple-500 to-purple-600 h-1.5 rounded-full transition-all duration-200"
								style="width: {embedder.total_items > 0
									? (embedder.items_processed / embedder.total_items) * 100
									: 0}%"
							></div>
						</div>

						{#if embedder.error}
							<p class="text-red-600 dark:text-red-400 mt-1">{embedder.error}</p>
						{/if}
					</div>
				{/each}
			</div>
		</div>

		<!-- Summary stats -->
		<div class="grid grid-cols-3 gap-2">
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Processing</p>
				<p class="text-lg font-semibold text-purple-600 dark:text-purple-400">
					{processingEmbedders}
				</p>
			</div>
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Completed</p>
				<p class="text-lg font-semibold text-green-600 dark:text-green-400">{completedEmbedders}</p>
			</div>
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Failed</p>
				<p class="text-lg font-semibold text-red-600 dark:text-red-400">{failedEmbedders}</p>
			</div>
		</div>
	{/if}
</div>

<style>
	:global(.animate-fadeIn) {
		animation: fadeIn 0.3s ease-in;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateY(-10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
