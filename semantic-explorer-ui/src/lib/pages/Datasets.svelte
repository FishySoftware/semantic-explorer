<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Dataset {
		dataset_id: number;
		title: string;
		details: string | null;
		owner: string;
		tags: string[];
		item_count?: number;
		total_chunks?: number;
	}

	let { onViewDataset: handleViewDataset, onNavigate } = $props<{
		onViewDataset: (_: number) => void;
		onNavigate: (_: string) => void;
	}>();

	const onViewDataset = (id: number) => {
		handleViewDataset(id);
	};

	const onCreateTransform = (datasetId: number) => {
		onNavigate(`/dataset-transforms?action=create&dataset_id=${datasetId}`);
	};

	let datasets = $state<Dataset[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let editingDataset = $state<Dataset | null>(null);
	let newTitle = $state('');
	let newDetails = $state('');
	let newTags = $state('');
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let datasetPendingDelete = $state<Dataset | null>(null);

	$effect(() => {
		if (showCreateForm && !editingDataset && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `datasets-${date}-${time}`;
		}
	});

	async function fetchDatasets() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch datasets: ${response.statusText}`);
			}
			datasets = await response.json();
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
			} else {
				datasets = [...datasets, savedDataset];
				toastStore.success('Dataset created successfully');
			}

			newTitle = '';
			newDetails = '';
			newTags = '';
			showCreateForm = false;
			editingDataset = null;
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
			deleting = target.dataset_id;
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
		} finally {
			deleting = null;
		}
	}

	onMount(() => {
		fetchDatasets();
	});

	let filteredDatasets = $derived(
		datasets.filter((d) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				d.title.toLowerCase().includes(query) ||
				d.details?.toLowerCase().includes(query) ||
				d.tags.some((tag) => tag.toLowerCase().includes(query)) ||
				d.owner.toLowerCase().includes(query)
			);
		})
	);
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Datasets"
		description="Contains processed texts as JSON with name and chunks, to be used for embedding transforms. Datasets can be generated from collections using transforms, or exported to the dataset endpoints directly via API."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Datasets</h1>
		<button
			onclick={() => (showCreateForm = !showCreateForm)}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Dataset'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
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
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
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
						class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	{#if !showCreateForm && datasets.length > 0}
		<div class="mb-4">
			<div class="relative">
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search datasets by title, details, tags, or owner..."
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
				onclick={fetchDatasets}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if datasets.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No datasets yet</p>
			<button
				onclick={() => (showCreateForm = true)}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Create your first dataset
			</button>
		</div>
	{:else if filteredDatasets.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No datasets match your search</p>
			<button
				onclick={() => (searchQuery = '')}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Clear search
			</button>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredDatasets as dataset (dataset.dataset_id)}
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 hover:shadow-lg transition-shadow"
				>
					<div class="flex justify-between items-start">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h3 class="text-xl font-semibold text-gray-900 dark:text-white">
									{dataset.title}
								</h3>
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
								{#if dataset.item_count !== undefined}
									<span
										class="inline-flex items-center gap-1.5 px-3 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded-full text-sm"
										title="Number of items in this dataset"
									>
										<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
											></path>
										</svg>
										{dataset.item_count}
										{dataset.item_count === 1 ? 'item' : 'items'}
									</span>
								{/if}
								{#if dataset.total_chunks !== undefined && dataset.total_chunks > 0}
									<span
										class="inline-flex items-center gap-1.5 px-3 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded-full text-sm"
										title="Total number of chunks across all items"
									>
										<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M4 6h16M4 10h16M4 14h16M4 18h16"
											></path>
										</svg>
										{dataset.total_chunks}
										{dataset.total_chunks === 1 ? 'chunk' : 'chunks'}
									</span>
								{/if}
								{#each dataset.tags as tag (tag)}
									<span
										class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
									>
										#{tag}
									</span>
								{/each}
							</div>
						</div>
						<div class="ml-4 flex flex-col gap-2">
							<button
								onclick={() => openEditForm(dataset)}
								class="px-3 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700 transition-colors flex items-center gap-2 dark:bg-gray-500 dark:hover:bg-gray-600"
								title="Edit dataset"
							>
								<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
								onclick={() => onViewDataset(dataset.dataset_id)}
								class="px-3 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
								title="Manage dataset"
							>
								<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4"
									></path>
								</svg>
								Manage Dataset
							</button>
							{#if dataset.item_count && dataset.item_count > 0}
								<button
									onclick={() => onCreateTransform(dataset.dataset_id)}
									class="px-3 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors flex items-center gap-2"
									title="Create transform to embed this dataset"
								>
									<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"
										></path>
									</svg>
									Create Transform
								</button>
							{/if}
							<button
								onclick={() => requestDeleteDataset(dataset)}
								disabled={deleting === dataset.dataset_id}
								class="px-3 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors flex items-center gap-2"
								title="Delete dataset"
							>
								{#if deleting === dataset.dataset_id}
									<span class="animate-spin" role="status" aria-label="Deleting dataset">⏳</span>
									Deleting...
								{:else}
									<svg
										class="w-4 h-4"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
										aria-hidden="true"
									>
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
										/>
									</svg>
									Delete
								{/if}
							</button>
						</div>
					</div>
				</div>
			{/each}
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
	on:confirm={confirmDeleteDataset}
	on:cancel={() => (datasetPendingDelete = null)}
/>
