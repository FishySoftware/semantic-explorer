<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateDatasetTransformModal from '../components/CreateDatasetTransformModal.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import type {
		DatasetTransform,
		DatasetTransformStats as Stats,
		Dataset,
		Embedder,
		PaginatedResponse,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		// eslint-disable-next-line no-unused-vars
		onViewTransform?: (id: number) => void;
	}

	let { onViewTransform }: Props = $props();

	let transforms = $state<DatasetTransform[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Pagination state
	let totalCount = $state(0);
	let currentPage = $state(1);
	let pageSize = $state(10);
	const pageSizeOptions = [10, 50, 100];

	// Sort state
	let sortBy = $state('created_at');
	let sortDirection = $state('desc');

	// Search state
	let searchQuery = $state('');
	let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

	// SSE batching for stats updates
	let sseUpdateQueue = new SvelteSet<number>();
	let sseUpdateTimer: ReturnType<typeof setTimeout> | null = null;

	// Modal state
	let showCreateModal = $state(false);

	let transformPendingDelete = $state<DatasetTransform | null>(null);

	// Selection state
	let selected = new SvelteSet<number>();
	let selectAll = $state(false);

	function toggleSelectAll() {
		selectAll = !selectAll;
		if (selectAll) {
			selected.clear();
			for (const t of transforms) {
				selected.add(t.dataset_transform_id);
			}
		} else {
			selected.clear();
		}
	}

	function toggleSelect(id: number) {
		if (selected.has(id)) {
			selected.delete(id);
			selectAll = false;
		} else {
			selected.add(id);
		}
	}

	async function bulkToggleEnabled(enable: boolean) {
		for (const id of selected) {
			const transform = transforms.find((t) => t.dataset_transform_id === id);
			if (transform) {
				await toggleEnabled(transform, enable, false);
			}
		}
		selected.clear();
		selectAll = false;
	}

	async function bulkTrigger() {
		for (const id of selected) {
			await triggerTransform(id);
		}
		selected.clear();
		selectAll = false;
	}

	async function bulkDelete() {
		for (const id of selected) {
			const transform = transforms.find((t) => t.dataset_transform_id === id);
			if (transform) {
				await requestDeleteTransform(transform, false);
			}
		}
		selected.clear();
		selectAll = false;
	}

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const offset = (currentPage - 1) * pageSize;
			const params = new SvelteURLSearchParams({
				limit: pageSize.toString(),
				offset: offset.toString(),
				sort_by: sortBy,
				sort_direction: sortDirection,
			});
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			const response = await fetch(`/api/dataset-transforms?${params}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch dataset transforms: ${response.statusText}`);
			}
			const data: PaginatedResponse<DatasetTransform> = await response.json();
			transforms = data.items;
			totalCount = data.total_count;

			// Fetch stats in batch for all transforms
			const transformIds = transforms.map((t) => t.dataset_transform_id);
			await fetchBatchStats(transformIds);
		} catch (e) {
			const message = formatError(e, 'Failed to fetch dataset transforms');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	function handleSort(field: string) {
		if (sortBy === field) {
			sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
		} else {
			sortBy = field;
			sortDirection = 'desc';
		}
		currentPage = 1;
		fetchTransforms();
	}

	function handleSearchInput() {
		if (searchDebounceTimer) {
			clearTimeout(searchDebounceTimer);
		}
		searchDebounceTimer = setTimeout(() => {
			currentPage = 1;
			fetchTransforms();
		}, 300);
	}

	function handlePageChange(newPage: number) {
		currentPage = newPage;
		fetchTransforms();
	}

	function handlePageSizeChange(newSize: number) {
		pageSize = newSize;
		currentPage = 1;
		fetchTransforms();
	}

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap;
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function fetchBatchStats(transformIds: number[]) {
		if (transformIds.length === 0) return;

		try {
			const response = await fetch('/api/dataset-transforms-batch-stats', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ dataset_transform_ids: transformIds }),
			});

			if (response.ok) {
				const batchStats: Record<number, any> = await response.json();
				for (const [idStr, stats] of Object.entries(batchStats)) {
					const id = parseInt(idStr, 10);
					statsMap.set(id, stats);
				}
				statsMap = statsMap; // Trigger reactivity
			} else {
				console.error('Failed to fetch batch stats:', await response.text());
			}
		} catch (e) {
			console.error('Failed to fetch batch stats:', e);
		}
	}

	function queueSSEStatsUpdate(transformId: number) {
		// Add to queue
		sseUpdateQueue.add(transformId);

		// Clear existing timer
		if (sseUpdateTimer) {
			clearTimeout(sseUpdateTimer);
		}

		// Batch updates: wait 100ms to collect multiple events, then fetch in batch
		sseUpdateTimer = setTimeout(() => {
			const idsToUpdate = Array.from(sseUpdateQueue);
			sseUpdateQueue.clear();

			if (idsToUpdate.length === 1) {
				// Single update - use individual endpoint
				fetchStatsForTransform(idsToUpdate[0]);
			} else {
				// Multiple updates - use batch endpoint
				fetchBatchStats(idsToUpdate);
			}
		}, 100);
	}

	async function fetchDatasets() {
		try {
			const response = await fetch('/api/datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch datasets: ${response.statusText}`);
			}
			const data = await response.json();
			datasets = data.items ?? [];
		} catch (e) {
			console.error('Failed to fetch datasets:', e);
		}
	}

	async function fetchEmbedders() {
		try {
			const response = await fetch('/api/embedders');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedders: ${response.statusText}`);
			}
			const data = await response.json();
			embedders = data.items || [];
		} catch (e) {
			console.error('Failed to fetch embedders:', e);
		}
	}

	async function toggleEnabled(transform: DatasetTransform, targetState: boolean, refresh = true) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transform.dataset_transform_id}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ is_enabled: targetState }),
			});

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const responseData = await response.json();
			const updated = responseData.transform || responseData;
			transforms = transforms.map((t) =>
				t.dataset_transform_id === updated.dataset_transform_id ? updated : t
			);

			toastStore.success(
				`Dataset transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
			if (refresh) {
				await fetchTransforms();
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle dataset transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ dataset_transform_id: transformId }),
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger transform: ${response.statusText}`);
			}

			toastStore.success('Dataset transform triggered successfully');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger dataset transform'));
		}
	}

	function requestDeleteTransform(transform: DatasetTransform, refresh = true) {
		transformPendingDelete = transform;
		(transformPendingDelete as any)._skipRefresh = !refresh;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) return;

		const target = transformPendingDelete;
		const skipRefresh = (target as any)._skipRefresh;
		transformPendingDelete = null;

		try {
			const response = await fetch(`/api/dataset-transforms/${target.dataset_transform_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete dataset transform: ${response.statusText}`);
			}

			if (!skipRefresh) {
				await fetchTransforms();
			}
			toastStore.success('Dataset transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete dataset transform'));
		}
	}

	function connectSSE() {
		// Close existing connection
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}

		try {
			eventSource = new EventSource('/api/dataset-transforms/stream', { withCredentials: true });

			eventSource.addEventListener('connected', () => {
				reconnectAttempts = 0;
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// Handle status update - refresh specific transform or trigger refetch
					if (statusUpdate.dataset_transform_id) {
						// Queue stats update for batching (reduces requests during high-frequency updates)
						queueSSEStatsUpdate(statusUpdate.dataset_transform_id);
					}
				} catch (e) {
					console.error('Failed to parse SSE status event:', e);
				}
			});

			eventSource.addEventListener('closed', () => {
				reconnectSSE();
			});

			eventSource.onerror = () => {
				eventSource?.close();
				eventSource = null;
				reconnectSSE();
			};
		} catch (e) {
			console.error('Failed to connect to SSE stream:', e);
			reconnectSSE();
		}
	}

	function reconnectSSE() {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
		}

		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error('Max SSE reconnection attempts reached');
			return;
		}

		// Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 64s... up to 60s max
		const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 60000);
		reconnectAttempts++;

		reconnectTimer = setTimeout(() => {
			connectSSE();
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
		await Promise.all([fetchTransforms(), fetchDatasets(), fetchEmbedders()]);

		// Connect to SSE stream for real-time updates
		connectSSE();
	});

	onDestroy(() => {
		disconnectSSE();
	});

	function getDataset(datasetId: number) {
		return datasets.find((d) => d.dataset_id === datasetId);
	}

	function getTotalPages(): number {
		if (totalCount <= 0 || pageSize <= 0) return 1;
		return Math.ceil(totalCount / pageSize);
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Dataset Transforms"
		description="Process Datasets with embedders to create Embedded Datasets. Each Dataset Transform can use multiple embedders, creating one Embedded Dataset per embedder."
	/>

	<div class="flex justify-between items-center mb-6">
		<Heading tag="h1" class="text-3xl font-bold">Dataset Transforms</Heading>
		<button
			onclick={() => (showCreateModal = true)}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			Create Dataset Transform
		</button>
	</div>

	<CreateDatasetTransformModal
		bind:open={showCreateModal}
		onSuccess={() => {
			// Redirect to embedded datasets page to monitor transform progress
			window.location.hash = '#/embedded-datasets';
		}}
	/>

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
			oninput={handleSearchInput}
			placeholder="Search dataset transforms..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading dataset transforms...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if transforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No dataset transforms found matching your search.'
					: 'No dataset transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		{#if selected.size > 0}
			<div
				class="mb-4 flex items-center gap-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4"
			>
				<span class="text-sm text-blue-700 dark:text-blue-300 flex-1">
					{selected.size} transform{selected.size !== 1 ? 's' : ''} selected
				</span>
				<button
					onclick={() => bulkToggleEnabled(true)}
					class="text-sm px-3 py-1 rounded bg-green-600 hover:bg-green-700 text-white transition-colors"
				>
					Enable
				</button>
				<button
					onclick={() => bulkToggleEnabled(false)}
					class="text-sm px-3 py-1 rounded bg-yellow-600 hover:bg-yellow-700 text-white transition-colors"
				>
					Disable
				</button>
				<button
					onclick={() => bulkTrigger()}
					class="text-sm px-3 py-1 rounded bg-blue-600 hover:bg-blue-700 text-white transition-colors"
				>
					Trigger
				</button>
				<button
					onclick={() => bulkDelete()}
					class="text-sm px-3 py-1 rounded bg-red-600 hover:bg-red-700 text-white transition-colors"
				>
					Delete
				</button>
				<button
					onclick={() => {
						selected.clear();
						selectAll = false;
					}}
					class="text-sm px-3 py-1 rounded bg-gray-300 hover:bg-gray-400 dark:bg-gray-600 dark:hover:bg-gray-500 text-gray-900 dark:text-white transition-colors"
				>
					Clear
				</button>
			</div>
		{/if}
		<div class="overflow-x-auto">
			<table
				class="dataset-transforms-table w-full text-sm text-left text-gray-600 dark:text-gray-400"
			>
				<thead class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
					<tr>
						<th class="px-4 py-3 w-12">
							<input
								type="checkbox"
								checked={selectAll}
								onchange={() => toggleSelectAll()}
								class="cursor-pointer"
							/>
						</th>
						<th class="px-4 py-3">
							<button
								type="button"
								onclick={() => handleSort('title')}
								class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
							>
								Title
								{#if sortBy === 'title'}
									{sortDirection === 'asc' ? '▲' : '▼'}
								{/if}
							</button>
						</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Source Dataset</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Embedders</th>
						<th class="px-4 py-3">
							<button
								type="button"
								onclick={() => handleSort('is_enabled')}
								class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
							>
								Status
								{#if sortBy === 'is_enabled'}
									{sortDirection === 'asc' ? '▲' : '▼'}
								{/if}
							</button>
						</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Chunks Embedded</th>
						<th class="px-4 py-3">
							<button
								type="button"
								onclick={() => handleSort('created_at')}
								class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
							>
								Created
								{#if sortBy === 'created_at'}
									{sortDirection === 'asc' ? '▲' : '▼'}
								{/if}
							</button>
						</th>
					</tr>
				</thead>
				<tbody>
					{#each transforms as transform (transform.dataset_transform_id)}
						{@const stats = statsMap.get(transform.dataset_transform_id)}
						{@const dataset = getDataset(transform.source_dataset_id)}
						<tr
							class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
						>
							<td class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(transform.dataset_transform_id)}
									onchange={() => toggleSelect(transform.dataset_transform_id)}
									class="cursor-pointer"
								/>
							</td>
							<td class="px-4 py-3 font-medium text-gray-900 dark:text-white">
								{#if onViewTransform}
									<button
										onclick={() => onViewTransform(transform.dataset_transform_id)}
										class="text-blue-600 dark:text-blue-400 hover:underline"
									>
										{transform.title}
									</button>
								{:else}
									{transform.title}
								{/if}
							</td>
							<td class="px-4 py-3 text-sm">
								{#if dataset}
									<a
										href="#/datasets/{transform.source_dataset_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{dataset.title}
									</a>
								{:else}
									Dataset {transform.source_dataset_id}
								{/if}
							</td>
							<td class="px-4 py-3 text-sm">
								{#if transform.embedder_ids && transform.embedder_ids.length > 0}
									<div class="flex flex-wrap gap-1">
										{#each transform.embedder_ids as embedderId (embedderId)}
											{@const embedder = embedders.find((e) => e.embedder_id === embedderId)}
											<a
												href="#/embedders/{embedderId}/details"
												class="inline-block text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
											>
												{embedder?.name || `Embedder ${embedderId}`}
											</a>
										{/each}
									</div>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">None</span>
								{/if}
							</td>
							<td class="px-4 py-3">
								<span
									class={transform.is_enabled
										? 'px-2 py-1 rounded-full text-xs font-semibold bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
										: 'px-2 py-1 rounded-full text-xs font-semibold bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'}
								>
									{transform.is_enabled ? 'Enabled' : 'Disabled'}
								</span>
							</td>
							<td class="px-4 py-3">
								{stats?.total_chunks_embedded ?? '-'}
							</td>
							<td class="px-4 py-3">
								{new Date(transform.created_at).toLocaleDateString()}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	<div class="mt-6 flex items-center justify-between">
		<div class="flex items-center">
			<div class="text-sm text-gray-600 dark:text-gray-400">
				Showing {(currentPage - 1) * pageSize + 1} to {Math.min(currentPage * pageSize, totalCount)} of
				{totalCount} transforms
			</div>
			<div class="ml-4 flex items-center gap-2">
				<label for="page-size" class="text-sm font-medium text-gray-700 dark:text-gray-300">
					Per page:
				</label>
				<select
					id="page-size"
					value={pageSize}
					onchange={(e) => handlePageSizeChange(Number(e.currentTarget.value))}
					class="px-2 py-1 pr-8 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-1 focus:ring-blue-500"
				>
					{#each pageSizeOptions as option (option)}
						<option value={option}>{option}</option>
					{/each}
				</select>
			</div>
		</div>
		<div class="flex gap-2">
			<button
				onclick={() => handlePageChange(currentPage - 1)}
				disabled={currentPage === 1}
				class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				Previous
			</button>
			<div class="flex items-center gap-1">
				{#each Array.from({ length: getTotalPages() }, (_, i) => i + 1) as page (page)}
					{#if page === 1 || page === getTotalPages() || (page >= currentPage - 1 && page <= currentPage + 1)}
						<button
							onclick={() => handlePageChange(page)}
							class={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
								currentPage === page
									? 'bg-blue-600 text-white'
									: 'border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
							}`}
						>
							{page}
						</button>
					{:else if page === currentPage - 2 || page === currentPage + 2}
						<span class="px-2 py-2 text-gray-500">...</span>
					{/if}
				{/each}
			</div>
			<button
				onclick={() => handlePageChange(currentPage + 1)}
				disabled={currentPage === getTotalPages()}
				class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				Next
			</button>
		</div>
	</div>
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete Dataset Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This will also delete associated Embedded Datasets.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>

<style>
	:global(.dataset-transforms-table :is(td, th)) {
		word-wrap: break-word;
		word-break: normal;
		white-space: normal;
		overflow-wrap: break-word;
	}
</style>
