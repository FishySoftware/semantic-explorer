<script lang="ts">
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ActionMenu from '../components/ActionMenu.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateDatasetTransformModal from '../components/CreateDatasetTransformModal.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import SearchInput from '../components/SearchInput.svelte';
	import type { Dataset, PaginatedResponse } from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';

	let { onViewDataset: handleViewDataset } = $props<{
		onViewDataset: (_: number) => void;
	}>();

	const onViewDataset = (id: number) => {
		handleViewDataset(id);
	};

	const onCreateTransform = (datasetId: number) => {
		selectedDatasetForTransform = datasetId;
		transformModalOpen = true;
	};

	let datasets = $state<Dataset[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let totalCount = $state(0);
	let currentOffset = $state(0);
	const pageSize = 20;

	let searchQuery = $state('');

	let filteredDatasets = $derived(datasets);

	let showCreateForm = $state(false);
	let editingDataset = $state<Dataset | null>(null);
	let newTitle = $state('');
	let newDetails = $state('');
	let newTags = $state('');
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let datasetPendingDelete = $state<Dataset | null>(null);

	// Selection state for bulk operations
	let selected = new SvelteSet<number>();
	let selectAll = $state(false);
	let datasetsPendingBulkDelete = $state<Dataset[]>([]);

	let transformModalOpen = $state(false);
	let selectedDatasetForTransform = $state<number | null>(null);

	$effect(() => {
		if (showCreateForm && !editingDataset && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `datasets-${date}-${time}`;
		}
	});

	async function fetchDatasets(showLoading = true) {
		try {
			if (showLoading) loading = true;
			error = null;
			const params = new SvelteURLSearchParams();
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			params.append('limit', pageSize.toString());
			params.append('offset', currentOffset.toString());
			const url = `/api/datasets?${params.toString()}`;
			const response = await fetch(url);
			if (!response.ok) {
				throw new Error(`Failed to fetch datasets: ${response.statusText}`);
			}
			const data: PaginatedResponse<Dataset> = await response.json();
			datasets = data.items;
			totalCount = data.total_count;
		} catch (e) {
			const message = formatError(e, 'Failed to fetch datasets');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function createDataset() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		const tags = newTags
			.split(',')
			.map((tag) => tag.trim())
			.filter((tag) => tag.length > 0);

		try {
			creating = true;
			createError = null;

			const url = editingDataset ? `/api/datasets/${editingDataset.dataset_id}` : '/api/datasets';
			const method = editingDataset ? 'PATCH' : 'POST';

			const response = await fetch(url, {
				method,
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					title: newTitle,
					details: newDetails.trim() || null,
					tags,
				}),
			});

			if (!response.ok) {
				throw new Error(
					`Failed to ${editingDataset ? 'update' : 'create'} dataset: ${response.statusText}`
				);
			}

			const savedDataset = await response.json();

			if (editingDataset) {
				datasets = datasets.map((d) =>
					d.dataset_id === savedDataset.dataset_id ? savedDataset : d
				);
				toastStore.success('Dataset updated successfully');
				newTitle = '';
				newDetails = '';
				newTags = '';
				showCreateForm = false;
				editingDataset = null;
			} else {
				datasets = [...datasets, savedDataset];
				toastStore.success('Dataset created successfully');
				newTitle = '';
				newDetails = '';
				newTags = '';
				showCreateForm = false;
				editingDataset = null;
				handleViewDataset(savedDataset.dataset_id);
			}
		} catch (e) {
			const message = formatError(e, `Failed to ${editingDataset ? 'update' : 'create'} dataset`);
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	function openEditForm(dataset: Dataset) {
		editingDataset = dataset;
		newTitle = dataset.title;
		newDetails = dataset.details || '';
		newTags = dataset.tags.join(', ');
		showCreateForm = true;
	}

	function requestDeleteDataset(dataset: Dataset) {
		datasetPendingDelete = dataset;
	}

	async function confirmDeleteDataset() {
		if (!datasetPendingDelete) {
			return;
		}

		const target = datasetPendingDelete;
		datasetPendingDelete = null;

		try {
			const response = await fetch(`/api/datasets/${target.dataset_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete dataset: ${response.statusText}`);
			}

			datasets = datasets.filter((d) => d.dataset_id !== target.dataset_id);
			toastStore.success('Dataset deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete dataset'));
		}
	}

	function toggleSelectAll() {
		selectAll = !selectAll;
		if (selectAll) {
			selected.clear();
			for (const d of filteredDatasets) {
				selected.add(d.dataset_id);
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

	function bulkDelete() {
		const toDelete: Dataset[] = [];
		for (const id of selected) {
			const dataset = datasets.find((d) => d.dataset_id === id);
			if (dataset) {
				toDelete.push(dataset);
			}
		}
		if (toDelete.length > 0) {
			datasetsPendingBulkDelete = toDelete;
		}
	}

	async function confirmBulkDelete() {
		const toDelete = datasetsPendingBulkDelete;
		datasetsPendingBulkDelete = [];

		for (const dataset of toDelete) {
			try {
				const response = await fetch(`/api/datasets/${dataset.dataset_id}`, {
					method: 'DELETE',
				});

				if (!response.ok) {
					throw new Error(`Failed to delete: ${response.statusText}`);
				}

				datasets = datasets.filter((d) => d.dataset_id !== dataset.dataset_id);
			} catch (e) {
				toastStore.error(formatError(e, `Failed to delete "${dataset.title}"`));
			}
		}

		selected.clear();
		selectAll = false;
		toastStore.success(`Deleted ${toDelete.length} dataset${toDelete.length !== 1 ? 's' : ''}`);
	}

	// Refetch when search query changes
	// Debounce search to avoid spamming API on every keystroke
	let searchDebounceTimeout: ReturnType<typeof setTimeout> | null = null;
	$effect(() => {
		if (searchQuery !== undefined) {
			currentOffset = 0; // Reset to first page when searching
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
			searchDebounceTimeout = setTimeout(() => {
				fetchDatasets();
			}, 300); // 300ms debounce
		}
		return () => {
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
		};
	});

	// Auto-refresh every 5 seconds
	let refreshInterval: ReturnType<typeof setInterval> | null = null;
	$effect(() => {
		refreshInterval = setInterval(() => {
			fetchDatasets(false);
		}, 5000);
		return () => {
			if (refreshInterval) {
				clearInterval(refreshInterval);
			}
		};
	});

	function goToPreviousPage() {
		currentOffset = Math.max(0, currentOffset - pageSize);
		fetchDatasets();
	}

	function goToNextPage() {
		if (currentOffset + pageSize < totalCount) {
			currentOffset += pageSize;
			fetchDatasets();
		}
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Datasets"
		description="Contains processed texts as JSON with name and chunks, to be used for embedding transforms. Datasets can be generated from collections using transforms, or exported to the dataset endpoints directly via API."
	/>

	<div class="flex justify-between items-center mb-4">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Datasets</h1>
		<button onclick={() => (showCreateForm = !showCreateForm)} class="btn-primary">
			{showCreateForm ? 'Cancel' : 'Create Dataset'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingDataset ? 'Edit Dataset' : 'Create New Dataset'}
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					createDataset();
				}}
			>
				<div class="mb-4">
					<label
						for="dataset-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Title
					</label>
					<input
						id="dataset-title"
						type="text"
						bind:value={newTitle}
						placeholder="Enter dataset title"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
						required
					/>
				</div>
				<div class="mb-4">
					<label
						for="dataset-details"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Details
					</label>
					<textarea
						id="dataset-details"
						bind:value={newDetails}
						placeholder="Enter dataset details (optional)"
						rows="3"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
					></textarea>
				</div>
				<div class="mb-4">
					<label
						for="dataset-tags"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Tags <span class="text-xs text-gray-500 dark:text-gray-400">(comma-separated)</span>
					</label>
					<input
						id="dataset-tags"
						type="text"
						bind:value={newTags}
						placeholder="e.g., documentation, support, faq"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
					/>
				</div>
				{#if createError}
					<div
						class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400"
					>
						{createError}
					</div>
				{/if}
				<div class="flex gap-3">
					<button
						type="submit"
						disabled={creating}
						class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{creating
							? editingDataset
								? 'Updating...'
								: 'Creating...'
							: editingDataset
								? 'Update'
								: 'Create'}
					</button>
					<button
						type="button"
						onclick={() => {
							showCreateForm = false;
							editingDataset = null;
							newTitle = '';
							newDetails = '';
							newTags = '';
							createError = null;
						}}
						class="btn-secondary"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	{#if !showCreateForm}
		<SearchInput
			bind:value={searchQuery}
			placeholder="Search datasets by title, details, tags, or owner..."
		/>
	{/if}

	{#if loading}
		<LoadingState message="Loading datasets..." />
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
			<button
				onclick={() => fetchDatasets()}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if datasets.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No datasets yet</p>
			<button
				onclick={() => (showCreateForm = true)}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Create your first dataset
			</button>
		</div>
	{:else}
		{#if selected.size > 0}
			<div
				class="mb-4 flex items-center gap-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4"
			>
				<span class="text-sm text-blue-700 dark:text-blue-300 flex-1">
					{selected.size} dataset{selected.size !== 1 ? 's' : ''} selected
				</span>
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
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
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
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Title</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Description</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Tags</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Items</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Chunks</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Transforms</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredDatasets as dataset (dataset.dataset_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(dataset.dataset_id)}
									onchange={() => toggleSelect(dataset.dataset_id)}
									class="cursor-pointer"
								/>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<button
									onclick={() => onViewDataset(dataset.dataset_id)}
									class="font-semibold text-blue-600 dark:text-blue-400 hover:underline"
								>
									{dataset.title}
								</button>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								{#if dataset.details}
									<span class="text-gray-600 dark:text-gray-400 text-sm line-clamp-2"
										>{dataset.details}</span
									>
								{:else}
									<span class="text-gray-400 dark:text-gray-500 text-sm italic">No description</span
									>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<div class="flex flex-wrap gap-1">
									{#if dataset.tags && dataset.tags.length > 0}
										{#each dataset.tags.slice(0, 3) as tag (tag)}
											<span
												class="inline-flex items-center gap-1 px-2 py-0.5 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
											>
												#{tag}
											</span>
										{/each}
										{#if dataset.tags.length > 3}
											<span class="text-xs text-gray-500 dark:text-gray-400 px-1 py-0.5">
												+{dataset.tags.length - 3}
											</span>
										{/if}
									{:else}
										<span class="text-gray-400 dark:text-gray-500 text-xs italic">—</span>
									{/if}
								</div>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if dataset.item_count !== undefined && dataset.item_count !== null}
									<span
										class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-sm font-medium"
									>
										{dataset.item_count}
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if dataset.total_chunks !== undefined && dataset.total_chunks !== null}
									<span
										class="inline-block px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-sm font-medium"
									>
										{dataset.total_chunks}
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if dataset.transform_count !== undefined && dataset.transform_count !== null && dataset.transform_count > 0}
									<span
										class="inline-block px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-sm font-medium"
									>
										{dataset.transform_count}
									</span>
								{:else}
									<span class="text-gray-400 dark:text-gray-500 text-xs">None</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								<ActionMenu
									actions={[
										{
											label: 'View Details',
											handler: () => onViewDataset(dataset.dataset_id),
										},
										{
											label: 'Edit',
											handler: () => openEditForm(dataset),
										},
										...(dataset.item_count && dataset.item_count > 0
											? [
													{
														label: 'Create Transform',
														handler: () => onCreateTransform(dataset.dataset_id),
													},
												]
											: []),
										{
											label: 'Delete',
											handler: () => requestDeleteDataset(dataset),
											isDangerous: true,
										},
									]}
								/>
							</TableBodyCell>
						</tr>
					{/each}
				</TableBody>
			</Table>

			<!-- Pagination Controls -->
			<div class="mt-6 px-4 pb-4 flex items-center justify-between">
				<div class="text-sm text-gray-600 dark:text-gray-400">
					Showing {currentOffset + 1}-{Math.min(currentOffset + pageSize, totalCount)} of {totalCount}
					datasets
				</div>
				<div class="flex gap-2">
					<button
						onclick={goToPreviousPage}
						disabled={currentOffset === 0}
						class="px-4 py-2 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-100 dark:hover:bg-gray-700"
					>
						Previous
					</button>
					<button
						onclick={goToNextPage}
						disabled={currentOffset + pageSize >= totalCount}
						class="px-4 py-2 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-100 dark:hover:bg-gray-700"
					>
						Next
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>

<ConfirmDialog
	open={datasetPendingDelete !== null}
	title="Delete dataset"
	message={datasetPendingDelete
		? `Are you sure you want to delete "${datasetPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteDataset}
	onCancel={() => (datasetPendingDelete = null)}
/>

<ConfirmDialog
	open={datasetsPendingBulkDelete.length > 0}
	title="Delete Datasets"
	message={`Are you sure you want to delete ${datasetsPendingBulkDelete.length} dataset${datasetsPendingBulkDelete.length !== 1 ? 's' : ''}? This action cannot be undone.`}
	confirmLabel="Delete All"
	variant="danger"
	onConfirm={confirmBulkDelete}
	onCancel={() => (datasetsPendingBulkDelete = [])}
/>

<CreateDatasetTransformModal
	open={transformModalOpen}
	datasetId={selectedDatasetForTransform}
	onSuccess={() => {
		transformModalOpen = false;
		selectedDatasetForTransform = null;
		// Redirect to embedded datasets page to monitor transform progress
		window.location.hash = '#/embedded-datasets';
	}}
/>
