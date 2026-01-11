<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import {
		Heading,
		Table,
		TableBody,
		TableBodyCell,
		TableHead,
		TableHeadCell,
	} from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateCollectionTransformModal from '../components/CreateCollectionTransformModal.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface CollectionTransform {
		collection_transform_id: number;
		title: string;
		collection_id: number;
		dataset_id: number;
		owner: string;
		is_enabled: boolean;
		chunk_size: number;
		job_config: any;
		created_at: string;
		updated_at: string;
	}

	interface Collection {
		collection_id: number;
		title: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface Stats {
		collection_transform_id: number;
		total_files_processed: number;
		successful_files: number;
		failed_files: number;
		total_items_created: number;
	}

	interface ProcessedFile {
		id: number;
		transform_type: string;
		transform_id: number;
		file_key: string;
		processed_at: string;
		item_count: number;
		process_status: string;
		process_error: string | null;
		processing_duration_ms: number | null;
	}

	interface PaginatedResponse {
		items: CollectionTransform[];
		total_count: number;
		limit: number;
		offset: number;
	}

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

	// Modal state
	let showCreateModal = $state(false);

	let transformPendingDelete = $state<CollectionTransform | null>(null);

	// Selection state
	// eslint-disable-next-line svelte/no-unnecessary-state-wrap
	let selected = $state(new SvelteSet<number>());
	let selectAll = $state(false);

	function toggleSelectAll() {
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
				await toggleEnabled(transform, false);
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
			const params = new URLSearchParams({
				limit: pageSize.toString(),
				offset: offset.toString(),
				sort_by: sortBy,
				sort_direction: sortDirection,
			});
			const response = await fetch(`/api/collection-transforms?${params}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch collection transforms: ${response.statusText}`);
			}
			const data: PaginatedResponse = await response.json();
			transforms = data.items;
			totalCount = data.total_count;
			for (const transform of transforms) {
				fetchStatsForTransform(transform.collection_transform_id);
			}
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
			collections = await response.json();
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
			datasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch datasets:', e);
		}
	}

	async function toggleEnabled(transform: CollectionTransform, refresh = true) {
		try {
			const response = await fetch(
				`/api/collection-transforms/${transform.collection_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: !transform.is_enabled,
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

	onMount(async () => {
		await Promise.all([fetchTransforms(), fetchCollections(), fetchDatasets()]);
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

	let filteredTransforms = $derived(transforms);

	function getCollectionTitle(collectionId: number): string {
		const collection = collections.find((c) => c.collection_id === collectionId);
		return collection ? collection.title : `Collection ${collectionId}`;
	}

	function getDatasetTitle(datasetId: number): string {
		const dataset = datasets.find((d) => d.dataset_id === datasetId);
		return dataset ? dataset.title : `Dataset ${datasetId}`;
	}

	function getTotalPages(): number {
		return Math.ceil(totalCount / pageSize);
	}

	function getSortIcon(field: string): string {
		if (sortBy !== field) return '';
		return sortDirection === 'asc' ? '▲' : '▼';
	}
</script>

<div class="max-w-7xl mx-auto px-4">
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
			currentPage = 1;
			fetchTransforms();
		}}
	/>

	<div class="mb-4 flex gap-4">
		<input
			type="text"
			bind:value={searchQuery}
			placeholder="Filter by title or owner (client-side)..."
			class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
		<div class="flex gap-2 items-center">
			<label for="page-size" class="text-sm text-gray-600 dark:text-gray-400">Per page:</label>
			<select
				id="page-size"
				bind:value={pageSize}
				onchange={(e) => handlePageSizeChange(parseInt(e.currentTarget.value))}
				class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
			>
				{#each pageSizeOptions as option (option)}
					<option value={option}>{option}</option>
				{/each}
			</select>
		</div>
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
	{:else if filteredTransforms.length === 0}
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
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 w-12">
						<input
							type="checkbox"
							checked={selectAll}
							onchange={() => toggleSelectAll()}
							class="cursor-pointer"
						/>
					</TableHeadCell>
					<TableHeadCell
						class="px-4 py-3 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700"
						onclick={() => handleSort('title')}
					>
						<div class="flex items-center gap-2">
							Title
							{getSortIcon('title')}
						</div>
					</TableHeadCell>
					<TableHeadCell class="px-4 py-3">Collection</TableHeadCell>
					<TableHeadCell class="px-4 py-3">Dataset</TableHeadCell>
					<TableHeadCell
						class="px-4 py-3 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700"
						onclick={() => handleSort('is_enabled')}
					>
						<div class="flex items-center gap-2">
							Status
							{getSortIcon('is_enabled')}
						</div>
					</TableHeadCell>
					<TableHeadCell class="px-4 py-3">Files Processed</TableHeadCell>
					<TableHeadCell class="px-4 py-3">Items Created</TableHeadCell>
					<TableHeadCell
						class="px-4 py-3 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700"
						onclick={() => handleSort('created_at')}
					>
						<div class="flex items-center gap-2">
							Created
							{getSortIcon('created_at')}
						</div>
					</TableHeadCell>
					<TableHeadCell class="px-4 py-3 w-12 text-center">Edit</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredTransforms as transform (transform.collection_transform_id)}
						{@const stats = statsMap.get(transform.collection_transform_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(transform.collection_transform_id)}
									onchange={() => toggleSelect(transform.collection_transform_id)}
									class="cursor-pointer"
								/>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 font-medium text-gray-900 dark:text-white">
								{transform.title}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-sm">
								<a
									href="#/collections/{transform.collection_id}/details"
									class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
								>
									{getCollectionTitle(transform.collection_id)}
								</a>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-sm">
								<a
									href="#/datasets/{transform.dataset_id}/details"
									class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
								>
									{getDatasetTitle(transform.dataset_id)}
								</a>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span
									class={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
										transform.is_enabled
											? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
											: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-400'
									}`}
								>
									{transform.is_enabled ? 'Enabled' : 'Disabled'}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-sm">
								{stats?.total_files_processed ?? '-'}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-sm">
								{stats?.total_items_created ?? '-'}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-sm">
								{new Date(transform.created_at).toLocaleDateString()}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								<button
									onclick={() => openEditForm(transform)}
									title="Edit"
									class="text-gray-600 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-300 transition-colors"
								>
									✎
								</button>
							</TableBodyCell>
						</tr>
					{/each}
				</TableBody>
			</Table>
		</div>
	{/if}

	<div class="mt-6 flex items-center justify-between">
		<div class="text-sm text-gray-600 dark:text-gray-400">
			Showing {(currentPage - 1) * pageSize + 1} to {Math.min(currentPage * pageSize, totalCount)} of
			{totalCount} transforms
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
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
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
