<script lang="ts">
	import { onMount } from 'svelte';
	import ApiExamples from '../ApiExamples.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Dataset {
		dataset_id: number;
		title: string;
		details: string | null;
		owner: string;
		tags: string[];
		is_public: boolean;
	}

	interface DatasetItem {
		item_id: number;
		dataset_id: number;
		title: string;
		chunks: Array<{ content: string; metadata: Record<string, any> }>;
		metadata: Record<string, any>;
	}

	interface CollectionTransform {
		collection_transform_id: number;
		title: string;
		collection_id: number;
		dataset_id: number;
		owner: string;
		is_enabled: boolean;
		chunk_size: number;
		job_config: Record<string, any>;
		created_at: string;
		updated_at: string;
	}

	interface DatasetTransform {
		dataset_transform_id: number;
		title: string;
		source_dataset_id: number;
		embedder_ids: number[];
		owner: string;
		is_enabled: boolean;
		job_config: Record<string, any>;
		created_at: string;
		updated_at: string;
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		dataset_transform_id: number;
		source_dataset_id: number;
		embedder_id: number;
		owner: string;
		collection_name: string;
		created_at: string;
		updated_at: string;
	}

	interface PaginatedItems {
		items: DatasetItem[];
		page: number;
		page_size: number;
		total_count: number;
		has_more: boolean;
	}

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

	let paginatedItems = $state<PaginatedItems | null>(null);
	let itemsLoading = $state(false);
	let itemsError = $state<string | null>(null);
	let currentPage = $state(0);
	let pageSize = $state(10);
	let expandedItemId = $state<number | null>(null);

	// Search state
	let searchQuery = $state('');

	// Delete state
	let deletingItem = $state<number | null>(null);
	let itemPendingDelete = $state<DatasetItem | null>(null);
	let updatingPublic = $state(false);

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
			const response = await fetch('/api/datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch datasets: ${response.statusText}`);
			}
			const datasets: Dataset[] = await response.json();
			dataset = datasets.find((d) => d.dataset_id === datasetId) || null;
			if (!dataset) {
				throw new Error('Dataset not found');
			}
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

	async function fetchDatasetTransforms() {
		try {
			transformsLoading = true;

			// Fetch collection transforms (Collection → this Dataset)
			const collectionResponse = await fetch('/api/collection-transforms');
			if (collectionResponse.ok) {
				const allCollectionTransforms: CollectionTransform[] = await collectionResponse.json();
				collectionTransforms = allCollectionTransforms
					.filter((t) => t.dataset_id === datasetId)
					.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
			}

			// Fetch dataset transforms (this Dataset → Embedded Datasets)
			const datasetResponse = await fetch('/api/dataset-transforms');
			if (datasetResponse.ok) {
				const allDatasetTransforms: DatasetTransform[] = await datasetResponse.json();
				datasetTransforms = allDatasetTransforms
					.filter((t) => t.source_dataset_id === datasetId)
					.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
			}

			// Fetch embedded datasets (created from this Dataset)
			const embeddedResponse = await fetch('/api/embedded-datasets');
			if (embeddedResponse.ok) {
				const allEmbeddedDatasets: EmbeddedDataset[] = await embeddedResponse.json();
				embeddedDatasets = allEmbeddedDatasets
					.filter((ed) => ed.source_dataset_id === datasetId)
					.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to load related transforms'));
		} finally {
			transformsLoading = false;
		}
	}

	function formatTimestamp(timestamp: string) {
		try {
			return new Date(timestamp).toLocaleString();
		} catch (e) {
			console.warn('Failed to format timestamp', e);
			return timestamp;
		}
	}

	async function fetchItems() {
		try {
			itemsLoading = true;
			itemsError = null;
			const response = await fetch(
				`/api/datasets/${datasetId}/items?page=${currentPage}&page_size=${pageSize}`
			);
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
		expandedItemId = expandedItemId === itemId ? null : itemId;
	}

	// Filtered items based on search
	let filteredItems = $derived(
		paginatedItems?.items.filter((item) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				item.title.toLowerCase().includes(query) ||
				item.chunks.some((chunk) => chunk.content.toLowerCase().includes(query)) ||
				Object.values(item.metadata || {}).some((value) =>
					String(value).toLowerCase().includes(query)
				)
			);
		}) || []
	);

	function requestDeleteItem(item: DatasetItem) {
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

	onMount(() => {
		fetchDataset();
		fetchDatasetTransforms();
		fetchItems();
	});
</script>

<div class="max-w-7xl mx-auto">
	<div class="mb-6">
		<button
			onclick={onBack}
			class="mb-4 px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors inline-flex items-center gap-2"
		>
			<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M10 19l-7-7m0 0l7-7m-7 7h18"
				></path>
			</svg>
			Back to Datasets
		</button>

		{#if loading}
			<div class="flex items-center justify-center py-12">
				<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
			</div>
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
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
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
					<span
						class="inline-flex items-center gap-1.5 px-3 py-1 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-full text-sm"
					>
						<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
							></path>
						</svg>
						{dataset.owner}
					</span>

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
								<span class="font-semibold text-gray-600 dark:text-gray-400">Private</span> - only visible
								to you
							{/if}
						</span>
					</label>
				</div>
			</div>

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<div class="flex items-center justify-between mb-4">
					<h2 class="text-2xl font-bold text-gray-900 dark:text-white">Related Transforms</h2>
				</div>

				<p class="text-gray-600 dark:text-gray-400 mb-6">
					Transforms and embeddings related to this dataset.
				</p>

				{#if transformsLoading}
					<div class="flex items-center justify-center py-8">
						<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
					</div>
				{:else if collectionTransforms.length === 0 && datasetTransforms.length === 0 && embeddedDatasets.length === 0}
					<div
						class="text-center py-8 bg-gray-50 dark:bg-gray-900/30 rounded-lg border border-dashed border-gray-300 dark:border-gray-700"
					>
						<p class="text-gray-500 dark:text-gray-400 mb-2">
							No transforms reference this dataset yet.
						</p>
						<p class="text-sm text-gray-400 dark:text-gray-500">
							Create transforms to process collections into this dataset or embed items from this
							dataset.
						</p>
					</div>
				{:else}
					<div class="space-y-6">
						<!-- Collection Transforms: Collection → This Dataset -->
						{#if collectionTransforms.length > 0}
							<div>
								<h3
									class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3 flex items-center gap-2"
								>
									<span
										class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 text-xs font-bold"
									>
										{collectionTransforms.length}
									</span>
									Collection Transforms (Collection → Dataset)
								</h3>
								<div class="space-y-2">
									{#each collectionTransforms as transform (transform.collection_transform_id)}
										<div
											class="rounded-lg border border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20 p-3"
										>
											<div
												class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
											>
												<div>
													<button
														onclick={() => {
															if (typeof window !== 'undefined') {
																window.location.hash = '/collection-transforms';
															}
														}}
														class="text-sm font-semibold text-green-700 dark:text-green-300 hover:underline text-left"
														type="button"
													>
														{transform.title}
													</button>
													<p class="text-xs text-gray-600 dark:text-gray-400">
														Chunk size: {transform.chunk_size}
														{#if transform.is_enabled}
															<span class="ml-2 text-green-600 dark:text-green-400">● Enabled</span>
														{:else}
															<span class="ml-2 text-gray-500 dark:text-gray-500">● Disabled</span>
														{/if}
													</p>
												</div>
												<div class="text-xs text-gray-500 dark:text-gray-400">
													Updated {formatTimestamp(transform.updated_at)}
												</div>
											</div>
										</div>
									{/each}
								</div>
							</div>
						{/if}

						<!-- Dataset Transforms: This Dataset → Embedded Datasets -->
						{#if datasetTransforms.length > 0}
							<div>
								<h3
									class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3 flex items-center gap-2"
								>
									<span
										class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 text-xs font-bold"
									>
										{datasetTransforms.length}
									</span>
									Dataset Transforms (Dataset → Embedded Datasets)
								</h3>
								<div class="space-y-2">
									{#each datasetTransforms as transform (transform.dataset_transform_id)}
										<div
											class="rounded-lg border border-purple-200 dark:border-purple-800 bg-purple-50 dark:bg-purple-900/20 p-3"
										>
											<div
												class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
											>
												<div>
													<button
														onclick={() => {
															if (typeof window !== 'undefined') {
																window.location.hash = '/dataset-transforms';
															}
														}}
														class="text-sm font-semibold text-purple-700 dark:text-purple-300 hover:underline text-left"
														type="button"
													>
														{transform.title}
													</button>
													<p class="text-xs text-gray-600 dark:text-gray-400">
														{transform.embedder_ids.length} embedder{transform.embedder_ids
															.length !== 1
															? 's'
															: ''} configured
														{#if transform.is_enabled}
															<span class="ml-2 text-purple-600 dark:text-purple-400"
																>● Enabled</span
															>
														{:else}
															<span class="ml-2 text-gray-500 dark:text-gray-500">● Disabled</span>
														{/if}
													</p>
												</div>
												<div class="text-xs text-gray-500 dark:text-gray-400">
													Updated {formatTimestamp(transform.updated_at)}
												</div>
											</div>
										</div>
									{/each}
								</div>
							</div>
						{/if}

						<!-- Embedded Datasets: Results from Dataset Transforms -->
						{#if embeddedDatasets.length > 0}
							<div>
								<h3
									class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3 flex items-center gap-2"
								>
									<span
										class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 text-xs font-bold"
									>
										{embeddedDatasets.length}
									</span>
									Embedded Datasets (Vector Embeddings)
								</h3>
								<div class="space-y-2">
									{#each embeddedDatasets as embedded (embedded.embedded_dataset_id)}
										<div
											class="rounded-lg border border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20 p-3"
										>
											<div
												class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
											>
												<div>
													<button
														onclick={() => {
															if (typeof window !== 'undefined') {
																window.location.hash = '/embedded-datasets';
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
													Updated {formatTimestamp(embedded.updated_at)}
												</div>
											</div>
										</div>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</div>

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md mb-6">
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

				{#if paginatedItems && paginatedItems.items.length > 0}
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
					<div class="flex items-center justify-center py-12">
						<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
					</div>
				{:else if itemsError}
					<div class="p-6">
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
						<p class="text-gray-500 dark:text-gray-400 mb-2">No items yet</p>
						<p class="text-sm text-gray-400 dark:text-gray-500">
							Upload data via the API below to populate this dataset
						</p>
					</div>
				{:else if paginatedItems && filteredItems.length === 0}
					<div class="p-12 text-center">
						<p class="text-gray-500 dark:text-gray-400 mb-4">No items match your search</p>
						<button
							onclick={() => (searchQuery = '')}
							class="text-blue-600 dark:text-blue-400 hover:underline"
						>
							Clear search
						</button>
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
								{#each filteredItems as item (item.item_id)}
									<tr class="hover:bg-gray-50 dark:hover:bg-gray-750">
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
											{item.chunks.length} chunk{item.chunks.length !== 1 ? 's' : ''}
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
													<div>
														<h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
															Chunks ({item.chunks.length})
														</h4>
														<div class="space-y-2">
															{#each item.chunks as chunk, idx (idx)}
																<div
																	class="bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700"
																>
																	<div class="text-xs text-gray-500 dark:text-gray-400 mb-1">
																		Chunk {idx + 1}
																	</div>
																	<p
																		class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap"
																	>
																		{chunk.content}
																	</p>
																</div>
															{/each}
														</div>
													</div>
													{#if item.metadata && Object.keys(item.metadata).length > 0}
														<div>
															<h4
																class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2"
															>
																Metadata
															</h4>
															<pre
																class="bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700 text-xs overflow-x-auto"><code
																	>{JSON.stringify(item.metadata, null, 2)}</code
																></pre>
														</div>
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

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
				<h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-4">API Integration</h2>
				<p class="text-sm text-gray-600 dark:text-gray-400 mb-6">
					Use these examples to upload data to this dataset programmatically.
				</p>

				<div class="mb-6">
					<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
						Upload dataset items
					</h3>
					<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
						Send a JSON payload with an array of items. Each item must contain:
					</p>
					<ul class="list-disc list-inside text-sm text-gray-600 dark:text-gray-400 space-y-1 mb-4">
						<li><strong>title</strong>: String - The title/name of the document or item</li>
						<li><strong>chunks</strong>: Array of strings - Text chunks (at least one required)</li>
						<li><strong>metadata</strong>: Object - Any additional metadata as key-value pairs</li>
					</ul>

					<ApiExamples
						endpoint="/api/datasets/{datasetId}/items"
						method="POST"
						body={examplePayload}
					/>
				</div>

				<div
					class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg p-4"
				>
					<h4 class="text-sm font-semibold text-yellow-900 dark:text-yellow-300 mb-2">
						Important Notes
					</h4>
					<ul class="list-disc list-inside text-sm text-yellow-800 dark:text-yellow-400 space-y-1">
						<li>Authentication is required via the access_token cookie</li>
						<li>Each item's chunks array must contain at least one chunk</li>
						<li>The items array must contain at least one item</li>
						<li>Metadata can be any valid JSON object</li>
						<li>Response includes "completed" and "failed" arrays with item titles</li>
					</ul>
				</div>
			</div>
		{/if}
	</div>
</div>

<ConfirmDialog
	open={itemPendingDelete !== null}
	title="Delete dataset item"
	message={itemPendingDelete
		? `Are you sure you want to delete "${itemPendingDelete.title}"? This will also remove all associated chunks from embedded dataset. This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteItem}
	on:cancel={() => (itemPendingDelete = null)}
/>
