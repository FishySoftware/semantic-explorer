<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface Props {
		datasetTransformId: number;
		onBack: () => void;
		onNavigate?: (_page: string, _params?: Record<string, unknown>) => void;
	}

	let { datasetTransformId, onBack, onNavigate }: Props = $props();

	interface DatasetTransform {
		dataset_transform_id: number;
		title: string;
		source_dataset_id: number;
		embedder_ids: number[];
		owner: string;
		is_enabled: boolean;
		job_config: any;
		created_at: string;
		updated_at: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
	}

	interface Stats {
		dataset_transform_id: number;
		embedder_count: number;
		total_batches_processed: number;
		successful_batches: number;
		failed_batches: number;
		processing_batches: number;
		total_chunks_embedded: number;
		total_chunks_processing: number;
		total_chunks_failed: number;
		total_chunks_to_process: number;
		status: string;
		is_processing: boolean;
		last_run_at: string | null;
		first_processing_at: string | null;
	}

	interface EmbedderStats {
		embedded_dataset_id: number;
		embedder_id: number;
		collection_name: string;
		title: string;
		total_batches_processed: number;
		successful_batches: number;
		failed_batches: number;
		processing_batches: number;
		total_chunks_embedded: number;
		total_chunks_failed: number;
		total_chunks_processing: number;
		last_run_at: string | null;
		first_processing_at: string | null;
		avg_processing_duration_ms: number | null;
		is_processing: boolean;
		error?: string;
	}

	interface DetailedStatsResponse {
		dataset_transform_id: number;
		title: string;
		aggregate: Stats;
		per_embedder: EmbedderStats[];
	}

	interface Batch {
		id: number;
		dataset_transform_id: number;
		batch_key: string;
		processed_at: string;
		status: string;
		chunk_count: number;
		error_message: string | null;
		processing_duration_ms: number | null;
		created_at: string;
		updated_at: string;
	}

	interface PaginatedBatchesResponse {
		items: Batch[];
		total_count: number;
		limit: number;
		offset: number;
	}

	let transform = $state<DatasetTransform | null>(null);
	let sourceDataset = $state<Dataset | null>(null);
	let embedders = $state<Embedder[]>([]);
	let stats = $state<Stats | null>(null);
	let embedderStats = $state<EmbedderStats[]>([]);
	let batches = $state<Batch[]>([]);
	let totalBatchesCount = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let retrying = $state(false);
	let showRetryConfirm = $state(false);

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete state
	let transformPendingDelete = $state<DatasetTransform | null>(null);
	let deleting = $state(false);

	// Pagination for batches
	let batchesCurrentPage = $state(1);
	let batchesPageSize = $state(10);

	// Sort state for batches
	let batchSortBy = $state('processed_at');
	let batchSortDirection = $state('desc');

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let isMounted = false; // Track if component is still mounted

	// Polling interval for auto-refresh
	let pollTimer: ReturnType<typeof setInterval> | null = null;
	const POLL_INTERVAL_MS = 5000;

	async function fetchTransform() {
		try {
			const response = await fetch(`/api/dataset-transforms/${datasetTransformId}`, {
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to fetch dataset transform: ${response.statusText}`);
			}

			transform = await response.json();

			// Fetch related resources
			if (transform?.source_dataset_id) {
				await fetchSourceDataset(transform.source_dataset_id);
			}
			if (transform?.embedder_ids?.length) {
				await fetchEmbedders(transform.embedder_ids);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Unknown error occurred';
			console.error('Error fetching dataset transform:', e);
		}
	}

	async function fetchSourceDataset(id: number) {
		try {
			const response = await fetch(`/api/datasets/${id}`, {
				credentials: 'include',
			});

			if (response.ok) {
				sourceDataset = await response.json();
			}
		} catch (e) {
			console.error('Error fetching source dataset:', e);
		}
	}

	async function fetchEmbedders(ids: number[]) {
		try {
			const response = await fetch('/api/embedders', {
				credentials: 'include',
			});

			if (response.ok) {
				const data = await response.json();
				const allEmbedders: Embedder[] = data.items || [];
				embedders = allEmbedders.filter((e) => ids.includes(e.embedder_id));
			}
		} catch (e) {
			console.error('Error fetching embedders:', e);
		}
	}

	async function retryFailedBatches() {
		retrying = true;
		showRetryConfirm = false;
		try {
			const response = await fetch(`/api/dataset-transforms/${datasetTransformId}/retry-failed`, {
				method: 'POST',
				credentials: 'include',
			});
			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to retry: ${errorText}`);
			}
			const result = await response.json();
			if (result.retried_count > 0) {
				toastStore.success(
					`${result.retried_count} batch job(s) re-submitted for processing.`,
					'Retry Started'
				);
			} else if (result.stats_reconciled) {
				toastStore.info('No failed batches found. Stats have been reconciled.', 'Stats Corrected');
			} else {
				toastStore.info('No failed batches to retry.', 'Nothing to Retry');
			}
			// Refresh stats and batches immediately
			await Promise.all([fetchDetailedStats(), fetchBatches()]);
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			toastStore.error(msg, 'Retry Failed');
			console.error('Error retrying failed batches:', e);
		} finally {
			retrying = false;
		}
	}

	async function fetchDetailedStats() {
		try {
			const response = await fetch(`/api/dataset-transforms/${datasetTransformId}/detailed-stats`, {
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to fetch detailed stats: ${response.statusText}`);
			}

			const data: DetailedStatsResponse = await response.json();
			stats = data.aggregate;
			embedderStats = data.per_embedder;
		} catch (e) {
			console.error('Error fetching detailed stats:', e);
		}
	}

	async function fetchBatches() {
		try {
			const offset = (batchesCurrentPage - 1) * batchesPageSize;
			const params = new URLSearchParams({
				limit: batchesPageSize.toString(),
				offset: offset.toString(),
				sort_by: batchSortBy,
				sort_direction: batchSortDirection,
			});
			const response = await fetch(
				`/api/dataset-transforms/${datasetTransformId}/batches?${params}`,
				{
					credentials: 'include',
				}
			);

			if (!response.ok) {
				console.error(`Failed to fetch batches: ${response.statusText}`);
				return;
			}

			const data: PaginatedBatchesResponse = await response.json();
			batches = data.items ?? [];
			totalBatchesCount = data.total_count ?? 0;
		} catch (e) {
			console.error('Error fetching batches:', e);
		}
	}

	function getBatchesTotalPages(): number {
		if (totalBatchesCount <= 0 || batchesPageSize <= 0) return 1;
		return Math.ceil(totalBatchesCount / batchesPageSize);
	}

	function handleBatchesPageChange(page: number) {
		if (page < 1 || page > getBatchesTotalPages()) return;
		batchesCurrentPage = page;
		fetchBatches();
	}

	function handleBatchSort(field: string) {
		if (batchSortBy === field) {
			batchSortDirection = batchSortDirection === 'asc' ? 'desc' : 'asc';
		} else {
			batchSortBy = field;
			batchSortDirection = 'desc';
		}
		batchesCurrentPage = 1;
		fetchBatches();
	}

	function startEdit() {
		if (!transform) return;
		editMode = true;
		editTitle = transform.title;
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editTitle = '';
		editError = null;
	}

	async function saveEdit() {
		if (!transform) return;
		if (!editTitle.trim()) {
			editError = 'Title is required';
			return;
		}

		try {
			saving = true;
			editError = null;
			const response = await fetch(`/api/dataset-transforms/${datasetTransformId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ title: editTitle.trim() }),
			});

			if (!response.ok) {
				throw new Error(`Failed to update transform: ${response.statusText}`);
			}

			const responseData = await response.json();
			transform = responseData.transform || responseData;
			editMode = false;
			toastStore.success('Dataset transform updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update dataset transform');
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function toggleEnabled() {
		if (!transform) return;

		try {
			const response = await fetch(`/api/dataset-transforms/${datasetTransformId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ is_enabled: !transform.is_enabled }),
			});

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const responseData = await response.json();
			const updated = responseData.transform || responseData;
			transform = updated;
			toastStore.success(
				`Dataset transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle dataset transform'));
		}
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) return;

		transformPendingDelete = null;

		try {
			deleting = true;
			const response = await fetch(`/api/dataset-transforms/${datasetTransformId}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete transform: ${response.statusText}`);
			}

			toastStore.success('Dataset transform deleted');
			onBack();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete dataset transform'));
		} finally {
			deleting = false;
		}
	}

	function connectSSE() {
		// Close existing connection first
		disconnectSSE();

		try {
			eventSource = new EventSource('/api/dataset-transforms/stream');

			eventSource.addEventListener('heartbeat', () => {
				// Keep connection alive
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// If this is an update for our transform, refresh stats and batches
					// API sends transform_id (generic) not dataset_transform_id
					if (statusUpdate.transform_id === datasetTransformId) {
						fetchDetailedStats();
						fetchBatches();
					}
				} catch (e) {
					console.error('Failed to parse SSE status event:', e);
				}
			});

			eventSource.onerror = () => {
				eventSource?.close();
				eventSource = null;
				reconnectSSE();
			};

			reconnectAttempts = 0;
		} catch (e) {
			console.error('Failed to connect to SSE stream:', e);
			reconnectSSE();
		}
	}

	function reconnectSSE() {
		if (!isMounted) {
			// Component has been unmounted, don't attempt reconnection
			return;
		}

		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error('Max SSE reconnection attempts reached');
			return;
		}

		const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 60000);
		reconnectAttempts++;

		reconnectTimer = setTimeout(() => {
			if (isMounted) {
				connectSSE();
			}
		}, delay);
	}

	function disconnectSSE() {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}
		reconnectAttempts = 0;
	}

	onMount(async () => {
		isMounted = true;
		loading = true;
		await Promise.all([fetchTransform(), fetchDetailedStats(), fetchBatches()]);
		loading = false;
		connectSSE();

		// Auto-refresh stats and batches every 5 seconds
		pollTimer = setInterval(() => {
			if (isMounted) {
				fetchDetailedStats();
				fetchBatches();
			}
		}, POLL_INTERVAL_MS);
	});

	onDestroy(() => {
		isMounted = false;
		disconnectSSE();
		if (pollTimer) {
			clearInterval(pollTimer);
			pollTimer = null;
		}
	});
