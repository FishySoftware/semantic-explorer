<script lang="ts">
	import { onMount } from 'svelte';
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

	let transforms = $state<DatasetTransform[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let editingTransform = $state<DatasetTransform | null>(null);
	let newTitle = $state('');
	let newDatasetId = $state<number | null>(null);
	let selectedEmbedderIds = $state<number[]>([]);
	let newWipeCollection = $state(false);
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<DatasetTransform | null>(null);

	$effect(() => {
		if (showCreateForm && !editingTransform && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `dataset-transform-${date}-${time}`;
		}
	});

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/dataset-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch dataset transforms: ${response.statusText}`);
			}
			transforms = await response.json();

			// Fetch stats for each transform
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

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap; // Trigger reactivity
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
						job_config: {
							wipe_collection: newWipeCollection,
						},
					}
				: {
						title: newTitle,
						source_dataset_id: newDatasetId,
						embedder_ids: selectedEmbedderIds,
						wipe_collection: newWipeCollection,
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
				transforms = [...transforms, savedTransform];
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

	async function toggleEnabled(transform: DatasetTransform) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transform.dataset_transform_id}`, {
				method: 'PATCH',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					is_enabled: !transform.is_enabled,
				}),
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
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle dataset transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/dataset-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
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

	function requestDeleteTransform(transform: DatasetTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.dataset_transform_id;
			const response = await fetch(`/api/dataset-transforms/${target.dataset_transform_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete dataset transform: ${response.statusText}`);
			}

			transforms = transforms.filter((t) => t.dataset_transform_id !== target.dataset_transform_id);
			toastStore.success('Dataset transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete dataset transform'));
		} finally {
			deleting = null;
		}
	}

	onMount(async () => {
		await Promise.all([fetchTransforms(), fetchDatasets(), fetchEmbedders()]);
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);

			const action = urlParams.get('action');
			const datasetIdParam = urlParams.get('dataset_id');

			let shouldOpenForm = false;

			if (action === 'create' && datasetIdParam) {
				const datasetId = parseInt(datasetIdParam, 10);
				if (!isNaN(datasetId)) {
					newDatasetId = datasetId;
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

	function getDataset(datasetId: number) {
		return datasets.find((d) => d.dataset_id === datasetId);
	}

	function getEmbeddersList(embedderIds: number[] | undefined) {
		if (!embedderIds || embedderIds.length === 0) {
			return [];
		}
		return embedderIds.map((id) => {
			const embedder = embedders.find((e) => e.embedder_id === id);
			return embedder || { embedder_id: id, name: `Embedder ${id}`, provider: 'Unknown' };
		});
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Dataset Transforms"
		description="Process Datasets with embedders to create Embedded Datasets. Each Dataset Transform can use multiple embedders, creating one Embedded Dataset per embedder. These embedded datasets contain vector representations stored in Qdrant for semantic search."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Dataset Transforms</h1>
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
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
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
					<p class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
						Embedders (Select at least one)
					</p>
					<div
						class="border border-gray-300 dark:border-gray-600 rounded-lg p-4 max-h-64 overflow-y-auto bg-white dark:bg-gray-700"
					>
						{#if embedders.length === 0}
							<p class="text-sm text-gray-500 dark:text-gray-400">
								No embedders available. Create an embedder first.
							</p>
						{:else}
							{#each embedders as embedder (embedder.embedder_id)}
								<label
									class="flex items-center py-2 hover:bg-gray-50 dark:hover:bg-gray-600 px-2 rounded cursor-pointer"
								>
									<input
										type="checkbox"
										checked={selectedEmbedderIds.includes(embedder.embedder_id)}
										onchange={() => toggleEmbedder(embedder.embedder_id)}
										class="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
									/>
									<span class="ml-2 text-sm text-gray-900 dark:text-white">
										{embedder.name}
										<span class="text-gray-500 dark:text-gray-400">
											({embedder.provider})
										</span>
									</span>
								</label>
							{/each}
						{/if}
					</div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
						{selectedEmbedderIds.length} embedder(s) selected. Each will create a separate Embedded Dataset.
					</p>
				</div>

				<div class="mb-4">
					<label class="flex items-center cursor-pointer">
						<input
							type="checkbox"
							bind:checked={newWipeCollection}
							class="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
						/>
						<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">
							Wipe existing data if applicable
						</span>
					</label>
					<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
						Warning: This will delete all existing embeddings in the target Qdrant collections
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
	{:else if filteredTransforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No dataset transforms found matching your search.'
					: 'No dataset transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredTransforms as transform (transform.dataset_transform_id)}
				{@const stats = statsMap.get(transform.dataset_transform_id)}
				{@const dataset = getDataset(transform.source_dataset_id)}
				{@const embeddersList = getEmbeddersList(transform.embedder_ids)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">
								{transform.title}
							</h3>
							<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
								<p>
									<strong>Source Dataset:</strong>
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
								</p>
								<div>
									<strong>Embedders ({transform.embedder_ids?.length ?? 0}):</strong>
									{#if embeddersList.length === 0}
										<p class="ml-4">None</p>
									{:else}
										<ul class="ml-4 list-disc list-inside">
											{#each embeddersList as embedder}
												<li>
													<a
														href="#/embedders?name={encodeURIComponent(embedder.name)}"
														class="text-blue-600 dark:text-blue-400 hover:underline"
													>
														{embedder.name}
													</a>
													<span class="text-gray-500 dark:text-gray-400">
														({embedder.provider})
													</span>
												</li>
											{/each}
										</ul>
									{/if}
								</div>
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
								onclick={() => triggerTransform(transform.dataset_transform_id)}
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
								disabled={deleting === transform.dataset_transform_id}
								class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{deleting === transform.dataset_transform_id ? 'Deleting...' : 'Delete'}
							</button>
						</div>
					</div>

					{#if stats}
						<div
							class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-3 gap-4"
						>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Batches Processed</p>
								<p class="text-lg font-semibold text-gray-900 dark:text-white">
									{stats.total_batches_processed}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Successful</p>
								<p class="text-lg font-semibold text-green-600 dark:text-green-400">
									{stats.successful_batches}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Failed</p>
								<p class="text-lg font-semibold text-red-600 dark:text-red-400">
									{stats.failed_batches}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Chunks Embedded</p>
								<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
									{stats.total_chunks_embedded}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Chunks Failed</p>
								<p class="text-lg font-semibold text-red-600 dark:text-red-400">
									{stats.total_chunks_failed}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Embedded Datasets</p>
								<p class="text-lg font-semibold text-purple-600 dark:text-purple-400">
									{stats.embedder_count}
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
	title="Delete Dataset Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This will also delete all associated Embedded Datasets. This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>
