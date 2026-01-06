<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface VisualizationTransform {
		visualization_transform_id: number;
		title: string;
		embedded_dataset_id: number;
		owner: string;
		is_enabled: boolean;
		reduced_collection_name: string | null;
		topics_collection_name: string | null;
		visualization_config: VisualizationConfig;
		created_at: string;
		updated_at: string;
	}

	interface VisualizationConfig {
		n_neighbors: number;
		n_components: number;
		min_dist: number;
		metric: string;
		min_cluster_size: number;
		min_samples: number | null;
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		embedder_name: string;
		source_dataset_title: string;
	}

	interface Stats {
		visualization_transform_id: number;
		total_points: number;
		total_clusters: number;
		noise_points: number;
	}

	let transforms = $state<VisualizationTransform[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let editingTransform = $state<VisualizationTransform | null>(null);
	let newTitle = $state('');
	let newEmbeddedDatasetId = $state<number | null>(null);
	let config = $state<VisualizationConfig>({
		n_neighbors: 15,
		n_components: 3,
		min_dist: 0.1,
		metric: 'cosine',
		min_cluster_size: 10,
		min_samples: null,
	});
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<VisualizationTransform | null>(null);

	$effect(() => {
		if (showCreateForm && !editingTransform && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `visualization-${date}-${time}`;
		}
	});

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/visualization-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			transforms = await response.json();

			// Fetch stats for each transform
			for (const transform of transforms) {
				fetchStatsForTransform(transform.visualization_transform_id);
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch visualization transforms');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/visualization-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function fetchEmbeddedDatasets() {
		try {
			const response = await fetch('/api/embedded-datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			embeddedDatasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch embedded datasets:', e);
		}
	}

	async function createTransform() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		if (!newEmbeddedDatasetId) {
			createError = 'Embedded Dataset is required';
			return;
		}

		try {
			creating = true;
			createError = null;

			const url = editingTransform
				? `/api/visualization-transforms/${editingTransform.visualization_transform_id}`
				: '/api/visualization-transforms';
			const method = editingTransform ? 'PATCH' : 'POST';

			const body = editingTransform
				? {
						title: newTitle,
						visualization_config: config,
					}
				: {
						title: newTitle,
						embedded_dataset_id: newEmbeddedDatasetId,
						visualization_config: config,
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
					`Failed to ${editingTransform ? 'update' : 'create'} visualization transform: ${response.statusText}`
				);
			}

			const savedTransform = await response.json();

			if (editingTransform) {
				transforms = transforms.map((t) =>
					t.visualization_transform_id === savedTransform.visualization_transform_id
						? savedTransform
						: t
				);
				toastStore.success('Visualization transform updated successfully');
			} else {
				transforms = [...transforms, savedTransform];
				toastStore.success('Visualization transform created successfully');
			}

			resetForm();
		} catch (e) {
			const message = formatError(
				e,
				`Failed to ${editingTransform ? 'update' : 'create'} visualization transform`
			);
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	async function toggleEnabled(transform: VisualizationTransform) {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${transform.visualization_transform_id}`,
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
				t.visualization_transform_id === updated.visualization_transform_id ? updated : t
			);

			toastStore.success(
				`Visualization transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle visualization transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/visualization-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({ visualization_transform_id: transformId }),
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger transform: ${response.statusText}`);
			}

			toastStore.success('Visualization transform triggered successfully');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger visualization transform'));
		}
	}

	function openEditForm(transform: VisualizationTransform) {
		editingTransform = transform;
		newTitle = transform.title;
		newEmbeddedDatasetId = transform.embedded_dataset_id;
		config = { ...transform.visualization_config };
		showCreateForm = true;
	}

	function resetForm() {
		newTitle = '';
		newEmbeddedDatasetId = null;
		config = {
			n_neighbors: 15,
			n_components: 3,
			min_dist: 0.1,
			metric: 'cosine',
			min_cluster_size: 10,
			min_samples: null,
		};
		showCreateForm = false;
		editingTransform = null;
		createError = null;
	}

	function requestDeleteTransform(transform: VisualizationTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.visualization_transform_id;
			const response = await fetch(
				`/api/visualization-transforms/${target.visualization_transform_id}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to delete visualization transform: ${response.statusText}`);
			}

			transforms = transforms.filter(
				(t) => t.visualization_transform_id !== target.visualization_transform_id
			);
			toastStore.success('Visualization transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete visualization transform'));
		} finally {
			deleting = null;
		}
	}

	onMount(() => {
		fetchTransforms();
		fetchEmbeddedDatasets();
	});

	let filteredTransforms = $derived(
		transforms.filter((t) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return t.title.toLowerCase().includes(query) || t.owner.toLowerCase().includes(query);
		})
	);

	function getEmbeddedDatasetTitle(embeddedDatasetId: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === embeddedDatasetId);
		return dataset
			? `${dataset.title} (${dataset.embedder_name})`
			: `Embedded Dataset ${embeddedDatasetId}`;
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Visualization Transforms"
		description="Create 3D visualizations of Embedded Datasets using UMAP dimensionality reduction and HDBSCAN clustering. Visualizations help explore semantic relationships and discover topics in your data."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Visualization Transforms</h1>
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
			{showCreateForm ? 'Cancel' : 'Create Visualization Transform'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingTransform ? 'Edit Visualization Transform' : 'Create New Visualization Transform'}
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
						placeholder="Enter visualization title..."
					/>
				</div>

				{#if !editingTransform}
					<div class="mb-4">
						<label
							for="embedded-dataset-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Embedded Dataset
						</label>
						<select
							id="embedded-dataset-select"
							bind:value={newEmbeddedDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select an embedded dataset...</option>
							{#each embeddedDatasets as dataset (dataset.embedded_dataset_id)}
								<option value={dataset.embedded_dataset_id}>
									{dataset.title} - {dataset.embedder_name} ({dataset.source_dataset_title})
								</option>
							{/each}
						</select>
					</div>
				{/if}

				<div
					class="mb-6 p-4 bg-blue-50 dark:bg-blue-900/10 border border-blue-200 dark:border-blue-800 rounded-lg"
				>
					<h3 class="text-sm font-semibold text-blue-900 dark:text-blue-300 mb-3">
						UMAP Parameters (Dimensionality Reduction)
					</h3>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="n-neighbors"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								N Neighbors
							</label>
							<input
								id="n-neighbors"
								type="number"
								bind:value={config.n_neighbors}
								min="2"
								max="200"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Controls local vs global structure (2-200, default: 15)
							</p>
						</div>

						<div>
							<label
								for="n-components"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								N Components
							</label>
							<input
								id="n-components"
								type="number"
								bind:value={config.n_components}
								min="2"
								max="3"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Target dimensions (2 or 3, default: 3)
							</p>
						</div>

						<div>
							<label
								for="min-dist"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Distance
							</label>
							<input
								id="min-dist"
								type="number"
								bind:value={config.min_dist}
								min="0.0"
								max="1.0"
								step="0.01"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Minimum distance between points (0.0-1.0, default: 0.1)
							</p>
						</div>

						<div>
							<label
								for="metric"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Metric
							</label>
							<select
								id="metric"
								bind:value={config.metric}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							>
								<option value="cosine">Cosine</option>
								<option value="euclidean">Euclidean</option>
								<option value="manhattan">Manhattan</option>
							</select>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Distance metric (default: cosine)
							</p>
						</div>
					</div>
				</div>

				<div
					class="mb-4 p-4 bg-purple-50 dark:bg-purple-900/10 border border-purple-200 dark:border-purple-800 rounded-lg"
				>
					<h3 class="text-sm font-semibold text-purple-900 dark:text-purple-300 mb-3">
						HDBSCAN Parameters (Clustering)
					</h3>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="min-cluster-size"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Cluster Size
							</label>
							<input
								id="min-cluster-size"
								type="number"
								bind:value={config.min_cluster_size}
								min="2"
								max="500"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Minimum points to form a cluster (2-500, default: 10)
							</p>
						</div>

						<div>
							<label
								for="min-samples"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Samples (Optional)
							</label>
							<input
								id="min-samples"
								type="number"
								bind:value={config.min_samples}
								min="1"
								max="500"
								placeholder="Auto (same as min_cluster_size)"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Conservative clustering (leave empty for auto)
							</p>
						</div>
					</div>
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
			placeholder="Search visualization transforms..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading visualization transforms...</p>
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
					? 'No visualization transforms found matching your search.'
					: 'No visualization transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredTransforms as transform (transform.visualization_transform_id)}
				{@const stats = statsMap.get(transform.visualization_transform_id)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">
								{transform.title}
							</h3>
							<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
								<p>
									<strong>Embedded Dataset:</strong>
									{getEmbeddedDatasetTitle(transform.embedded_dataset_id)}
								</p>
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
								{#if transform.reduced_collection_name}
									<p>
										<strong>3D Points Collection:</strong>
										<code class="px-2 py-1 bg-gray-100 dark:bg-gray-700 rounded text-xs font-mono">
											{transform.reduced_collection_name}
										</code>
									</p>
								{/if}
								{#if transform.topics_collection_name}
									<p>
										<strong>Topics Collection:</strong>
										<code class="px-2 py-1 bg-gray-100 dark:bg-gray-700 rounded text-xs font-mono">
											{transform.topics_collection_name}
										</code>
									</p>
								{/if}
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
								onclick={() => triggerTransform(transform.visualization_transform_id)}
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
								disabled={deleting === transform.visualization_transform_id}
								class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{deleting === transform.visualization_transform_id ? 'Deleting...' : 'Delete'}
							</button>
						</div>
					</div>

					<div
						class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-2 gap-4"
					>
						<div>
							<p class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">UMAP Config</p>
							<div class="text-xs text-gray-600 dark:text-gray-400 space-y-1">
								<p>Neighbors: {transform.visualization_config.n_neighbors}</p>
								<p>Components: {transform.visualization_config.n_components}</p>
								<p>Min Distance: {transform.visualization_config.min_dist}</p>
								<p>Metric: {transform.visualization_config.metric}</p>
							</div>
						</div>
						<div>
							<p class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
								HDBSCAN Config
							</p>
							<div class="text-xs text-gray-600 dark:text-gray-400 space-y-1">
								<p>Min Cluster Size: {transform.visualization_config.min_cluster_size}</p>
								<p>
									Min Samples: {transform.visualization_config.min_samples ?? 'Auto'}
								</p>
							</div>
						</div>
					</div>

					{#if stats}
						<div
							class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-3 gap-4"
						>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Total Points</p>
								<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
									{stats.total_points}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Clusters</p>
								<p class="text-lg font-semibold text-purple-600 dark:text-purple-400">
									{stats.total_clusters}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Noise Points</p>
								<p class="text-lg font-semibold text-gray-600 dark:text-gray-400">
									{stats.noise_points}
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
	title="Delete Visualization Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>
