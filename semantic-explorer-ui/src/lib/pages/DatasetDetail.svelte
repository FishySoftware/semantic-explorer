<script lang="ts">
	import { ArrowLeftOutline, ExpandOutline } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import ApiIntegrationModal from '../components/ApiIntegrationModal.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateDatasetTransformModal from '../components/CreateDatasetTransformModal.svelte';
	import DatasetTransformProgressPanel from '../components/DatasetTransformProgressPanel.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import TransformsList from '../components/TransformsList.svelte';
	import type {
		CollectionTransform,
		Dataset,
		DatasetItemChunks,
		DatasetItemSummary,
		DatasetTransform,
		EmbeddedDataset,
		PaginatedItems,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { createPollingInterval } from '../utils/polling';
	import { createSSEConnection, type SSEConnection } from '../utils/sse';
	import { formatDate } from '../utils/ui-helpers';

	interface Props {
		datasetId: number;
		onBack: () => void;
	}

	let { datasetId, onBack }: Props = $props();

	let dataset = $state<Dataset | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let collectionTransforms = $state<CollectionTransform[]>([]);
	let datasetTransforms = $state<DatasetTransform[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let transformsLoading = $state(false);
	let collectionTransformStatsMap = $state<Map<number, any>>(new Map());
	let datasetTransformStatsMap = $state<Map<number, any>>(new Map());

	let paginatedItems = $state<PaginatedItems | null>(null);
	let itemsLoading = $state(false);
	let itemsError = $state<string | null>(null);
	let currentPage = $state(0);
	let pageSize = $state(10);
	let expandedItemId = $state<number | null>(null);

	// Cache for loaded chunks keyed by item_id
	let chunksCache = $state<Record<number, DatasetItemChunks>>({});
	let loadingChunksItemId = $state<number | null>(null);

	// Chunk pagination state
	let chunkPageSize = $state(5);
	let chunkCurrentPages = $state<Record<number, number>>({});

	// Tab state
	let activeTab = $state('overview');
	let apiIntegrationModalOpen = $state(false);

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let editDetails = $state('');
	let editTags = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	const tabs = [
		{ id: 'overview', label: 'Overview', icon: '📋' },
		{ id: 'transforms', label: 'Transforms', icon: '🔄' },
		{ id: 'embeddings', label: 'Embeddings', icon: '🧬' },
	];

	// Dataset Transform Progress state
	let activeTransformProgress = $state<{
		id: number;
		title: string;
		startedAt: string;
		embedders: number[];
	} | null>(null);
	let transformProgressStats = $state<Record<string, any> | null>(null);
	let transformProgressPollingController: ReturnType<typeof createPollingInterval> | null = null;
	let isPollingTransformProgress = false;

	// SSE connection for real-time transform status updates
	let datasetSSE: SSEConnection | null = null;
	let sseRefreshTimeout: ReturnType<typeof setTimeout> | null = null;

	// Initialize search query from hash URL parameter early
	function getInitialSearchQuery(): string {
		if (typeof window === 'undefined') return '';
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const params = new SvelteURLSearchParams(hashParts[1]);
			const searchParam = params.get('search');
			if (searchParam) {
				// Remove the search param from the URL
				params.delete('search');
				const newQueryString = params.toString();
				const hashBase = hashParts[0];
				const newHash = newQueryString ? `${hashBase}?${newQueryString}` : hashBase;
				window.history.replaceState(null, '', newHash);
				return decodeURIComponent(searchParam);
			}
		}
		return '';
	}

	// Search state
	let searchQuery = $state(getInitialSearchQuery());
	let searchFetchTimeout: ReturnType<typeof setTimeout> | null = null;

	// Delete state
	let deletingItem = $state<number | null>(null);
	let itemPendingDelete = $state<DatasetItemSummary | null>(null);
	let datasetPendingDelete = $state(false);
	let updatingPublic = $state(false);

	// Dataset Transform Modal state
	let datasetTransformModalOpen = $state(false);

	const examplePayload = {
		items: [
			{
				title: 'Document Title 1',
				chunks: ['First chunk of text content...', 'Second chunk of text content...'],
				metadata: {
					source: 'example.pdf',
					page: 1,
					custom_field: 'custom_value',
				},
			},
			{
				title: 'Document Title 2',
				chunks: ['Another chunk of content...'],
				metadata: {
					source: 'example2.pdf',
					page: 1,
				},
			},
		],
	};

	async function fetchDataset() {
		try {
			loading = true;
			error = null;
			const response = await fetch(`/api/datasets/${datasetId}`);
			if (!response.ok) {
				if (response.status === 404) {
					throw new Error('Dataset not found');
				}
				throw new Error(`Failed to fetch dataset: ${response.statusText}`);
			}
			dataset = await response.json();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to fetch dataset';
		} finally {
			loading = false;
		}
	}

	async function togglePublic() {
		if (!dataset) return;

		try {
			updatingPublic = true;
			const response = await fetch(`/api/datasets/${datasetId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: dataset.title,
					details: dataset.details,
					tags: dataset.tags,
					is_public: !dataset.is_public,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to update dataset: ${response.statusText}`);
			}

			const updatedDataset = await response.json();
			dataset = updatedDataset;
			toastStore.success(
				updatedDataset.is_public ? 'Dataset is now public' : 'Dataset is now private'
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to update dataset visibility'));
		} finally {
			updatingPublic = false;
		}
	}

	async function fetchCollectionTransformStats(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				collectionTransformStatsMap.set(transformId, stats);
				collectionTransformStatsMap = collectionTransformStatsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(e);
		}
	}

	async function fetchDatasetTransformStats(transformId: number) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				datasetTransformStatsMap.set(transformId, stats);
				datasetTransformStatsMap = datasetTransformStatsMap; // Trigger reactivity

				// Check if this transform is currently processing and we're not already tracking it
				if (stats.is_processing && !activeTransformProgress) {
					// Find the transform to get its title
					const transform = datasetTransforms.find((t) => t.dataset_transform_id === transformId);
					if (transform) {
						console.info(`Detected active transform ${transformId}, resuming progress tracking`);
						activeTransformProgress = {
							id: transformId,
							title: transform.title,
							startedAt: stats.first_processing_at || new Date().toISOString(),
							embedders: transform.embedder_ids,
						};
						transformProgressStats = stats;
						startTransformProgressPolling();
					}
				} else if (
					stats.is_processing &&
					activeTransformProgress &&
					activeTransformProgress.id === transformId
				) {
					// Update stats for the currently tracked transform
					transformProgressStats = stats;
				}
			}
		} catch (e) {
			console.error(e);
		}
	}

	function startEdit() {
		if (!dataset) return;
		editMode = true;
		editTitle = dataset.title;
		editDetails = dataset.details || '';
		editTags = dataset.tags.join(', ');
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editTitle = '';
		editDetails = '';
		editTags = '';
		editError = null;
	}

	async function saveEdit() {
		if (!dataset) return;

		if (!editTitle.trim()) {
			editError = 'Title is required';
			return;
		}

		const tags = editTags
			.split(',')
			.map((tag) => tag.trim())
			.filter((tag) => tag.length > 0);

		try {
			saving = true;
			editError = null;
			const response = await fetch(`/api/datasets/${datasetId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: editTitle.trim(),
					details: editDetails.trim() || null,
					tags,
					is_public: dataset.is_public,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to update dataset: ${response.statusText}`);
			}

			const updatedDataset = await response.json();
			dataset = updatedDataset;
			editMode = false;
			toastStore.success('Dataset updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update dataset');
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function fetchDatasetTransforms() {
		try {
			transformsLoading = true;

			// Fetch all transform data in parallel for efficiency using filtered endpoints
			const [collectionResponse, datasetResponse, embeddedResponse] = await Promise.all([
				// Fetch collection transforms targeting this Dataset
				fetch(`/api/datasets/${datasetId}/collection-transforms`),
				// Fetch dataset transforms (this Dataset → Embedded Datasets)
				fetch(`/api/datasets/${datasetId}/transforms`),
				// Fetch embedded datasets (created from this Dataset)
				fetch(`/api/datasets/${datasetId}/embedded-datasets`),
			]);

			// Process collection transforms (already filtered by server)
			if (collectionResponse.ok) {
				const collectionData: CollectionTransform[] = await collectionResponse.json();
				collectionTransforms = collectionData.sort(
					(a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
				);

				// Fetch stats for each collection transform
				for (const transform of collectionTransforms) {
					fetchCollectionTransformStats(transform.collection_transform_id);
				}
			}

			// Process dataset transforms (already filtered by server)
			if (datasetResponse.ok) {
				const datasetData: DatasetTransform[] = await datasetResponse.json();
				datasetTransforms = datasetData.sort(
					(a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
				);

				// Fetch stats for each dataset transform
				for (const transform of datasetTransforms) {
					fetchDatasetTransformStats(transform.dataset_transform_id);
				}
			}

			// Process embedded datasets (already filtered by server)
			if (embeddedResponse.ok) {
				const embeddedData: EmbeddedDataset[] = await embeddedResponse.json();
				embeddedDatasets = embeddedData.sort(
					(a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
				);
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to load related transforms'));
		} finally {
			transformsLoading = false;
		}
	}

	async function handleTransformCreated(transformId: number, transformTitle: string) {
		// Set up progress tracking for the newly created transform
		activeTransformProgress = {
			id: transformId,
			title: transformTitle,
			startedAt: new Date().toISOString(),
			embedders: [],
		};

		// Start polling for progress immediately
		startTransformProgressPolling();

		// Also immediately refresh transforms list (don't wait for delay)
		fetchDatasetTransforms();

		// Use retry mechanism to ensure the transform appears if initial fetch failed
		let retryCount = 0;
		const maxRetries = 3;

		const fetchWithRetry = async () => {
			const foundTransform = datasetTransforms.find((t) => t.dataset_transform_id === transformId);
			if (!foundTransform && retryCount < maxRetries) {
				retryCount++;
				await fetchDatasetTransforms();
				if (retryCount < maxRetries) {
					setTimeout(fetchWithRetry, 2000); // Wait 2 seconds before retrying
				}
			}
		};

		// Start retrying after 1 second if transform not found initially
		setTimeout(fetchWithRetry, 1000);
	}

	async function fetchTransformProgressStats() {
		if (!activeTransformProgress) {
			return;
		}

		try {
			const response = await fetch(`/api/dataset-transforms/${activeTransformProgress.id}/stats`);
			if (!response.ok) {
				const errorText = await response.text();
				console.error(
					`Failed to fetch stats: ${response.status} ${response.statusText}`,
					errorText
				);
				return;
			}

			const stats = await response.json();
			transformProgressStats = stats;

			// Also update the stats map for the transforms list
			datasetTransformStatsMap.set(activeTransformProgress.id, stats);
			datasetTransformStatsMap = datasetTransformStatsMap;

			// Check if the transform is complete (only terminal states, not just !is_processing)
			const terminalStatuses = ['completed', 'completed_with_errors', 'failed'];
			const isTerminal = terminalStatuses.includes(stats.status);
			const isIdleWithWork =
				stats.status === 'idle' &&
				!stats.is_processing &&
				(stats.total_chunks_embedded > 0 || stats.total_batches_processed > 0);

			if (isTerminal || isIdleWithWork) {
				console.info(
					`Transform ${activeTransformProgress.id} ${stats.status}, is_processing=${stats.is_processing}, stopping polling (terminal=${isTerminal}, idleWithWork=${isIdleWithWork})`
				);
				stopTransformProgressPolling();
				// Refresh the transforms list to get final state
				fetchDatasetTransforms();
				// Keep showing the progress panel for 3 more seconds, then auto-dismiss
				setTimeout(() => {
					activeTransformProgress = null;
					transformProgressStats = null;
				}, 3000);
			}
		} catch (e) {
			console.error('Failed to fetch transform progress:', e);
			// Stop polling on persistent errors to avoid spam
			if (e instanceof TypeError) {
				console.error('TypeError in fetch, stopping polling');
				stopTransformProgressPolling();
			}
		}
	}

	function startTransformProgressPolling() {
		// Stops any existing polling
		if (transformProgressPollingController) {
			transformProgressPollingController.stop();
		}

		// Fetch stats immediately before starting interval
		fetchTransformProgressStats();

		// Create managed polling with deduplication to prevent race conditions
		transformProgressPollingController = createPollingInterval(
			async () => {
				if (isPollingTransformProgress) return;
				isPollingTransformProgress = true;
				try {
					await fetchTransformProgressStats();
				} finally {
					isPollingTransformProgress = false;
				}
			},
			{
				interval: 1000,
				shouldContinue: () => {
					// Continue polling only if transform is still active
					return activeTransformProgress !== null;
				},
				onError: (error, retryCount) => {
					console.debug(`Transform progress polling error (attempt ${retryCount}):`, error);
				},
			}
		);
	}

	function stopTransformProgressPolling() {
		if (transformProgressPollingController) {
			transformProgressPollingController.stop();
			transformProgressPollingController = null;
		}
	}

	async function fetchItems() {
		try {
			itemsLoading = true;
			itemsError = null;
			const params = new SvelteURLSearchParams({
				page: currentPage.toString(),
				page_size: pageSize.toString(),
			});
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			const response = await fetch(`/api/datasets/${datasetId}/items-summary?${params.toString()}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch items: ${response.statusText}`);
			}
			paginatedItems = await response.json();
		} catch (e) {
			itemsError = e instanceof Error ? e.message : 'Failed to fetch items';
		} finally {
			itemsLoading = false;
		}
	}

	async function fetchItemChunks(itemId: number) {
		// Return cached chunks if available
		if (chunksCache[itemId]) {
			return chunksCache[itemId];
		}

		try {
			loadingChunksItemId = itemId;
			const response = await fetch(`/api/datasets/${datasetId}/items/${itemId}/chunks`);
			if (!response.ok) {
				throw new Error(`Failed to fetch chunks: ${response.statusText}`);
			}
			const chunks: DatasetItemChunks = await response.json();
			chunksCache[itemId] = chunks;
			return chunks;
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to load chunks'));
			return null;
		} finally {
			loadingChunksItemId = null;
		}
	}

	function goToPage(page: number) {
		currentPage = page;
		fetchItems();
	}

	function changePageSize(newSize: number) {
		pageSize = newSize;
		currentPage = 0;
		fetchItems();
	}

	function toggleItem(itemId: number) {
		if (expandedItemId === itemId) {
			// Closing the item
			expandedItemId = null;
			delete chunkCurrentPages[itemId];
		} else {
			// Opening the item - fetch chunks if not cached
			expandedItemId = itemId;
			if (!chunksCache[itemId]) {
				fetchItemChunks(itemId);
			}
		}
	}

	function getChunkPage(itemId: number): number {
		return chunkCurrentPages[itemId] ?? 0;
	}

	function goToChunkPage(itemId: number, page: number) {
		chunkCurrentPages[itemId] = page;
	}

	function getPaginatedChunks(
		itemId: number,
		chunks: Array<{ content: string; metadata: Record<string, any> }>
	) {
		const currentChunkPage = getChunkPage(itemId);
		const startIdx = currentChunkPage * chunkPageSize;
		const endIdx = startIdx + chunkPageSize;
		return chunks.slice(startIdx, endIdx);
	}

	function getChunkPageInfo(itemId: number, totalChunks: number) {
		const currentChunkPage = getChunkPage(itemId);
		const totalPages = Math.ceil(totalChunks / chunkPageSize);
		const startIdx = currentChunkPage * chunkPageSize;
		const endIdx = Math.min(startIdx + chunkPageSize, totalChunks);
		return {
			currentPage: currentChunkPage,
			totalPages,
			startIdx: startIdx + 1,
			endIdx,
			totalChunks,
			hasMore: currentChunkPage + 1 < totalPages,
			hasPrevious: currentChunkPage > 0,
		};
	}

	// Reset to page 0 when search query changes
	$effect(() => {
		// Access searchQuery to create the dependency
		if (searchQuery !== undefined) {
			currentPage = 0;
		}
	});

	// Refetch items when current page or search query changes (debounced)
	$effect(() => {
		// Access both variables to create dependencies
		if (currentPage !== undefined && searchQuery !== undefined) {
			// Clear any pending fetch
			if (searchFetchTimeout) {
				clearTimeout(searchFetchTimeout);
			}

			// Debounce the fetch by 500ms to avoid excessive API calls during rapid typing
			searchFetchTimeout = setTimeout(() => {
				fetchItems();
			}, 500);
		}

		// Cleanup function
		return () => {
			if (searchFetchTimeout) {
				clearTimeout(searchFetchTimeout);
			}
		};
	});

	function requestDeleteItem(item: DatasetItemSummary) {
		itemPendingDelete = item;
	}

	async function confirmDeleteItem() {
		if (!itemPendingDelete) {
			return;
		}

		const target = itemPendingDelete;
		itemPendingDelete = null;

		try {
			deletingItem = target.item_id;
			const response = await fetch(`/api/datasets/${datasetId}/items/${target.item_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete item: ${response.statusText}`);
			}

			// Refresh the items list
			await fetchItems();
			toastStore.success(
				'Dataset item deleted and all associated chunks removed from embedded dataset'
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete dataset item'));
		} finally {
			deletingItem = null;
		}
	}

	async function confirmDeleteDataset() {
		if (!dataset) return;

		datasetPendingDelete = false;

		try {
			const response = await fetch(`/api/datasets/${dataset.dataset_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to delete dataset: ${errorText}`);
			}

			toastStore.success('Dataset deleted successfully');
			onBack();
		} catch (e) {
			const message = formatError(e, 'Failed to delete dataset');
			toastStore.error(message);
		}
	}

	function connectSSE() {
		// Connect to dataset transforms stream (for transforms that process this dataset)
		// Dataset transforms use source_dataset_id in their subject for filtering
		datasetSSE = createSSEConnection({
			url: `/api/dataset-transforms/stream?dataset_id=${datasetId}`,
			onStatus: (data: unknown) => {
				const status = data as { dataset_transform_id?: number };
				if (status.dataset_transform_id) {
					fetchDatasetTransformStats(status.dataset_transform_id);

					// Debounced refresh of transforms list to ensure new transforms are shown
					// This helps catch newly created transforms that might not appear immediately
					if (sseRefreshTimeout) {
						clearTimeout(sseRefreshTimeout);
					}
					sseRefreshTimeout = setTimeout(() => {
						fetchDatasetTransforms();
					}, 500); // Debounce for 500ms
				}
			},
			onMaxRetriesReached: () => {
				console.warn('SSE connection lost for dataset transforms');
			},
		});
	}

	onMount(() => {
		fetchDataset();
		fetchDatasetTransforms();
		connectSSE();
		fetchItems();

		return () => {
			// Cleanup polling on unmount
			stopTransformProgressPolling();
			if (searchFetchTimeout) {
				clearTimeout(searchFetchTimeout);
			}
		};
	});

	onDestroy(() => {
		datasetSSE?.disconnect();
		transformProgressPollingController?.stop();
		// Clean up any pending search fetch
		if (searchFetchTimeout) {
			clearTimeout(searchFetchTimeout);
		}
		// Clean up SSE refresh timeout
		if (sseRefreshTimeout) {
			clearTimeout(sseRefreshTimeout);
		}
	});
</script>

<div class=" mx-auto">
	<div class="mb-4">
		<button onclick={onBack} class="mb-4 btn-secondary inline-flex items-center gap-2">
			<ArrowLeftOutline class="w-5 h-5" />
			Back to Datasets
		</button>

		{#if loading}
			<LoadingState message="Loading dataset..." />
		{:else if error}
			<div
				class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
			>
				<p class="text-red-700 dark:text-red-400">{error}</p>
				<button
					onclick={fetchDataset}
					class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
				>
					Try again
				</button>
			</div>
		{:else if dataset}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
				{#if editMode}
					<!-- Edit Mode -->
					<div class="mb-4">
						<div class="flex justify-between items-center mb-4">
							<h2 class="text-xl font-semibold text-gray-900 dark:text-white">Edit Dataset</h2>
							<div class="flex gap-2">
								<button
									type="button"
									onclick={cancelEdit}
									disabled={saving}
									class="px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors disabled:opacity-50"
								>
									Cancel
								</button>
								<button
									type="button"
									onclick={saveEdit}
									disabled={saving}
									class="px-3 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors disabled:opacity-50"
								>
									{saving ? 'Saving...' : 'Save Changes'}
								</button>
							</div>
						</div>

						{#if editError}
							<div
								class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3 mb-4"
							>
								<p class="text-sm text-red-700 dark:text-red-400">{editError}</p>
							</div>
						{/if}

						<div class="space-y-4">
							<div>
								<label
									for="edit-title"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Title <span class="text-red-500">*</span>
								</label>
								<input
									id="edit-title"
									type="text"
									bind:value={editTitle}
									disabled={saving}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
									placeholder="Enter dataset title"
								/>
							</div>

							<div>
								<label
									for="edit-details"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Details
								</label>
								<textarea
									id="edit-details"
									bind:value={editDetails}
									disabled={saving}
									rows="3"
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
									placeholder="Enter dataset details (optional)"
								></textarea>
							</div>

							<div>
								<label
									for="edit-tags"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Tags
								</label>
								<input
									id="edit-tags"
									type="text"
									bind:value={editTags}
									disabled={saving}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
									placeholder="Enter tags separated by commas"
								/>
								<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
									Separate multiple tags with commas
								</p>
							</div>
						</div>
					</div>
				{:else}
					<!-- View Mode -->
					<div class="flex justify-between items-start mb-2">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
									{dataset.title}
								</h1>
								<span class="text-sm text-gray-500 dark:text-gray-400">
									#{dataset.dataset_id}
								</span>
							</div>
							{#if dataset.details}
								<p class="text-gray-600 dark:text-gray-400 mb-3">
									{dataset.details}
								</p>
							{/if}
							<div class="flex items-center gap-2 flex-wrap">
								{#each dataset.tags as tag (tag)}
									<span
										class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
									>
										#{tag}
									</span>
								{/each}
							</div>
							<div class="mt-3">
								<label class="inline-flex items-center gap-2 cursor-pointer">
									<input
										type="checkbox"
										checked={dataset.is_public}
										onchange={togglePublic}
										disabled={updatingPublic}
										class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">
										{#if dataset.is_public}
											<span class="font-semibold text-green-600 dark:text-green-400">Public</span> - visible
											in marketplace
										{:else}
											<span class="font-semibold text-gray-600 dark:text-gray-400">Private</span> - only
											visible to you
										{/if}
									</span>
								</label>
							</div>
						</div>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={() => (apiIntegrationModalOpen = true)}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-lg transition-colors"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
									/>
								</svg>
								API
							</button>
							<button
								type="button"
								onclick={startEdit}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
								title="Edit dataset"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
									/>
								</svg>
								Edit
							</button>
							<button
								type="button"
								onclick={() => (datasetPendingDelete = true)}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
								title="Delete dataset"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
									/>
								</svg>
								Delete
							</button>
						</div>
					</div>
				{/if}
			</div>

			{#if activeTransformProgress}
				<div class="mb-6">
					<DatasetTransformProgressPanel
						datasetTransformId={activeTransformProgress.id}
						title={activeTransformProgress.title}
						sourceDatasetTitle={dataset?.title || 'Unknown Dataset'}
						overallStatus={transformProgressStats?.status || 'processing'}
						totalItemsProcessed={transformProgressStats?.total_chunks_embedded || 0}
						totalItems={transformProgressStats?.total_chunks_to_process || 0}
						startedAt={activeTransformProgress.startedAt}
						embedderProgresses={transformProgressStats?.embedders || []}
					/>
				</div>
			{/if}

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
				<TabPanel {tabs} activeTabId={activeTab} onChange={(tabId: string) => (activeTab = tabId)}>
					{#snippet children(tabId)}
						{#if tabId === 'overview'}
							<div id="overview-panel" role="tabpanel" class="animate-fadeIn">
								<div class="p-4 border-b border-gray-200 dark:border-gray-700">
									<div class="flex justify-between items-center">
										<h2 class="text-2xl font-bold text-gray-900 dark:text-white">Dataset Items</h2>
										<label class="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
											<span>Per page:</span>
											<select
												bind:value={pageSize}
												onchange={() => changePageSize(pageSize)}
												class="pl-3 pr-8 py-1 border border-gray-300 dark:border-gray-600 rounded-lg dark:bg-gray-700 dark:text-white"
											>
												<option value={10}>10</option>
												<option value={25}>25</option>
												<option value={50}>50</option>
												<option value={100}>100</option>
											</select>
										</label>
									</div>
								</div>

								{#if (paginatedItems && paginatedItems.items.length > 0) || searchQuery.trim()}
									<div class="px-6 pt-4 pb-4">
										<div class="relative">
											<input
												type="text"
												bind:value={searchQuery}
												placeholder="Search items by title, chunks, or metadata..."
												class="w-full px-4 py-2 pl-10 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
											/>
											<svg
												class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400"
												fill="none"
												stroke="currentColor"
												viewBox="0 0 24 24"
											>
												<path
													stroke-linecap="round"
													stroke-linejoin="round"
													stroke-width="2"
													d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
												/>
											</svg>
										</div>
									</div>
								{/if}

								{#if itemsLoading}
									<LoadingState message="Loading items..." />
								{:else if itemsError}
									<div class="p-4">
										<div
											class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
										>
											<p class="text-red-700 dark:text-red-400">{itemsError}</p>
											<button
												onclick={fetchItems}
												class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
											>
												Try again
											</button>
										</div>
									</div>
								{:else if paginatedItems && paginatedItems.items.length === 0}
									<div class="p-12 text-center">
										<svg
											class="w-16 h-16 mx-auto mb-4 text-gray-400"
											fill="none"
											stroke="currentColor"
											viewBox="0 0 24 24"
										>
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
											></path>
										</svg>
										{#if searchQuery.trim()}
											<p class="text-gray-500 dark:text-gray-400 mb-2">
												No results match the search term
											</p>
											<button
												onclick={() => (searchQuery = '')}
												class="text-blue-600 dark:text-blue-400 hover:underline"
											>
												Clear search
											</button>
										{:else}
											<p class="text-gray-500 dark:text-gray-400 mb-2">No items yet</p>
											<p class="text-sm text-gray-400 dark:text-gray-500">
												Upload data via the API below to populate this dataset
											</p>
										{/if}
									</div>
								{:else if paginatedItems}
									<div class="overflow-x-auto">
										<table class="w-full">
											<thead
												class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700"
											>
												<tr>
													<th
														class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
													>
														ID
													</th>
													<th
														class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
													>
														Title
													</th>
													<th
														class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
													>
														Chunks
													</th>
													<th
														class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
													>
														Actions
													</th>
												</tr>
											</thead>
											<tbody
												class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700"
											>
												{#each paginatedItems.items as item (item.item_id)}
													<tr class="hover:bg-gray-50 dark:hover:bg-gray-700">
														<td
															class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400"
														>
															#{item.item_id}
														</td>
														<td class="px-6 py-4 text-sm text-gray-900 dark:text-white">
															<div class="max-w-md truncate">
																{item.title}
															</div>
														</td>
														<td
															class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400"
														>
															{item.chunk_count} chunk{item.chunk_count !== 1 ? 's' : ''}
														</td>
														<td class="px-6 py-4 whitespace-nowrap text-sm">
															<div class="flex items-center gap-2">
																<button
																	onclick={() => toggleItem(item.item_id)}
																	class="text-blue-600 dark:text-blue-400 hover:underline"
																>
																	{expandedItemId === item.item_id ? 'Hide' : 'View'} Details
																</button>
																<button
																	onclick={() => requestDeleteItem(item)}
																	disabled={deletingItem === item.item_id}
																	class="text-red-600 dark:text-red-400 hover:underline disabled:opacity-50 disabled:cursor-not-allowed"
																	title="Delete item"
																>
																	{#if deletingItem === item.item_id}
																		Deleting...
																	{:else}
																		Delete
																	{/if}
																</button>
															</div>
														</td>
													</tr>
													{#if expandedItemId === item.item_id}
														<tr>
															<td colspan="4" class="px-6 py-4 bg-gray-50 dark:bg-gray-900">
																<div class="space-y-4">
																	{#if loadingChunksItemId === item.item_id}
																		<div class="flex items-center justify-center py-4">
																			<div
																				class="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600"
																			></div>
																		</div>
																	{:else if chunksCache[item.item_id]}
																		{@const cachedData = chunksCache[item.item_id]}
																		<div>
																			<div class="flex items-center justify-between mb-2">
																				<h4
																					class="text-sm font-semibold text-gray-700 dark:text-gray-300"
																				>
																					Chunks ({cachedData.chunks.length})
																				</h4>
																				<label
																					class="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400"
																				>
																					<span>Per page:</span>
																					<select
																						bind:value={chunkPageSize}
																						onchange={() => {}}
																						class="pl-2 pr-6 py-1 border border-gray-300 dark:border-gray-600 rounded text-xs dark:bg-gray-700 dark:text-white"
																					>
																						<option value={5}>5</option>
																						<option value={10}>10</option>
																						<option value={20}>20</option>
																					</select>
																				</label>
																			</div>

																			{#if cachedData.chunks.length > 0}
																				<div class="space-y-2 mb-3">
																					{#each getPaginatedChunks(item.item_id, cachedData.chunks) as chunk, idx (idx)}
																						{@const chunkPageInfo = getChunkPageInfo(
																							item.item_id,
																							cachedData.chunks.length
																						)}
																						{@const actualChunkNumber =
																							chunkPageInfo.startIdx + idx}
																						<div
																							class="bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700"
																						>
																							<div
																								class="text-xs text-gray-500 dark:text-gray-400 mb-1"
																							>
																								Chunk {actualChunkNumber}
																							</div>
																							<p
																								class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap"
																							>
																								{chunk.content}
																							</p>
																						</div>
																					{/each}
																				</div>

																				{#if cachedData.chunks.length > chunkPageSize}
																					{@const chunkPageInfo = getChunkPageInfo(
																						item.item_id,
																						cachedData.chunks.length
																					)}
																					<div
																						class="flex items-center justify-between border-t border-gray-200 dark:border-gray-700 pt-3"
																					>
																						<div class="text-xs text-gray-600 dark:text-gray-400">
																							Showing {chunkPageInfo.startIdx} to {chunkPageInfo.endIdx}
																							of {chunkPageInfo.totalChunks}
																						</div>
																						<div class="flex gap-2">
																							<button
																								onclick={() =>
																									goToChunkPage(
																										item.item_id,
																										getChunkPage(item.item_id) - 1
																									)}
																								disabled={!chunkPageInfo.hasPrevious}
																								class="px-2 py-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
																							>
																								Previous
																							</button>
																							<span
																								class="px-2 py-1 text-xs text-gray-600 dark:text-gray-400"
																							>
																								Page {chunkPageInfo.currentPage + 1} of {chunkPageInfo.totalPages}
																							</span>
																							<button
																								onclick={() =>
																									goToChunkPage(
																										item.item_id,
																										getChunkPage(item.item_id) + 1
																									)}
																								disabled={!chunkPageInfo.hasMore}
																								class="px-2 py-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
																							>
																								Next
																							</button>
																						</div>
																					</div>
																				{/if}
																			{/if}
																		</div>
																		{#if cachedData.metadata && Object.keys(cachedData.metadata).length > 0}
																			<div>
																				<h4
																					class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2"
																				>
																					Metadata
																				</h4>
																				<pre
																					class="bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700 text-xs overflow-x-auto"><code
																						class="text-gray-900 dark:text-gray-300"
																						>{JSON.stringify(cachedData.metadata, null, 2)}</code
																					></pre>
																			</div>
																		{/if}
																	{/if}
																</div>
															</td>
														</tr>
													{/if}
												{/each}
											</tbody>
										</table>
									</div>

									{#if paginatedItems.total_count > 0}
										<div
											class="px-6 py-4 bg-gray-50 dark:bg-gray-900 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between"
										>
											<div class="text-sm text-gray-700 dark:text-gray-300">
												Showing {currentPage * pageSize + 1} to {Math.min(
													(currentPage + 1) * pageSize,
													paginatedItems.total_count
												)} of {paginatedItems.total_count} items
											</div>
											<div class="flex gap-2">
												<button
													onclick={() => goToPage(currentPage - 1)}
													disabled={currentPage === 0}
													class="px-3 py-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
												>
													Previous
												</button>
												<button
													onclick={() => goToPage(currentPage + 1)}
													disabled={!paginatedItems.has_more}
													class="px-3 py-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
												>
													Next
												</button>
											</div>
										</div>
									{/if}
								{/if}
							</div>
						{:else if tabId === 'transforms'}
							<div id="transforms-panel" role="tabpanel" class="animate-fadeIn">
								<div>
									{#if transformsLoading}
										<div class="flex items-center justify-center py-8">
											<div
												class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"
											></div>
										</div>
									{:else if collectionTransforms.length === 0 && datasetTransforms.length === 0}
										<div
											class="text-center py-8 bg-gray-50 dark:bg-gray-900/30 rounded-lg border border-dashed border-gray-300 dark:border-gray-700"
										>
											<p class="text-gray-500 dark:text-gray-400 mb-2">
												No transforms reference this dataset yet.
											</p>
											<p class="text-sm text-gray-400 dark:text-gray-500">
												Create transforms to process collections into this dataset or embed items
												from this dataset.
											</p>
											<div class="mt-4">
												<button
													onclick={() => (datasetTransformModalOpen = true)}
													class="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium"
													title="Create a transform to embed items from this dataset"
												>
													<ExpandOutline class="w-5 h-5" />
													Create Dataset Transform
												</button>
											</div>
										</div>
									{:else}
										<div class="flex justify-between items-center mb-6">
											<h2 class="text-2xl font-bold text-gray-900 dark:text-white">Transforms</h2>
											<button
												onclick={() => (datasetTransformModalOpen = true)}
												class="inline-flex items-center gap-2 px-3 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium"
												title="Create a transform to embed items from this dataset"
											>
												<ExpandOutline class="w-5 h-5" />
												Create Dataset Transform
											</button>
										</div>
										<div class="space-y-6">
											<!-- Collection Transforms: Collection → This Dataset -->
											{#if collectionTransforms.length > 0}
												<div>
													<h3
														class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-3 flex items-center gap-2"
													>
														<span
															class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 text-xs font-bold"
														>
															{collectionTransforms.length}
														</span>
														Collection Transforms (Collection → Dataset)
													</h3>
													<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
														Transforms that process collections and output to this dataset
													</p>
													<TransformsList
														transforms={collectionTransforms.map((t) => ({
															...t,
															last_run_stats: collectionTransformStatsMap.get(
																t.collection_transform_id
															),
														}))}
														type="collection"
														loading={transformsLoading}
													/>
												</div>
											{/if}

											<!-- Dataset Transforms: This Dataset → Embedded Datasets -->
											{#if datasetTransforms.length > 0}
												<div>
													<h3
														class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-3 flex items-center gap-2"
													>
														<span
															class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 text-xs font-bold"
														>
															{datasetTransforms.length}
														</span>
														Dataset Transforms (Dataset → Embedded Datasets)
													</h3>
													<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
														Transforms that embed items from this dataset
													</p>
													<TransformsList
														transforms={datasetTransforms.map((t) => ({
															...t,
															last_run_stats: datasetTransformStatsMap.get(t.dataset_transform_id),
														}))}
														type="dataset"
														loading={transformsLoading}
													/>
												</div>
											{/if}
										</div>
									{/if}
								</div>
							</div>
						{:else if tabId === 'embeddings'}
							<div id="embeddings-panel" role="tabpanel" class="animate-fadeIn">
								<div>
									{#if transformsLoading}
										<div class="flex items-center justify-center py-8">
											<div
												class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"
											></div>
										</div>
									{:else if embeddedDatasets.length === 0}
										<div
											class="text-center py-8 bg-gray-50 dark:bg-gray-900/30 rounded-lg border border-dashed border-gray-300 dark:border-gray-700"
										>
											<p class="text-gray-500 dark:text-gray-400 mb-2">No embedded datasets yet.</p>
											<p class="text-sm text-gray-400 dark:text-gray-500">
												Create dataset transforms to embed items from this dataset.
											</p>
										</div>
									{:else}
										<div>
											<h3
												class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-3 flex items-center gap-2"
											>
												<span
													class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 text-xs font-bold"
												>
													{embeddedDatasets.length}
												</span>
												Embedded Datasets (Vector Embeddings)
											</h3>
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
												Vector embeddings created from this dataset
											</p>
											<div class="space-y-2">
												{#each embeddedDatasets as embedded (embedded.embedded_dataset_id)}
													<div
														class="rounded-lg border border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20 p-3 hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors"
													>
														<div
															class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
														>
															<div>
																<button
																	onclick={() => {
																		if (typeof window !== 'undefined') {
																			window.location.hash = `/embedded-datasets/${embedded.embedded_dataset_id}/details`;
																		}
																	}}
																	class="text-sm font-semibold text-blue-700 dark:text-blue-300 hover:underline text-left"
																	type="button"
																>
																	{embedded.title}
																</button>
																<p class="text-xs text-gray-600 dark:text-gray-400">
																	Collection: {embedded.collection_name}
																</p>
															</div>
															<div class="text-xs text-gray-500 dark:text-gray-400">
																Updated {formatDate(embedded.updated_at)}
															</div>
														</div>
													</div>
												{/each}
											</div>
										</div>
									{/if}
								</div>
							</div>
						{/if}
					{/snippet}
				</TabPanel>
			</div>
		{/if}
	</div>
</div>

<ConfirmDialog
	open={datasetPendingDelete}
	title="Delete Dataset?"
	message="Are you sure you want to delete this dataset? This action cannot be undone."
	confirmLabel="Delete"
	cancelLabel="Cancel"
	onConfirm={confirmDeleteDataset}
	onCancel={() => (datasetPendingDelete = false)}
	variant="danger"
/>

<ConfirmDialog
	title="Delete dataset item"
	message={itemPendingDelete
		? `Are you sure you want to delete "${itemPendingDelete.title}"? This will also remove all associated chunks from embedded dataset. This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteItem}
	onCancel={() => (itemPendingDelete = null)}
/>

<CreateDatasetTransformModal
	bind:open={datasetTransformModalOpen}
	{datasetId}
	onSuccess={(transformId, transformTitle) => {
		datasetTransformModalOpen = false;
		// Start tracking progress for the new transform
		handleTransformCreated(transformId, transformTitle);
		// Switch to transforms tab to show progress
		activeTab = 'transforms';
	}}
/>

<ApiIntegrationModal
	bind:open={apiIntegrationModalOpen}
	type="dataset"
	resourceId={datasetId}
	{examplePayload}
/>
