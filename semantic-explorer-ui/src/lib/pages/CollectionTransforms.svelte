<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
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

	let transforms = $state<CollectionTransform[]>([]);
	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let editingTransform = $state<CollectionTransform | null>(null);
	let newTitle = $state('');
	let newCollectionId = $state<number | null>(null);
	let newDatasetId = $state<number | null>(null);
	let newChunkSize = $state(200);
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<CollectionTransform | null>(null);

	$effect(() => {
		if (showCreateForm && !editingTransform && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `collection-transform-${date}-${time}`;
		}
	});

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/collection-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch collection transforms: ${response.statusText}`);
			}
			transforms = await response.json();
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

	async function createTransform() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		if (!newCollectionId) {
			createError = 'Collection is required';
			return;
		}

		if (!newDatasetId) {
			createError = 'Dataset is required';
			return;
		}

		try {
			creating = true;
			createError = null;

			const url = editingTransform
				? `/api/collection-transforms/${editingTransform.collection_transform_id}`
				: '/api/collection-transforms';
			const method = editingTransform ? 'PATCH' : 'POST';

			const body = editingTransform
				? {
						title: newTitle,
					}
				: {
						title: newTitle,
						collection_id: newCollectionId,
						dataset_id: newDatasetId,
						chunk_size: newChunkSize,
					};

			const response = await fetch(url, {
				method,
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				throw new Error(
					`Failed to ${editingTransform ? 'update' : 'create'} collection transform: ${response.statusText}`
				);
			}

			const savedTransform = await response.json();

			if (editingTransform) {
				transforms = transforms.map((t) =>
					t.collection_transform_id === savedTransform.collection_transform_id ? savedTransform : t
				);
				toastStore.success('Collection transform updated successfully');
			} else {
				transforms = [...transforms, savedTransform];
				toastStore.success('Collection transform created successfully');
			}

			resetForm();
		} catch (e) {
			const message = formatError(
				e,
				`Failed to ${editingTransform ? 'update' : 'create'} collection transform`
			);
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	async function toggleEnabled(transform: CollectionTransform) {
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

	function openEditForm(transform: CollectionTransform) {
		editingTransform = transform;
		newTitle = transform.title;
		newCollectionId = transform.collection_id;
		newDatasetId = transform.dataset_id;
		newChunkSize = transform.chunk_size;
		showCreateForm = true;
	}

	function resetForm() {
		newTitle = '';
		newCollectionId = null;
		newDatasetId = null;
		newChunkSize = 200;
		showCreateForm = false;
		editingTransform = null;
		createError = null;
	}

	function requestDeleteTransform(transform: CollectionTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.collection_transform_id;
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
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete collection transform'));
		} finally {
			deleting = null;
		}
	}

	onMount(async () => {
		await Promise.all([fetchTransforms(), fetchCollections(), fetchDatasets()]);
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);

			const action = urlParams.get('action');
			const collectionIdParam = urlParams.get('collection_id');

			let shouldOpenForm = false;

			if (action === 'create' && collectionIdParam) {
				const collectionId = parseInt(collectionIdParam, 10);
				if (!isNaN(collectionId)) {
					newCollectionId = collectionId;
					shouldOpenForm = true;
				}
			}

			if (shouldOpenForm) {
				showCreateForm = true;
				const basePath = hashParts[0];
				window.history.replaceState(
					null,
					'',
					window.location.pathname + window.location.search + basePath
				);
			}
		}
	});

	let filteredTransforms = $derived(
		transforms.filter((t) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return t.title.toLowerCase().includes(query) || t.owner.toLowerCase().includes(query);
		})
	);

	function getCollectionTitle(collectionId: number): string {
		const collection = collections.find((c) => c.collection_id === collectionId);
		return collection ? collection.title : `Collection ${collectionId}`;
	}

	function getDatasetTitle(datasetId: number): string {
		const dataset = datasets.find((d) => d.dataset_id === datasetId);
		return dataset ? dataset.title : `Dataset ${datasetId}`;
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Collection Transforms"
		description="Process files from Collections into Dataset items. Collection transforms extract text from files, chunk them into manageable pieces, and create Dataset items ready for embedding."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Collection Transforms</h1>
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
			{showCreateForm ? 'Cancel' : 'Create Collection Transform'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingTransform ? 'Edit Collection Transform' : 'Create New Collection Transform'}
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
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						placeholder="Enter transform title..."
					/>
				</div>

				{#if !editingTransform}
					<div class="mb-4">
						<label
							for="collection-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Source Collection
						</label>
						<select
							id="collection-select"
							bind:value={newCollectionId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select a collection...</option>
							{#each collections as collection (collection.collection_id)}
								<option value={collection.collection_id}>{collection.title}</option>
							{/each}
						</select>
					</div>

					<div class="mb-4">
						<label
							for="dataset-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Target Dataset
						</label>
						<select
							id="dataset-select"
							bind:value={newDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select a dataset...</option>
							{#each datasets as dataset (dataset.dataset_id)}
								<option value={dataset.dataset_id}>{dataset.title}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="mb-4">
					<label
						for="chunk-size"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Chunk Size
					</label>
					<input
						id="chunk-size"
						type="number"
						bind:value={newChunkSize}
						min="50"
						max="1000"
						disabled={editingTransform !== null}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
					/>
					<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
						{editingTransform
							? 'Chunk size cannot be changed after creation'
							: 'Number of characters per chunk (50-1000)'}
					</p>
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
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{creating
							? editingTransform
								? 'Updating...'
								: 'Creating...'
							: editingTransform
								? 'Update Transform'
								: 'Create Transform'}
					</button>
					<button
						type="button"
						onclick={resetForm}
						class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
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
	{:else if filteredTransforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No collection transforms found matching your search.'
					: 'No collection transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredTransforms as transform (transform.collection_transform_id)}
				{@const stats = statsMap.get(transform.collection_transform_id)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">
								{transform.title}
							</h3>
							<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
								<p>
									<strong>Collection:</strong>
									<a
										href="#/collections/{transform.collection_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{getCollectionTitle(transform.collection_id)}
									</a>
								</p>
								<p>
									<strong>Dataset:</strong>
									<a
										href="#/datasets/{transform.dataset_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{getDatasetTitle(transform.dataset_id)}
									</a>
								</p>
								<p><strong>Chunk Size:</strong> {transform.chunk_size}</p>
								<p><strong>Owner:</strong> {transform.owner}</p>
								<p>
									<strong>Status:</strong>
									<span
										class={transform.is_enabled
											? 'text-green-600 dark:text-green-400'
											: 'text-gray-500 dark:text-gray-400'}
									>
										{transform.is_enabled ? 'Enabled' : 'Disabled'}
									</span>
								</p>
							</div>
						</div>
						<div class="flex flex-col gap-2">
							<button
								onclick={() => toggleEnabled(transform)}
								class="px-3 py-1 text-sm rounded-lg {transform.is_enabled
									? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400'
									: 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400'}"
							>
								{transform.is_enabled ? 'Disable' : 'Enable'}
							</button>
							<button
								onclick={() => triggerTransform(transform.collection_transform_id)}
								class="px-3 py-1 text-sm bg-blue-100 text-blue-700 hover:bg-blue-200 rounded-lg dark:bg-blue-900/20 dark:text-blue-400"
							>
								Trigger
							</button>
							<button
								onclick={() => openEditForm(transform)}
								class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300"
							>
								Edit
							</button>
							<button
								onclick={() => requestDeleteTransform(transform)}
								disabled={deleting === transform.collection_transform_id}
								class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{deleting === transform.collection_transform_id ? 'Deleting...' : 'Delete'}
							</button>
						</div>
					</div>

					{#if stats}
						<div
							class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-4 gap-4"
						>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Files Processed</p>
								<p class="text-lg font-semibold text-gray-900 dark:text-white">
									{stats.total_files_processed}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Successful</p>
								<p class="text-lg font-semibold text-green-600 dark:text-green-400">
									{stats.successful_files}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Failed</p>
								<p class="text-lg font-semibold text-red-600 dark:text-red-400">
									{stats.failed_files}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Items Created</p>
								<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
									{stats.total_items_created}
								</p>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete Collection Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>