</script>

<div class="mx-auto">
	<PageHeader
		title="Dataset Transform Details"
		description="View detailed information, embedding progress, and statistics for this dataset transform."
	/>

	<div class="mb-6">
		<button
			onclick={onBack}
			class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white transition-colors flex items-center gap-2"
		>
			← Back to Dataset Transforms
		</button>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading dataset transform details...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if transform}
		<!-- Transform Info Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<div class="flex justify-between items-start mb-4">
				<div class="flex-1">
					{#if editMode}
						<form
							onsubmit={(e) => {
								e.preventDefault();
								saveEdit();
							}}
							class="flex items-center gap-2 mb-2"
						>
							<input
								type="text"
								bind:value={editTitle}
								placeholder="Enter transform title"
								class="text-2xl font-bold px-3 py-1 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white flex-1"
								required
							/>
							<button
								type="submit"
								disabled={saving}
								class="px-3 py-1.5 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{saving ? 'Saving...' : 'Save'}
							</button>
							<button
								type="button"
								onclick={cancelEdit}
								class="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
							>
								Cancel
							</button>
						</form>
						{#if editError}
							<p class="text-sm text-red-600 dark:text-red-400 mt-1">{editError}</p>
						{/if}
					{:else}
						<div class="flex items-baseline gap-3 mb-2">
							<Heading tag="h2" class="text-2xl font-bold">{transform.title}</Heading>
							<span class="text-sm text-gray-500 dark:text-gray-400"
								>#{transform.dataset_transform_id}</span
							>
						</div>
					{/if}
					<p class="text-sm text-gray-500 dark:text-gray-400">
						Created {formatDate(transform.created_at)}
						{#if transform.updated_at && transform.updated_at !== transform.created_at}
							&middot; Updated {formatDate(transform.updated_at)}
						{/if}
					</p>
				</div>
				<div class="flex items-center gap-2 ml-4">
					{#if !editMode}
						<button
							onclick={startEdit}
							title="Edit title"
							class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300 transition-colors"
						>
							Edit
						</button>
					{/if}
					<button
						onclick={toggleEnabled}
						title={transform.is_enabled ? 'Disable transform' : 'Enable transform'}
						class={transform.is_enabled
							? 'px-3 py-1 text-sm rounded-lg bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400 transition-colors'
							: 'px-3 py-1 text-sm rounded-lg bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400 transition-colors'}
					>
						{transform.is_enabled ? 'Disable' : 'Enable'}
					</button>
					<button
						onclick={() => (transformPendingDelete = transform)}
						disabled={deleting}
						title="Delete transform"
						class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
					>
						{deleting ? 'Deleting...' : 'Delete'}
					</button>
				</div>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Source Dataset</p>
					{#if sourceDataset}
						<button
							onclick={() =>
								onNavigate?.('dataset-detail', { datasetId: transform?.source_dataset_id })}
							class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
						>
							{sourceDataset.title}
						</button>
					{:else}
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							ID: {transform.source_dataset_id}
						</p>
					{/if}
				</div>
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Embedders</p>
					{#if embedders.length > 0}
						<div class="flex flex-wrap gap-2">
							{#each embedders as embedder (embedder.embedder_id)}
								<button
									onclick={() =>
										onNavigate?.('embedder-detail', { embedderId: embedder.embedder_id })}
									class="text-sm font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
								>
									{embedder.name}
								</button>
								{#if embedders.indexOf(embedder) < embedders.length - 1}
									<span class="text-gray-400">,</span>
								{/if}
							{/each}
						</div>
					{:else}
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							{transform.embedder_ids?.join(', ') || 'None'}
						</p>
					{/if}
				</div>
			</div>
		</div>

		<!-- Job Configuration Card -->
		{#if transform.job_config && Object.keys(transform.job_config).length > 0}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<Heading tag="h3" class="text-lg font-bold mb-4">Job Configuration</Heading>
				<div class="space-y-4">
					{#each Object.entries(transform.job_config) as [key, value] (key)}
						<div>
							<h4 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-1">{key}</h4>
							{#if typeof value === 'object'}
								<pre
									class="text-sm font-mono bg-gray-50 dark:bg-gray-900 rounded-lg p-3 overflow-x-auto text-gray-900 dark:text-gray-100">{JSON.stringify(
										value,
										null,
										2
									)}</pre>
							{:else}
								<p class="text-sm font-medium text-gray-900 dark:text-white">{value}</p>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Aggregate Stats Card -->
		{#if stats}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<div class="flex items-center justify-between mb-4">
					<Heading tag="h3" class="text-xl font-bold">Aggregate Embedding Statistics</Heading>
					<div class="flex items-center gap-2">
						{#if stats.failed_batches > 0 && !stats.is_processing}
							<button
								onclick={() => (showRetryConfirm = true)}
								disabled={retrying}
								class="inline-flex items-center gap-1.5 px-3 py-1 text-sm font-medium rounded-full bg-orange-100 text-orange-700 hover:bg-orange-200 dark:bg-orange-900/20 dark:text-orange-400 dark:hover:bg-orange-900/40 disabled:opacity-50 transition-colors"
							>
								{#if retrying}
									<svg class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
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
											d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
										></path>
									</svg>
									Retrying…
								{:else}
									<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
										/>
									</svg>
									Retry Failed ({stats.failed_batches})
								{/if}
							</button>
						{/if}
						<span
							class={`px-3 py-1 rounded-full text-sm font-semibold ${
								stats.is_processing
									? 'bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400'
									: stats.status === 'completed'
										? 'bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
										: stats.status === 'completed_with_errors'
											? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/20 dark:text-yellow-400'
											: stats.status === 'failed'
												? 'bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400'
												: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'
							}`}
						>
							{stats.is_processing
								? 'Processing'
								: stats.status === 'completed_with_errors'
									? 'Completed with Errors'
									: stats.status.charAt(0).toUpperCase() + stats.status.slice(1)}
						</span>
					</div>
				</div>
				<div class="grid grid-cols-2 md:grid-cols-5 gap-4">
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Total Chunks Embedded</p>
						<p class="text-2xl font-bold text-blue-600 dark:text-blue-400">
							{stats.total_chunks_embedded}
						</p>
					</div>
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Processing</p>
						<p class="text-2xl font-bold text-yellow-600 dark:text-yellow-400">
							{stats.total_chunks_processing}
						</p>
					</div>
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Failed</p>
						<p class="text-2xl font-bold text-red-600 dark:text-red-400">
							{stats.total_chunks_failed}
						</p>
					</div>
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Remaining</p>
						<p class="text-2xl font-bold text-purple-600 dark:text-purple-400">
							{Math.max(
								0,
								stats.total_chunks_to_process -
									stats.total_chunks_embedded -
									stats.total_chunks_failed
							)}
						</p>
					</div>
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Embedders</p>
						<p class="text-2xl font-bold text-indigo-600 dark:text-indigo-400">
							{stats.embedder_count}
						</p>
					</div>
				</div>
				{#if stats.last_run_at}
					<p class="text-xs text-gray-500 dark:text-gray-400 mt-4">
						Last run: {formatDate(stats.last_run_at)}
					</p>
				{/if}
			</div>
		{/if}

		<!-- Per-Embedder Stats Table -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
			<Heading tag="h3" class="text-xl font-bold mb-4">Per-Embedder Breakdown</Heading>

			{#if embedderStats.length === 0}
				<p class="text-center text-gray-500 dark:text-gray-400 py-8">
					No embedder statistics available yet.
				</p>
			{:else}
				<div class="space-y-4">
					{#each embedderStats as stat (stat.embedded_dataset_id)}
						<div class="border border-gray-200 dark:border-gray-700 rounded-lg p-4">
							<div class="flex items-start justify-between mb-3">
								<div>
									<h4 class="font-semibold text-gray-900 dark:text-white">{stat.title}</h4>
									<p class="text-xs text-gray-500 dark:text-gray-400">
										Collection: {stat.collection_name}
									</p>
								</div>
								{#if stat.error}
									<span
										class="px-2 py-1 rounded-full text-xs font-semibold bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400"
									>
										Error
									</span>
								{:else}
									<span
										class={`px-2 py-1 rounded-full text-xs font-semibold ${
											stat.is_processing
												? 'bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400'
												: 'bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
										}`}
									>
										{stat.is_processing ? 'Processing' : 'Idle'}
									</span>
								{/if}
							</div>

							{#if stat.error}
								<p class="text-sm text-red-600 dark:text-red-400">{stat.error}</p>
							{:else}
								<div class="grid grid-cols-2 md:grid-cols-4 gap-3 text-sm">
									<div>
										<p class="text-gray-500 dark:text-gray-400 mb-1">Chunks Embedded</p>
										<p class="font-semibold text-gray-900 dark:text-white">
											{stat.total_chunks_embedded}
										</p>
									</div>
									<div>
										<p class="text-gray-500 dark:text-gray-400 mb-1">Processing</p>
										<p class="font-semibold text-gray-900 dark:text-white">
											{stat.total_chunks_processing}
										</p>
									</div>
									<div>
										<p class="text-gray-500 dark:text-gray-400 mb-1">Failed</p>
										<p class="font-semibold text-gray-900 dark:text-white">
											{stat.total_chunks_failed}
										</p>
									</div>
									<div>
										<p class="text-gray-500 dark:text-gray-400 mb-1">Batch Status</p>
										<p class="font-semibold text-gray-900 dark:text-white">
											{stat.successful_batches}/{stat.total_batches_processed}
										</p>
									</div>
								</div>
								{#if stat.avg_processing_duration_ms}
									<p class="text-xs text-gray-500 dark:text-gray-400 mt-2">
										Avg duration: {stat.avg_processing_duration_ms}ms
									</p>
								{/if}
								{#if stat.last_run_at}
									<p class="text-xs text-gray-500 dark:text-gray-400">
										Last run: {formatDate(stat.last_run_at)}
									</p>
								{/if}
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Recent Processing Batches -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
			<Heading tag="h3" class="text-xl font-bold mb-4">Recent Processing Batches</Heading>

			{#if batches.length === 0}
				<p class="text-center text-gray-500 dark:text-gray-400 py-8">
					No batches have been processed yet.
				</p>
			{:else}
				<div class="overflow-x-auto">
					<table class="w-full text-sm text-left text-gray-600 dark:text-gray-400">
						<thead
							class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700"
						>
							<tr>
								<th class="px-4 py-3">
									<button
										type="button"
										onclick={() => handleBatchSort('batch_key')}
										class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
									>
										Batch Key
										{#if batchSortBy === 'batch_key'}
											{batchSortDirection === 'asc' ? '▲' : '▼'}
										{/if}
									</button>
								</th>
								<th class="px-4 py-3">
									<button
										type="button"
										onclick={() => handleBatchSort('status')}
										class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
									>
										Status
										{#if batchSortBy === 'status'}
											{batchSortDirection === 'asc' ? '▲' : '▼'}
										{/if}
									</button>
								</th>
								<th class="px-4 py-3">
									<button
										type="button"
										onclick={() => handleBatchSort('chunk_count')}
										class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
									>
										Chunks
										{#if batchSortBy === 'chunk_count'}
											{batchSortDirection === 'asc' ? '▲' : '▼'}
										{/if}
									</button>
								</th>
								<th class="px-4 py-3">
									<button
										type="button"
										onclick={() => handleBatchSort('processing_duration_ms')}
										class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
									>
										Duration
										{#if batchSortBy === 'processing_duration_ms'}
											{batchSortDirection === 'asc' ? '▲' : '▼'}
										{/if}
									</button>
								</th>
								<th class="px-4 py-3">
									<button
										type="button"
										onclick={() => handleBatchSort('processed_at')}
										class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
									>
										Processed At
										{#if batchSortBy === 'processed_at'}
											{batchSortDirection === 'asc' ? '▲' : '▼'}
										{/if}
									</button>
								</th>
							</tr>
						</thead>
						<tbody>
							{#each batches as batch (batch.id)}
								<tr
									class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
								>
									<td class="px-4 py-3 font-medium text-gray-900 dark:text-white"
										>{batch.batch_key}</td
									>
									<td class="px-4 py-3">
										<span
											class={`px-2 py-1 rounded-full text-xs font-semibold ${
												batch.status === 'success'
													? 'bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
													: batch.status === 'failed'
														? 'bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400'
														: batch.status === 'processing'
															? 'bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400'
															: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'
											}`}
										>
											{batch.status}
										</span>
									</td>
									<td class="px-4 py-3">{batch.chunk_count}</td>
									<td class="px-4 py-3">
										{batch.processing_duration_ms ? `${batch.processing_duration_ms}ms` : '-'}
									</td>
									<td class="px-4 py-3">{formatDate(batch.processed_at)}</td>
								</tr>
								{#if batch.error_message}
									<tr
										class="bg-red-50 dark:bg-red-900/10 border-b border-gray-200 dark:border-gray-700"
									>
										<td colspan="5" class="px-4 py-2 text-xs text-red-600 dark:text-red-400">
											Error: {batch.error_message}
										</td>
									</tr>
								{/if}
							{/each}
						</tbody>
					</table>
				</div>

				<!-- Pagination -->
				{#if getBatchesTotalPages() > 1}
					<div class="mt-4 flex items-center justify-between">
						<div class="text-sm text-gray-600 dark:text-gray-400">
							Showing {(batchesCurrentPage - 1) * batchesPageSize + 1} to {Math.min(
								batchesCurrentPage * batchesPageSize,
								totalBatchesCount
							)} of {totalBatchesCount} batches
						</div>
						<div class="flex gap-2">
							<button
								onclick={() => handleBatchesPageChange(batchesCurrentPage - 1)}
								disabled={batchesCurrentPage === 1}
								class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								Previous
							</button>
							<div class="flex items-center gap-1">
								{#each Array.from({ length: getBatchesTotalPages() }, (_, i) => i + 1) as page (page)}
									{#if page === 1 || page === getBatchesTotalPages() || (page >= batchesCurrentPage - 1 && page <= batchesCurrentPage + 1)}
										<button
											onclick={() => handleBatchesPageChange(page)}
											class={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
												batchesCurrentPage === page
													? 'bg-blue-600 text-white'
													: 'border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
											}`}
										>
											{page}
										</button>
									{:else if page === batchesCurrentPage - 2 || page === batchesCurrentPage + 2}
										<span class="px-2 py-2 text-gray-500">...</span>
									{/if}
								{/each}
							</div>
							<button
								onclick={() => handleBatchesPageChange(batchesCurrentPage + 1)}
								disabled={batchesCurrentPage === getBatchesTotalPages()}
								class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								Next
							</button>
						</div>
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete Dataset Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This will also delete associated Qdrant collections. This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteTransform}
	onCancel={() => (transformPendingDelete = null)}
/>

<!-- Retry Confirmation Dialog -->
{#if showRetryConfirm}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 max-w-md mx-4">
			<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Retry Failed Batches</h3>
			<p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
				This will retry {stats?.failed_batches} failed
				{stats?.failed_batches === 1 ? 'batch' : 'batches'}. Failed batch files still in storage
				will be re-submitted for embedding.
			</p>
			<div class="flex justify-end gap-3">
				<button
					onclick={() => (showRetryConfirm = false)}
					class="px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
				>
					Cancel
				</button>
				<button
					onclick={retryFailedBatches}
					class="px-4 py-2 text-sm font-medium rounded-lg bg-orange-600 text-white hover:bg-orange-700"
				>
					Retry Failed Batches
				</button>
			</div>
		</div>
	</div>
{/if}
