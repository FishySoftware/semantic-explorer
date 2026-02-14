<script lang="ts">
	import { Badge } from 'flowbite-svelte';
	import { onMount } from 'svelte';

	interface VisualizationRun {
		visualization_id: number;
		status: string;
		started_at: string | null;
		completed_at: string | null;
		point_count?: number | null;
		embedding_count?: number;
		cluster_count?: number | null;
		error_message: string | null;
		stats_json?: Record<string, unknown> | null;
		created_at: string;
	}

	interface Props {
		lastRunStatus: string | null;
		lastRunAt: string | null;
		lastError: string | null;
		processingRuns: VisualizationRun[];
		onDismiss?: () => void;
	}

	let { lastRunStatus, lastRunAt, lastError, processingRuns, onDismiss }: Props = $props();

	let now = $state(Date.now());

	onMount(() => {
		const interval = setInterval(() => {
			now = Date.now();
		}, 1000);
		return () => clearInterval(interval);
	});

	const isProcessing = $derived(lastRunStatus === 'pending' || lastRunStatus === 'processing');

	const elapsedTime = $derived.by(() => {
		if (!lastRunAt) return 0;
		const start = new Date(lastRunAt).getTime();
		return Math.floor((now - start) / 1000);
	});

	const formatTime = (seconds: number): string => {
		if (seconds < 60) return `${seconds}s`;
		if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
		const hours = Math.floor(seconds / 3600);
		const minutes = Math.floor((seconds % 3600) / 60);
		return `${hours}h ${minutes}m`;
	};
</script>

<div
	class="mb-6 rounded-lg border overflow-hidden transition-all duration-300 {isProcessing
		? 'bg-blue-50 dark:bg-blue-900/10 border-blue-200 dark:border-blue-800'
		: lastRunStatus === 'failed'
			? 'bg-red-50 dark:bg-red-900/10 border-red-200 dark:border-red-800'
			: 'bg-green-50 dark:bg-green-900/10 border-green-200 dark:border-green-800'}"
>
	<!-- Progress bar -->
	{#if isProcessing}
		<div class="h-1.5 bg-gray-200 dark:bg-gray-700">
			<div class="h-full bg-blue-500 animate-pulse" style="width: 100%"></div>
		</div>
	{/if}

	<div class="p-4">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				{#if isProcessing}
					<div
						class="flex items-center justify-center w-8 h-8 rounded-full bg-blue-100 dark:bg-blue-900/30"
					>
						<svg
							class="animate-spin w-5 h-5 text-blue-600 dark:text-blue-400"
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
				{:else if lastRunStatus === 'failed'}
					<div
						class="flex items-center justify-center w-8 h-8 rounded-full bg-red-100 dark:bg-red-900/30"
					>
						<svg
							class="w-5 h-5 text-red-600 dark:text-red-400"
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
					</div>
				{/if}
				<div>
					<h3
						class="text-sm font-semibold {isProcessing
							? 'text-blue-900 dark:text-blue-100'
							: lastRunStatus === 'failed'
								? 'text-red-900 dark:text-red-100'
								: 'text-green-900 dark:text-green-100'}"
					>
						{#if lastRunStatus === 'pending'}
							Visualization Pending
						{:else if lastRunStatus === 'processing'}
							Generating Visualization...
						{:else if lastRunStatus === 'failed'}
							Visualization Failed
						{/if}
					</h3>
					<p
						class="text-xs {isProcessing
							? 'text-blue-700 dark:text-blue-300'
							: 'text-red-700 dark:text-red-300'}"
					>
						{#if isProcessing && lastRunAt}
							Started {formatTime(elapsedTime)} ago
						{/if}
						{#if processingRuns.length > 0}
							· {processingRuns.length} run{processingRuns.length !== 1 ? 's' : ''} in progress
						{/if}
					</p>
				</div>
			</div>

			<div class="flex items-center gap-3">
				{#if isProcessing}
					<span
						class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300"
					>
						<span class="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse"></span>
						{lastRunStatus === 'pending' ? 'Pending' : 'Processing'}
					</span>
				{/if}

				{#if !isProcessing && onDismiss}
					<button
						onclick={onDismiss}
						class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-1 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
						title="Dismiss"
					>
						<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M6 18L18 6M6 6l12 12"
							></path>
						</svg>
					</button>
				{/if}
			</div>
		</div>

		{#if lastError}
			<div class="mt-2 text-sm text-red-600 dark:text-red-400">
				<span class="font-medium">Error:</span>
				{lastError}
			</div>
		{/if}

		<!-- In-progress runs breakdown -->
		{#if processingRuns.length > 0}
			<div class="mt-3 space-y-2">
				{#each processingRuns as run (run.visualization_id)}
					<div
						class="flex items-center justify-between px-3 py-2 rounded-lg bg-white/50 dark:bg-gray-800/50"
					>
						<div class="flex items-center gap-2">
							<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
								Run #{run.visualization_id}
							</span>
							<Badge color={run.status === 'processing' ? 'blue' : 'yellow'} class="text-xs">
								{run.status === 'processing' ? 'Processing' : 'Pending'}
							</Badge>
						</div>
						<div class="text-xs text-gray-500 dark:text-gray-400">
							{#if (run.point_count ?? run.embedding_count ?? 0) > 0}
								{(run.point_count ?? run.embedding_count ?? 0).toLocaleString()} points
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
