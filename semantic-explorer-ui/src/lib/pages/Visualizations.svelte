<script lang="ts">
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import ActionMenu from '../components/ActionMenu.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateVisualizationTransformModal from '../components/CreateVisualizationTransformModal.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import type {
		VisualizationConfig,
		VisualizationTransform,
		VisualizationRun,
	} from '../types/visualizations';

	interface Props {
		onViewVisualization?: (_id: number) => void;
	}

	let { onViewVisualization }: Props = $props();

	let visualizations = $state<VisualizationTransform[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let searchQuery = $state('');

	let visualizationPendingDelete = $state<VisualizationTransform | null>(null);

	let transformModalOpen = $state(false);
	let selectedEmbeddedDatasetIdForTransform = $state<number | null>(null);

	onMount(async () => {
		await loadVisualizations();
	});

	async function loadVisualizations() {
		loading = true;
		error = null;

		try {
			const response = await fetch('/api/visualization-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			visualizations = await response.json();
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

	function handleCreateTransform(embeddedDatasetId: number) {
		selectedEmbeddedDatasetIdForTransform = embeddedDatasetId;
		transformModalOpen = true;
	}

	function handleTransformCreated() {
		transformModalOpen = false;
		selectedEmbeddedDatasetIdForTransform = null;
		loadVisualizations();
		toastStore.success('Visualization transform created successfully');
	}

	function requestDeleteVisualization(viz: VisualizationTransform) {
		visualizationPendingDelete = viz;
	}

	async function confirmDeleteVisualization() {
		if (!visualizationPendingDelete) return;

		try {
			const response = await fetch(
				`/api/visualization-transforms/${visualizationPendingDelete.visualization_transform_id}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to delete visualization: ${response.statusText}`);
			}

			toastStore.success('Visualization deleted');
			visualizationPendingDelete = null;
			await loadVisualizations();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete visualization'));
		}
	}

	async function toggleEnabled(viz: VisualizationTransform) {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${viz.visualization_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: !viz.is_enabled,
					}),
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to update visualization: ${response.statusText}`);
			}

			toastStore.success(`Visualization ${viz.is_enabled ? 'disabled' : 'enabled'}`);
			await loadVisualizations();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to update visualization'));
		}
	}

	async function triggerRun(viz: VisualizationTransform) {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${viz.visualization_transform_id}/trigger`,
				{
					method: 'POST',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to trigger visualization: ${response.statusText}`);
			}

			toastStore.success('Visualization run triggered');
			await loadVisualizations();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger visualization'));
		}
	}

	async function downloadLatestHtml(viz: VisualizationTransform) {
		try {
			// First, get the latest run
			const runsResponse = await fetch(
				`/api/visualization-transforms/${viz.visualization_transform_id}/runs?limit=1`
			);
			if (!runsResponse.ok) {
				throw new Error(`Failed to fetch runs: ${runsResponse.statusText}`);
			}

			const runs: VisualizationRun[] = await runsResponse.json();
			if (runs.length === 0 || !runs[0].html_s3_key) {
				toastStore.error('No HTML file available for this visualization');
				return;
			}

			const run = runs[0];

			// Download the HTML file
			const downloadResponse = await fetch(
				`/api/visualization-transforms/${viz.visualization_transform_id}/runs/${run.run_id}/download`
			);

			if (!downloadResponse.ok) {
				throw new Error(`Failed to download HTML: ${downloadResponse.statusText}`);
			}

			// Create a blob and download it
			const blob = await downloadResponse.blob();
			const url = window.URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `visualization-${viz.visualization_transform_id}-${run.run_id}.html`;
			document.body.appendChild(a);
			a.click();
			window.URL.revokeObjectURL(url);
			document.body.removeChild(a);

			toastStore.success('HTML file downloaded');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to download HTML'));
		}
	}

	function getStatusBadge(status: string | null): { color: string; text: string } {
		if (!status) return { color: 'gray', text: 'Unknown' };

		switch (status) {
			case 'completed':
				return { color: 'green', text: 'Completed' };
			case 'processing':
				return { color: 'blue', text: 'Processing' };
			case 'failed':
				return { color: 'red', text: 'Failed' };
			case 'pending':
				return { color: 'yellow', text: 'Pending' };
			default:
				return { color: 'gray', text: status };
		}
	}

	let filteredVisualizations = $derived(
		visualizations.filter((v) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				v.title.toLowerCase().includes(query) ||
				v.owner.toLowerCase().includes(query) ||
				v.embedded_dataset_id.toString().includes(query)
			);
		})
	);
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Visualizations"
		description="Interactive visualizations of embedding spaces with UMAP dimensionality reduction and HDBSCAN clustering. Visualizations can be generated from embedded datasets using transforms."
	/>

	<div class="flex justify-between items-center mb-4">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Visualizations</h1>
	</div>

	{#if !loading && visualizations.length > 0}
		<div class="mb-4">
			<div class="relative">
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search visualizations by title, owner, or embedded dataset..."
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
				onclick={loadVisualizations}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if visualizations.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
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
			<p class="text-gray-500 dark:text-gray-400 mb-4">No visualizations yet</p>
			<p class="text-sm text-gray-600 dark:text-gray-400">
				Create visualization transforms from embedded datasets in the Transforms section.
			</p>
		</div>
	{:else if filteredVisualizations.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No visualizations match your search</p>
			<button
				onclick={() => (searchQuery = '')}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Clear search
			</button>
		</div>
	{:else}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Title</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Embedded Dataset</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Status</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Statistics</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Config</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredVisualizations as viz (viz.visualization_transform_id)}
						{@const statusBadge = getStatusBadge(viz.last_run_status)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-2">
								<div>
									<button
										onclick={() => handleView(viz.visualization_transform_id)}
										class="font-medium text-blue-600 dark:text-blue-400 hover:underline"
									>
										{viz.title}
									</button>
									<div class="text-xs text-gray-600 dark:text-gray-400 mt-1">
										{viz.is_enabled ? '✓ Enabled' : '✗ Disabled'}
									</div>
								</div>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								<span
									class="inline-block px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-sm font-medium"
								>
									ED #{viz.embedded_dataset_id}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								<span
									class="inline-block px-2 py-1 bg-{statusBadge.color}-100 dark:bg-{statusBadge.color}-900/30 text-{statusBadge.color}-700 dark:text-{statusBadge.color}-300 rounded text-sm font-medium"
								>
									{statusBadge.text}
								</span>
								{#if viz.last_run_at}
									<div class="text-xs text-gray-600 dark:text-gray-400 mt-1">
										{formatDate(viz.last_run_at)}
									</div>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								{#if viz.last_run_stats}
									<div class="text-sm">
										{#if viz.last_run_stats.point_count !== undefined}
											<div class="text-gray-700 dark:text-gray-300">
												{viz.last_run_stats.point_count.toLocaleString()} points
											</div>
										{/if}
										{#if viz.last_run_stats.cluster_count !== undefined}
											<div class="text-gray-600 dark:text-gray-400 text-xs">
												{viz.last_run_stats.cluster_count} clusters
											</div>
										{/if}
										{#if viz.last_run_stats.processing_duration_ms !== undefined}
											<div class="text-gray-500 dark:text-gray-500 text-xs">
												{(viz.last_run_stats.processing_duration_ms / 1000).toFixed(1)}s
											</div>
										{/if}
									</div>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2">
								<div class="text-xs space-y-0.5">
									<div class="text-gray-700 dark:text-gray-300">
										{viz.visualization_config.n_neighbors} neighbors
									</div>
									<div class="text-gray-600 dark:text-gray-400">
										{viz.visualization_config.metric}, min_cluster={viz.visualization_config
											.min_cluster_size}
									</div>
								</div>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								<ActionMenu
									actions={[
										{
											label: 'View Details',
											handler: () => handleView(viz.visualization_transform_id),
										},
										{
											label: 'Create Transform',
											handler: () => handleCreateTransform(viz.embedded_dataset_id),
										},
										{
											label: 'Trigger Run',
											handler: () => triggerRun(viz),
										},
										...(viz.last_run_status === 'completed'
											? [
													{
														label: 'Download HTML',
														handler: () => downloadLatestHtml(viz),
													},
												]
											: []),
										{
											label: viz.is_enabled ? 'Disable' : 'Enable',
											handler: () => toggleEnabled(viz),
										},
										{
											label: 'Delete',
											handler: () => requestDeleteVisualization(viz),
											isDangerous: true,
										},
									]}
								/>
							</TableBodyCell>
						</tr>
					{/each}
				</TableBody>
			</Table>
		</div>
	{/if}
</div>

<ConfirmDialog
	open={visualizationPendingDelete !== null}
	title="Delete visualization"
	message={visualizationPendingDelete
		? `Are you sure you want to delete "${visualizationPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteVisualization}
	on:cancel={() => (visualizationPendingDelete = null)}
/>

{#if transformModalOpen && selectedEmbeddedDatasetIdForTransform !== null}
	<CreateVisualizationTransformModal
		isOpen={transformModalOpen}
		presetEmbeddedDatasetId={selectedEmbeddedDatasetIdForTransform}
		onClose={() => {
			transformModalOpen = false;
			selectedEmbeddedDatasetIdForTransform = null;
		}}
		onSuccess={handleTransformCreated}
	/>
{/if}
