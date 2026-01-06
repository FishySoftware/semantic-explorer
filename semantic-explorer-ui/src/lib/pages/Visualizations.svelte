<script lang="ts">
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Transform {
		transform_id: number;
		title: string;
		job_type: string;
		dataset_id: number;
		source_transform_id: number | null;
		embedder_ids?: number[] | null;
		job_config: any;
		updated_at: string;
	}

	interface Props {
		onViewVisualization?: (_id: number) => void;
	}

	let { onViewVisualization }: Props = $props();

	let visualizations = $state<Transform[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(async () => {
		await loadVisualizations();
	});

	async function loadVisualizations() {
		loading = true;
		error = null;

		try {
			const response = await fetch('/api/transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch transforms: ${response.statusText}`);
			}
			const allTransforms: Transform[] = await response.json();

			// Filter for visualization transforms only
			visualizations = allTransforms.filter(
				(t) => t.job_type === 'dataset_visualization_transform'
			);
		} catch (err) {
			error = formatError(err);
			toastStore.error(error);
		} finally {
			loading = false;
		}
	}

	function formatDate(dateString: string): string {
		return new Date(dateString).toLocaleDateString('en-US', {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit',
		});
	}

	function handleView(transformId: number) {
		if (onViewVisualization) {
			onViewVisualization(transformId);
		}
	}
</script>

<div class="max-w-7xl mx-auto">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">Embedding Visualizations</h1>
		<p class="text-gray-600 dark:text-gray-400">
			3D visualizations of embedding spaces with UMAP dimensionality reduction and HDBSCAN
			clustering
		</p>
	</div>

	{#if loading}
		<div class="flex items-center justify-center h-64">
			<div class="text-center">
				<svg
					class="animate-spin h-12 w-12 text-blue-600 dark:text-blue-400 mx-auto mb-4"
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
				>
					<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
					></circle>
					<path
						class="opacity-75"
						fill="currentColor"
						d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
					></path>
				</svg>
				<p class="text-gray-600 dark:text-gray-400">Loading visualizations...</p>
			</div>
		</div>
	{:else if error}
		<div class="bg-red-50 dark:bg-red-900/20 border-l-4 border-red-400 p-4 rounded-lg">
			<p class="text-red-700 dark:text-red-400">{error}</p>
		</div>
	{:else if visualizations.length === 0}
		<div class="text-center py-12">
			<svg
				class="mx-auto h-12 w-12 text-gray-400 dark:text-gray-600 mb-4"
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
				></path>
			</svg>
			<h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">No visualizations yet</h3>
			<p class="text-gray-600 dark:text-gray-400 mb-4">
				Create a visualization transform in the Transforms section to get started.
			</p>
		</div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each visualizations as viz (viz.transform_id)}
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow hover:shadow-lg transition-shadow overflow-hidden"
				>
					<div class="p-6">
						<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">
							{viz.title}
						</h3>

						<div class="space-y-2 mb-4">
							<div class="text-sm text-gray-600 dark:text-gray-400">
								<span class="font-medium">Dataset ID:</span>
								{viz.dataset_id}
							</div>
							{#if viz.embedder_ids && viz.embedder_ids.length > 0}
								<div class="text-sm text-gray-600 dark:text-gray-400">
									<span class="font-medium">Embedder ID:</span>
									{viz.embedder_ids[0]}
								</div>
							{/if}
							<div class="text-sm text-gray-600 dark:text-gray-400">
								<span class="font-medium">Updated:</span>
								{formatDate(viz.updated_at)}
							</div>
						</div>

						{#if viz.job_config}
							<div class="mb-4">
								<div class="text-xs font-medium text-gray-500 dark:text-gray-500 mb-1">
									Configuration
								</div>
								<div class="text-xs text-gray-600 dark:text-gray-400 space-y-1">
									<div>UMAP neighbors: {viz.job_config.n_neighbors || 15}</div>
									<div>
										HDBSCAN min cluster size: {viz.job_config.min_cluster_size || 5}
									</div>
								</div>
							</div>
						{/if}

						<button
							onclick={() => handleView(viz.transform_id)}
							class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors flex items-center justify-center gap-2"
						>
							<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
								></path>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
								></path>
							</svg>
							View 3D Visualization
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
