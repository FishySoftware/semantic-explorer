<script lang="ts">
	import {
		Badge,
		Spinner,
		Table,
		TableBody,
		TableBodyCell,
		TableHead,
		TableHeadCell,
	} from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import ActionMenu from '../components/ActionMenu.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import type { Visualization, VisualizationTransform } from '../types/visualizations';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		onViewVisualization?: (_id: number) => void;
	}

	let { onViewVisualization }: Props = $props();

	let transforms = $state<VisualizationTransform[]>([]);
	let completedVisualizations = $state.raw(new SvelteMap<number, Visualization>());
	let loading = $state(true);
	let error = $state<string | null>(null);
	let searchQuery = $state('');
	let pollInterval: number | null = null;
	let transformPendingDelete = $state<VisualizationTransform | null>(null);

	onMount(async () => {
		await loadTransforms();
		startPolling();
	});

	onDestroy(() => {
		stopPolling();
	});

	function startPolling() {
		// Poll for updates every 3 seconds if any transforms are processing or pending
		pollInterval = window.setInterval(async () => {
			const hasProcessing = transforms.some(
				(t) => t.last_run_status === 'processing' || t.last_run_status === 'pending'
			);
			if (hasProcessing) {
				await loadTransforms();
			}
		}, 3000);
	}

	function stopPolling() {
		if (pollInterval !== null) {
			clearInterval(pollInterval);
			pollInterval = null;
		}
	}

	async function loadTransforms() {
		if (!loading) {
			// Silent refresh for polling - don't show loading state
		}
		error = null;

		try {
			const response = await fetch('/api/visualization-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			transforms = await response.json();

			// Load completed visualizations for each transform
			await loadCompletedVisualizations();
		} catch (err) {
			error = formatError(err);
			toastStore.error(error);
		} finally {
			loading = false;
		}
	}

	async function loadCompletedVisualizations() {
		const newCompletedVisualizations = new SvelteMap<number, Visualization>();

		for (const transform of transforms) {
			try {
				const response = await fetch(
					`/api/visualization-transforms/${transform.visualization_transform_id}/visualizations?limit=50`
				);
				if (response.ok) {
					const visualizations: Visualization[] = await response.json();
					// Get the most recent completed visualization
					const completed = visualizations.find((v) => v.status === 'completed');
					if (completed) {
						newCompletedVisualizations.set(transform.visualization_transform_id, completed);
					}
				}
			} catch (err) {
				console.error(
					`Failed to load visualizations for transform ${transform.visualization_transform_id}:`,
					err
				);
			}
		}

		completedVisualizations = newCompletedVisualizations;
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

	function requestDeleteTransform(transform: VisualizationTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) return;

		try {
			const response = await fetch(
				`/api/visualization-transforms/${transformPendingDelete.visualization_transform_id}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to delete visualization transform: ${response.statusText}`);
			}

			toastStore.success('Visualization transform deleted');
			transformPendingDelete = null;
			await loadTransforms();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete visualization transform'));
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
			await loadTransforms();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger visualization'));
		}
	}

	async function downloadLatestHtml(viz: VisualizationTransform) {
		try {
			// First, get the latest completed visualization
			const runsResponse = await fetch(
				`/api/visualization-transforms/${viz.visualization_transform_id}/visualizations?limit=50`
			);
			if (!runsResponse.ok) {
				throw new Error(`Failed to fetch visualizations: ${runsResponse.statusText}`);
			}

			const visualizations: Visualization[] = await runsResponse.json();
			const completed = visualizations.find((v) => v.status === 'completed');

			if (!completed || !completed.html_s3_key) {
				toastStore.error('No HTML file available for this visualization');
				return;
			}

			// Download the HTML file
			const downloadResponse = await fetch(
				`/api/visualization-transforms/${viz.visualization_transform_id}/visualizations/${completed.visualization_id}/download`
			);

			if (!downloadResponse.ok) {
				throw new Error(`Failed to download HTML: ${downloadResponse.statusText}`);
			}

			// Create a blob and download it
			const blob = await downloadResponse.blob();
			const url = window.URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `visualization-${viz.visualization_transform_id}-${completed.visualization_id}.html`;
			document.body.appendChild(a);
			a.click();
			window.URL.revokeObjectURL(url);
			document.body.removeChild(a);

			toastStore.success('HTML file downloaded');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to download HTML'));
		}
	}

	let filteredTransforms = $derived(
		transforms.filter((v) => {
			// Only show transforms that have a completed visualization
			if (!completedVisualizations.has(v.visualization_transform_id)) return false;

			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				v.title.toLowerCase().includes(query) ||
				v.owner.toLowerCase().includes(query) ||
				v.embedded_dataset_id.toString().includes(query)
			);
		})
	);

	let pendingTransforms = $derived(
		transforms.filter((t) => {
			const hasPending = t.last_run_status === 'pending' || t.last_run_status === 'processing';
			return hasPending && !completedVisualizations.has(t.visualization_transform_id);
		})
	);

	let processingTransforms = $derived(
		transforms.filter((t) => {
			const hasCompleted = completedVisualizations.has(t.visualization_transform_id);
			const isProcessing = t.last_run_status === 'pending' || t.last_run_status === 'processing';
			return hasCompleted && isProcessing;
		})
	);
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Visualizations"
		description="View completed interactive visualizations of your embedding spaces. Each visualization shows UMAP dimensionality reduction with HDBSCAN clustering."
	/>

	<!-- Pending Visualizations Status Tracker -->
	{#if pendingTransforms.length > 0 || processingTransforms.length > 0}
		<div
			class="mb-8 bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden border border-gray-200 dark:border-gray-700"
		>
			<!-- Header Section -->
			<div
				class="bg-linear-to-r from-blue-50 to-blue-100 dark:from-blue-900/30 dark:to-blue-800/30 px-6 py-4 border-b border-blue-200 dark:border-blue-700"
			>
				<div class="flex items-center gap-3 mb-1">
					<Spinner size="5" color="blue" />
					<h2 class="text-xl font-bold text-blue-900 dark:text-blue-100">
						Processing Visualizations
					</h2>
				</div>
				<p class="text-sm text-blue-700 dark:text-blue-300 mt-2">
					{pendingTransforms.length + processingTransforms.length} visualization{pendingTransforms.length +
						processingTransforms.length !==
					1
						? 's'
						: ''} in progress · Updates automatically
				</p>
			</div>

			<!-- Content Section -->
			<div class="divide-y divide-gray-200 dark:divide-gray-700">
				{#each [...pendingTransforms, ...processingTransforms] as transform (transform.visualization_transform_id)}
					<div class="px-6 py-4 hover:bg-gray-50 dark:hover:bg-gray-700/30 transition-colors">
						<div class="flex items-center justify-between">
							<div class="flex-1">
								<div class="flex items-center gap-2 mb-2">
									<h3 class="font-semibold text-gray-900 dark:text-white text-base">
										{transform.title}
									</h3>
									<Badge
										color={transform.last_run_status === 'processing' ? 'blue' : 'yellow'}
										class="text-xs"
									>
										{transform.last_run_status === 'processing' ? 'Processing' : 'Pending'}
									</Badge>
								</div>
								<div
									class="flex flex-wrap items-center gap-3 text-sm text-gray-600 dark:text-gray-400"
								>
									{#if transform.last_run_at}
										<span>Started {formatDate(transform.last_run_at)}</span>
									{/if}
									{#if transform.last_run_stats?.point_count}
										<span class="flex items-center">
											<span class="inline-block w-1 h-1 bg-gray-400 rounded-full mx-2"></span>
											{transform.last_run_stats.point_count.toLocaleString()} points
										</span>
									{/if}
								</div>
								{#if transform.last_error}
									<div class="text-sm text-red-600 dark:text-red-400 mt-2">
										<span class="font-medium">Error:</span>
										{transform.last_error}
									</div>
								{/if}
							</div>
						</div>
					</div>
				{/each}
			</div>

			<!-- Footer Message -->
			<div
				class="bg-blue-50 dark:bg-blue-900/10 px-6 py-3 text-sm text-blue-700 dark:text-blue-300"
			>
				<span class="inline-flex items-center gap-1">
					<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
						<path
							fill-rule="evenodd"
							d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z"
							clip-rule="evenodd"
						/>
					</svg>
					Visualizations will appear in the list below once generation is complete.
				</span>
			</div>
		</div>
	{/if}

	{#if !loading && transforms.length > 0}
		<div class="mb-4">
			<div class="relative">
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search visualization transforms by title, owner, or embedded dataset..."
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
				onclick={loadTransforms}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if transforms.length === 0}
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
			<p class="text-gray-500 dark:text-gray-400 mb-4">No completed visualizations yet</p>
			<p class="text-sm text-gray-600 dark:text-gray-400">
				Create visualization transforms from embedded datasets to generate interactive
				visualizations. Once complete, they'll appear here.
			</p>
		</div>
	{:else if filteredTransforms.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">
				No completed visualizations match your search
			</p>
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
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Completed</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Statistics</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Config</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredTransforms as viz (viz.visualization_transform_id)}
						{@const completedViz = completedVisualizations.get(viz.visualization_transform_id)}
						{@const isProcessing =
							viz.last_run_status === 'processing' || viz.last_run_status === 'pending'}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-2">
								<div>
									<button
										onclick={() => handleView(viz.visualization_transform_id)}
										class="font-medium text-blue-600 dark:text-blue-400 hover:underline"
									>
										{viz.title}
									</button>
									<div
										class="text-xs text-gray-600 dark:text-gray-400 mt-1 flex items-center gap-2"
									>
										{#if isProcessing}
											<Badge color="blue" class="text-xs">New version processing</Badge>
										{/if}
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
								{#if completedViz}
									<Badge color="green">Completed</Badge>
									<div class="text-xs text-gray-600 dark:text-gray-400 mt-1">
										{formatDate(completedViz.completed_at || completedViz.created_at)}
									</div>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								{#if completedViz}
									<div class="text-sm">
										{#if completedViz.point_count !== undefined && completedViz.point_count !== null}
											<div class="text-gray-700 dark:text-gray-300">
												{completedViz.point_count.toLocaleString()} points
											</div>
										{/if}
										{#if completedViz.cluster_count !== undefined && completedViz.cluster_count !== null}
											<div class="text-gray-600 dark:text-gray-400 text-xs">
												{completedViz.cluster_count} clusters
											</div>
										{/if}
										{#if completedViz.stats_json?.processing_duration_ms && typeof completedViz.stats_json.processing_duration_ms === 'number'}
											<div class="text-gray-500 dark:text-gray-500 text-xs">
												{(completedViz.stats_json.processing_duration_ms / 1000).toFixed(1)}s
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
											label: 'Trigger Run',
											handler: () => triggerRun(viz),
										},
										{
											label: 'Download HTML',
											handler: () => downloadLatestHtml(viz),
										},
										{
											label: 'Delete',
											handler: () => requestDeleteTransform(viz),
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
	open={transformPendingDelete !== null}
	title="Delete Visualization Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This will also delete all associated visualizations. This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>
