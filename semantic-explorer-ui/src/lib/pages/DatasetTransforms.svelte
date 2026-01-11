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
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

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
		provider: string;
	}

	interface Stats {
		dataset_transform_id: number;
		embedder_count: number;
		total_batches_processed: number;
		successful_batches: number;
		failed_batches: number;
		total_chunks_embedded: number;
		total_chunks_failed: number;
	}

	interface PaginatedResponse {
		items: DatasetTransform[];
		total_count: number;
		limit: number;
		offset: number;
	}

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

	// Create/edit form state
	let showCreateForm = $state(false);
	let editingTransform = $state<DatasetTransform | null>(null);
	let newTitle = $state('');
	let newDatasetId = $state<number | null>(null);
	let selectedEmbedderIds = $state<number[]>([]);
	let newWipeCollection = $state(false);
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let transformPendingDelete = $state<DatasetTransform | null>(null);

	// Selection state
	let selected = new SvelteSet<number>();
	let selectAll = $state(false);

	function toggleSelectAll() {
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
				transform.is_enabled = enable;
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
			const params = new URLSearchParams({
				limit: pageSize.toString(),
				offset: offset.toString(),
				sort_by: sortBy,
				sort_direction: sortDirection,
			});
			const response = await fetch(`/api/dataset-transforms?${params}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch dataset transforms: ${response.statusText}`);
			}
			const data: PaginatedResponse = await response.json();
			transforms = data.items;
			totalCount = data.total_count;
			for (const transform of transforms) {
				fetchStatsForTransform(transform.dataset_transform_id);
			}
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

	async function fetchEmbedders() {
		try {
			const response = await fetch('/api/embedders');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedders: ${response.statusText}`);
			}
			embedders = await response.json();
		} catch (e) {
			console.error('Failed to fetch embedders:', e);
		}
	}

	async function createTransform() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}
		if (!newDatasetId) {
			createError = 'Dataset is required';
			return;
		}
		if (selectedEmbedderIds.length === 0) {
			createError = 'At least one embedder is required';
			return;
		}

		try {
			creating = true;
			createError = null;

			const url = editingTransform
				? `/api/dataset-transforms/${editingTransform.dataset_transform_id}`
				: '/api/dataset-transforms';
			const method = editingTransform ? 'PATCH' : 'POST';

			const body = editingTransform
				? {
						title: newTitle,
						embedder_ids: selectedEmbedderIds,
						job_config: { wipe_collection: newWipeCollection },
					}
				: {
						title: newTitle,
						source_dataset_id: newDatasetId,
						embedder_ids: selectedEmbedderIds,
						wipe_collection: newWipeCollection,
					};

			const response = await fetch(url, {
				method,
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				throw new Error(
					`Failed to ${editingTransform ? 'update' : 'create'} dataset transform: ${response.statusText}`
				);
			}

			const responseData = await response.json();
			const savedTransform = responseData.transform || responseData;

			if (editingTransform) {
				transforms = transforms.map((t) =>
					t.dataset_transform_id === savedTransform.dataset_transform_id ? savedTransform : t
				);
				toastStore.success('Dataset transform updated successfully');
			} else {
				currentPage = 1;
				await fetchTransforms();
				toastStore.success(
					`Dataset transform created successfully with ${selectedEmbedderIds.length} embedder(s)`
				);
			}

			resetForm();
		} catch (e) {
			const message = formatError(
				e,
				`Failed to ${editingTransform ? 'update' : 'create'} dataset transform`
			);
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	async function toggleEnabled(transform: DatasetTransform, refresh = true) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transform.dataset_transform_id}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ is_enabled: !transform.is_enabled }),
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

	function openEditForm(transform: DatasetTransform) {
		editingTransform = transform;
		newTitle = transform.title;
		newDatasetId = transform.source_dataset_id;
		selectedEmbedderIds = [...transform.embedder_ids];
		newWipeCollection = transform.job_config?.wipe_collection || false;
		showCreateForm = true;
	}

	function resetForm() {
		newTitle = '';
		newDatasetId = null;
		selectedEmbedderIds = [];
		newWipeCollection = false;
		showCreateForm = false;
		editingTransform = null;
		createError = null;
	}

	function toggleEmbedder(embedderId: number) {
		if (selectedEmbedderIds.includes(embedderId)) {
			selectedEmbedderIds = selectedEmbedderIds.filter((id) => id !== embedderId);
		} else {
			selectedEmbedderIds = [...selectedEmbedderIds, embedderId];
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

	onMount(async () => {
		await Promise.all([fetchTransforms(), fetchDatasets(), fetchEmbedders()]);
	});

	function getDataset(datasetId: number) {
		return datasets.find((d) => d.dataset_id === datasetId);
	}

	function getTotalPages(): number {
		return Math.ceil(totalCount / pageSize);
	}

	function getSortIcon(field: string): string {
		if (sortBy !== field) return '';
		return sortDirection === 'asc' ? '▲' : '▼';
	}

	function getEmbeddersText(embedderIds: number[] | undefined): string {
		if (!embedderIds || embedderIds.length === 0) return 'None';
		const names = embedderIds
			.map((id) => embedders.find((e) => e.embedder_id === id)?.name || `Embedder ${id}`)
			.join(', ');
		return names.length > 40 ? names.substring(0, 40) + '...' : names;
	}
</script>

<div class="max-w-7xl mx-auto px-4">
	<PageHeader
		title="Dataset Transforms"
		description="Process Datasets with embedders to create Embedded Datasets. Each Dataset Transform can use multiple embedders, creating one Embedded Dataset per embedder."
	/>

	<div class="flex justify-between items-center mb-6">
		<Heading tag="h1" class="text-3xl font-bold">Dataset Transforms</Heading>
		<button
			onclick={() => {
				if (showCreateForm) {
					resetForm();
				} else {
					showCreateForm = true;
				}
			}}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Dataset Transform'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingTransform ? 'Edit Dataset Transform' : 'Create New Dataset Transform'}
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					createTransform();
				}}
			>
				<div class="mb-4">
					<label
						for="transform-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Title
					</label>
					<input
						id="transform-title"
						type="text"
						bind:value={newTitle}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						placeholder="Enter transform title..."
					/>
				</div>

				{#if !editingTransform}
					<div class="mb-4">
						<label
							for="dataset-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Source Dataset
						</label>
						<select
							id="dataset-select"
							bind:value={newDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select a dataset...</option>
							{#each datasets as dataset (dataset.dataset_id)}
								<option value={dataset.dataset_id}>{dataset.title}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="mb-4">
					<p class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
						Embedders (Select at least one)
					</p>
					<div
						class="border border-gray-300 dark:border-gray-600 rounded-lg p-4 max-h-64 overflow-y-auto bg-white dark:bg-gray-700"
					>
						{#if embedders.length === 0}
							<p class="text-sm text-gray-500 dark:text-gray-400">No embedders available.</p>
						{:else}
							{#each embedders as embedder (embedder.embedder_id)}
								<label
									class="flex items-center py-2 hover:bg-gray-50 dark:hover:bg-gray-600 px-2 rounded cursor-pointer"
								>
									<input
										type="checkbox"
										checked={selectedEmbedderIds.includes(embedder.embedder_id)}
										onchange={() => toggleEmbedder(embedder.embedder_id)}
										class="w-4 h-4 text-blue-600"
									/>
									<span class="ml-2 text-sm text-gray-900 dark:text-white">
										{embedder.name}
										<span class="text-gray-500 dark:text-gray-400">({embedder.provider})</span>
									</span>
								</label>
							{/each}
						{/if}
					</div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
						{selectedEmbedderIds.length} embedder(s) selected
					</p>
				</div>

				<div class="mb-4">
					<label class="flex items-center cursor-pointer">
						<input type="checkbox" bind:checked={newWipeCollection} class="w-4 h-4 text-blue-600" />
						<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">Wipe existing data</span>
					</label>
				</div>

				{#if createError}
					<div
						class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg"
					>
						<p class="text-sm text-red-600 dark:text-red-400">{createError}</p>
					</div>
				{/if}

				<div class="flex gap-3">
					<button
						type="submit"
						disabled={creating}
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
					>
						{creating
							? editingTransform
								? 'Updating...'
								: 'Creating...'
							: editingTransform
								? 'Update'
								: 'Create'}
					</button>
					<button
						type="button"
						onclick={resetForm}
						class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	<div class="mb-4 flex gap-4">
		<div class="flex-1"></div>
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
				No dataset transforms yet. Create one to get started!
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
					<TableHeadCell class="px-4 py-3">Source Dataset</TableHeadCell>
					<TableHeadCell class="px-4 py-3">Embedders</TableHeadCell>
					<TableHeadCell
						class="px-4 py-3 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700"
						onclick={() => handleSort('is_enabled')}
					>
						<div class="flex items-center gap-2">
							Status
							{getSortIcon('is_enabled')}
						</div>
					</TableHeadCell>
					<TableHeadCell class="px-4 py-3">Chunks Embedded</TableHeadCell>
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
					{#each transforms as transform (transform.dataset_transform_id)}
						{@const stats = statsMap.get(transform.dataset_transform_id)}
						{@const dataset = getDataset(transform.source_dataset_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(transform.dataset_transform_id)}
									onchange={() => toggleSelect(transform.dataset_transform_id)}
									class="cursor-pointer"
								/>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 font-medium text-gray-900 dark:text-white">
								{transform.title}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-sm">
								{#if dataset}
									<a
										href="#/datasets/{transform.source_dataset_id}/details"
										class="text-blue-600 dark:text-blue-400 hover:underline"
									>
										{dataset.title}
									</a>
								{:else}
									Dataset {transform.source_dataset_id}
								{/if}
							</TableBodyCell>
							<TableBodyCell
								class="px-4 py-3 text-sm"
								title={getEmbeddersText(transform.embedder_ids)}
							>
								{getEmbeddersText(transform.embedder_ids)}
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
								{stats?.total_chunks_embedded ?? '-'}
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
							class={`px-3 py-2 rounded-lg text-sm font-medium ${
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
