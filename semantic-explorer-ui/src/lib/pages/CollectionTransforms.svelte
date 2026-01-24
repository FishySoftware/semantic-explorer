<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateCollectionTransformModal from '../components/CreateCollectionTransformModal.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import type {
		CollectionTransform,
		CollectionTransformStats as Stats,
		Collection,
		Dataset,
		PaginatedResponse,
		ProcessedFile,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface Props {
		// eslint-disable-next-line no-unused-vars
		onViewTransform?: (id: number) => void;
	}

	let { onViewTransform }: Props = $props();

	let transforms = $state<CollectionTransform[]>([]);
	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
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

	// Failed files modal state
	let showFailedFilesModal = $state(false);
	let failedFilesTransformTitle = $state('');
	let failedFiles = $state<ProcessedFile[]>([]);
	let loadingFailedFiles = $state(false);

	let searchQuery = $state('');
	let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let isMounted = false; // Track if component is still mounted

	// SSE batching for stats updates
	let sseUpdateQueue = new SvelteSet<number>();
	let sseUpdateTimer: ReturnType<typeof setTimeout> | null = null;

	// Modal state
	let showCreateModal = $state(false);

	let transformPendingDelete = $state<CollectionTransform | null>(null);

	// Selection state
	// eslint-disable-next-line svelte/no-unnecessary-state-wrap
	let selected = $state(new SvelteSet<number>());
	let selectAll = $state(false);

	function toggleSelectAll() {
		selectAll = !selectAll;
		if (selectAll) {
			selected.clear();
			for (const t of transforms) {
				selected.add(t.collection_transform_id);
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

	async function bulkToggleEnabled(_enable: boolean) {
		for (const id of selected) {
			const transform = transforms.find((t) => t.collection_transform_id === id);
			if (transform) {
				await toggleEnabled(transform, _enable, false);
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
			const transform = transforms.find((t) => t.collection_transform_id === id);
			if (transform) {
				await requestDeleteTransform(transform, false);
			}
		}
		selected = new SvelteSet();
		selectAll = false;
	}

	function openEditForm(_transform: CollectionTransform) {
		// Opens the modal for editing - implementation depends on your modal setup
		// For now, just trigger the create modal with the transform pre-populated
		showCreateModal = true;
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
			const response = await fetch(`/api/collection-transforms?${params}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch collection transforms: ${response.statusText}`);
			}
			const data: PaginatedResponse<CollectionTransform> = await response.json();
			transforms = data.items;
			totalCount = data.total_count;

			// Fetch stats in batch for all transforms
			const transformIds = transforms.map((t) => t.collection_transform_id);
			await fetchBatchStats(transformIds);
		} catch (e) {
			const message = formatError(e, 'Failed to fetch collection transforms');
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
			currentPage = 1; // Reset to first page on new search
			fetchTransforms();
		}, 300);
	}

	function handlePageChange(newPage: number) {
		currentPage = newPage;
		fetchTransforms();
	}

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function fetchBatchStats(transformIds: number[]) {
		if (transformIds.length === 0) return;

		try {
			const response = await fetch('/api/collection-transforms/batch-stats', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ collection_transform_ids: transformIds }),
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

	function closeFailedFilesModal() {
		showFailedFilesModal = false;
		failedFilesTransformTitle = '';
		failedFiles = [];
	}

	async function fetchCollections() {
		try {
			const response = await fetch('/api/collections');
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			const data = await response.json();
			collections = data.collections ?? [];
		} catch (e) {
			console.error('Failed to fetch collections:', e);
		}
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

	async function toggleEnabled(
		transform: CollectionTransform,
		targetState: boolean,
		refresh = true
	) {
		try {
			const response = await fetch(
				`/api/collection-transforms/${transform.collection_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: targetState,
					}),
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const updated = await response.json();
			transforms = transforms.map((t) =>
				t.collection_transform_id === updated.collection_transform_id ? updated : t
			);

			toastStore.success(
				`Collection transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
			if (refresh) {
				await fetchTransforms();
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle collection transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({ collection_transform_id: transformId }),
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger transform: ${response.statusText}`);
			}

			toastStore.success('Collection transform triggered successfully');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger collection transform'));
		}
	}

	function requestDeleteTransform(transform: CollectionTransform, refresh = true) {
		transformPendingDelete = transform;
		// Store refresh preference
		(transformPendingDelete as any)._skipRefresh = !refresh;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		const skipRefresh = (target as any)._skipRefresh;
		transformPendingDelete = null;

		try {
			const response = await fetch(`/api/collection-transforms/${target.collection_transform_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete collection transform: ${response.statusText}`);
			}

			transforms = transforms.filter(
				(t) => t.collection_transform_id !== target.collection_transform_id
			);
			toastStore.success('Collection transform deleted');
			if (!skipRefresh) {
				await fetchTransforms();
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete collection transform'));
		}
	}

	function connectSSE() {
		// Close existing connection
		disconnectSSE();

		try {
			eventSource = new EventSource('/api/collection-transforms/stream');

			eventSource.addEventListener('connected', () => {
				reconnectAttempts = 0;
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// Handle status update - refresh specific transform or trigger refetch
					if (statusUpdate.collection_transform_id) {
						// Queue stats update for batching (reduces requests during high-frequency updates)
						queueSSEStatsUpdate(statusUpdate.collection_transform_id);
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
		if (!isMounted) {
			// Component has been unmounted, don't attempt reconnection
			return;
		}

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
		await Promise.all([fetchTransforms(), fetchCollections(), fetchDatasets()]);

		// Connect to SSE stream for real-time updates
		connectSSE();

		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);

			const action = urlParams.get('action');
			const collectionIdParam = urlParams.get('collection_id');

			if (action === 'create' && collectionIdParam) {
				const collectionId = parseInt(collectionIdParam, 10);
				if (!isNaN(collectionId)) {
					showCreateModal = true;
				}
			}

			const basePath = hashParts[0];
			window.history.replaceState(
				null,
				'',
				window.location.pathname + window.location.search + basePath
			);
		}
	});

	onDestroy(() => {
		isMounted = false;
		disconnectSSE();
	});

	function getCollectionTitle(collectionId: number): string {
		const collection = collections.find((c) => c.collection_id === collectionId);
		return collection ? collection.title : `Collection ${collectionId}`;
	}

	function getDatasetTitle(datasetId: number): string {
		const dataset = datasets.find((d) => d.dataset_id === datasetId);
		return dataset ? dataset.title : `Dataset ${datasetId}`;
	}

	function getTotalPages(): number {
		if (totalCount <= 0 || pageSize <= 0) return 1;
		return Math.ceil(totalCount / pageSize);
	}

	function handlePageSizeChange(newSize: number) {
		pageSize = newSize;
		currentPage = 1; // Reset to first page when changing page size
		fetchTransforms();
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Collection Transforms"
		description="Process files from Collections into Dataset items. Collection transforms extract text from files, chunk them into manageable pieces, and create Dataset items ready for embedding."
	/>

	<div class="flex justify-between items-center mb-6">
		<Heading tag="h1" class="text-3xl font-bold">Collection Transforms</Heading>
		<button
			onclick={() => (showCreateModal = true)}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			Create Collection Transform
		</button>
	</div>

	<CreateCollectionTransformModal
		bind:open={showCreateModal}
		onSuccess={() => {
			// Redirect to datasets page to monitor transform progress
			window.location.hash = '#/datasets';
		}}
	/>

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
			oninput={handleSearchInput}
			placeholder="Search collection transforms..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading collection transforms...</p>
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
					? 'No collection transforms found matching your filter.'
					: 'No collection transforms yet. Create one to get started!'}
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
				class="collection-transforms-table w-full text-sm text-left text-gray-600 dark:text-gray-400"
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
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Collection</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Dataset</th>
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
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Files Processed</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Items Created</th>
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
						<th class="px-4 py-3 w-12 text-center font-semibold text-gray-900 dark:text-white"
							>Edit</th
						>
					</tr>
				</thead>
				<tbody>
					{#each transforms as transform (transform.collection_transform_id)}
						{@const stats = statsMap.get(transform.collection_transform_id)}
						<tr
							class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
						>
							<td class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(transform.collection_transform_id)}
									onchange={() => toggleSelect(transform.collection_transform_id)}
									class="cursor-pointer"
								/>
							</td>
							<td class="px-4 py-3 font-medium text-gray-900 dark:text-white">
								{#if onViewTransform}
									<button
										onclick={() => onViewTransform(transform.collection_transform_id)}
										class="text-blue-600 dark:text-blue-400 hover:underline"
									>
										{transform.dataset_title}
									</button>
								{:else}
									{transform.dataset_title}
								{/if}
							</td>
							<td class="px-4 py-3 text-sm">
								<a
									href="#/collections/{transform.collection_id}/details"
									class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
								>
									{getCollectionTitle(transform.collection_id)}
								</a>
							</td>
							<td class="px-4 py-3 text-sm">
								<a
									href="#/datasets/{transform.dataset_id}/details"
									class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
								>
									{getDatasetTitle(transform.dataset_id)}
								</a>
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
								{stats?.total_files_processed ?? '-'}
							</td>
							<td class="px-4 py-3">
								{stats?.total_chunks_created ?? '-'}
							</td>
							<td class="px-4 py-3">
								{formatDate(transform.created_at, false)}
							</td>
							<td class="px-4 py-3 text-center">
								<button
									type="button"
									onclick={() => openEditForm(transform)}
									title="Edit"
									class="text-gray-600 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-300 transition-colors"
								>
									✎
								</button>
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
	message={transformPendingDelete
		? `Are you sure you want to delete the transform "${transformPendingDelete.collection_title} > ${transformPendingDelete.dataset_title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteTransform}
	onCancel={() => (transformPendingDelete = null)}
/>

<!-- Failed Files Modal -->
{#if showFailedFilesModal}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
		<div
			class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-4xl w-full mx-4 max-h-[80vh] flex flex-col"
		>
			<div
				class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center"
			>
				<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
					Failed Files - {failedFilesTransformTitle}
				</h3>
				<button
					onclick={closeFailedFilesModal}
					class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
					aria-label="Close modal"
				>
					<svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M6 18L18 6M6 6l12 12"
						/>
					</svg>
				</button>
			</div>

			<div class="p-6 overflow-y-auto flex-1">
				{#if loadingFailedFiles}
					<div class="flex justify-center py-8">
						<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
					</div>
				{:else if failedFiles.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-center py-8">No failed files found.</p>
				{:else}
					<div class="space-y-4">
						{#each failedFiles as file (file.file_key)}
							<div
								class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
							>
								<div class="flex items-start justify-between">
									<div class="flex-1 min-w-0">
										<p
											class="font-mono text-sm text-gray-900 dark:text-white truncate"
											title={file.file_key}
										>
											{file.file_key.split('/').pop() || file.file_key}
										</p>
										<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
											Processed: {new Date(file.processed_at).toLocaleString()}
										</p>
									</div>
								</div>
								{#if file.process_error}
									<div class="mt-3 bg-red-100 dark:bg-red-900/40 rounded p-3">
										<p class="text-xs font-semibold text-red-700 dark:text-red-300 mb-1">Error:</p>
										<pre
											class="text-xs text-red-600 dark:text-red-400 whitespace-pre-wrap wrap-break-words font-mono">{file.process_error}</pre>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
				<button
					onclick={closeFailedFilesModal}
					class="px-4 py-2 bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
				>
					Close
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	:global(.collection-transforms-table :is(td, th)) {
		word-wrap: break-word;
		word-break: normal;
		white-space: normal;
		overflow-wrap: break-word;
	}
</style>
