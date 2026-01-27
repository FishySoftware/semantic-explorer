<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import { formatDate } from '../utils/ui-helpers';
	import PageHeader from '../components/PageHeader.svelte';

	interface Props {
		visualizationTransformId: number;
		onBack: () => void;
		onNavigate?: (_page: string, _params?: Record<string, unknown>) => void;
	}

	let { visualizationTransformId, onBack, onNavigate }: Props = $props();

	interface VisualizationTransform {
		visualization_transform_id: number;
		title: string;
		embedded_dataset_id: number;
		owner: string;
		is_enabled: boolean;
		reduced_collection_name: string | null;
		topics_collection_name: string | null;
		visualization_config: any;
		last_run_status: string | null;
		last_run_at: string | null;
		last_error: string | null;
		last_run_stats: any | null;
		created_at: string;
		updated_at: string;
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
	}

	interface Stats {
		visualization_transform_id: number;
		latest_visualization: DatabaseVisualization | null;
		total_runs: number;
		successful_runs: number;
		failed_runs: number;
	}

	// Database format from API
	interface DatabaseVisualization {
		visualization_id: number;
		visualization_transform_id: number;
		status: string;
		started_at: string | null;
		completed_at: string | null;
		html_s3_key: string | null;
		point_count: number | null;
		cluster_count: number | null;
		error_message: string | null;
		stats_json: Record<string, unknown> | null;
		created_at: string;
	}

	// UI format
	interface Visualization {
		visualization_id: number;
		visualization_transform_id: number;
		title: string;
		embedding_count: number;
		cluster_count: number;
		created_at: string;
		updated_at: string;
		status: string;
		started_at: string | null;
		completed_at: string | null;
		error_message: string | null;
		stats_json: Record<string, unknown> | null;
	}

	let transform = $state<VisualizationTransform | null>(null);
	let embeddedDataset = $state<EmbeddedDataset | null>(null);
	let stats = $state<Stats | null>(null);
	let visualizations = $state<Visualization[]>([]);
	let totalVisualizationsCount = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Pagination for visualizations
	let visualizationsCurrentPage = $state(1);
	let visualizationsPageSize = $state(20);

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let isMounted = false; // Track if component is still mounted

	async function fetchTransform() {
		try {
			const response = await fetch(`/api/visualization-transforms/${visualizationTransformId}`, {
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transform: ${response.statusText}`);
			}

			transform = await response.json();

			// Fetch the embedded dataset details
			if (transform?.embedded_dataset_id) {
				await fetchEmbeddedDataset(transform.embedded_dataset_id);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Unknown error occurred';
			console.error('Error fetching visualization transform:', e);
		}
	}

	async function fetchEmbeddedDataset(id: number) {
		try {
			const response = await fetch(`/api/embedded-datasets/${id}`, {
				credentials: 'include',
			});

			if (response.ok) {
				embeddedDataset = await response.json();
			}
		} catch (e) {
			console.error('Error fetching embedded dataset:', e);
		}
	}

	async function fetchStats() {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${visualizationTransformId}/stats`,
				{
					credentials: 'include',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to fetch stats: ${response.statusText}`);
			}

			stats = await response.json();
		} catch (e) {
			console.error('Error fetching stats:', e);
		}
	}

	async function fetchVisualizations() {
		try {
			const offset = (visualizationsCurrentPage - 1) * visualizationsPageSize;
			const response = await fetch(
				`/api/visualization-transforms/${visualizationTransformId}/visualizations?limit=${visualizationsPageSize}&offset=${offset}`,
				{
					credentials: 'include',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to fetch visualizations: ${response.statusText}`);
			}

			const dbVisualizations = (await response.json()) as DatabaseVisualization[];

			// Transform from database format to UI format
			visualizations = dbVisualizations.map((v) => ({
				...v, // Preserve all fields from database
				title: `${transform?.title || 'Visualization'} - ${new Date(v.created_at).toISOString().split('T')[0]}`,
				embedding_count: v.point_count ?? 0,
				cluster_count: v.cluster_count ?? 0,
				updated_at: v.completed_at ?? v.started_at ?? v.created_at,
			}));

			totalVisualizationsCount = visualizations.length;
		} catch (e) {
			console.error('Error fetching visualizations:', e);
		}
	}

	function getVisualizationsTotalPages(): number {
		if (totalVisualizationsCount <= 0 || visualizationsPageSize <= 0) return 1;
		return Math.ceil(totalVisualizationsCount / visualizationsPageSize);
	}

	function handleVisualizationsPageChange(page: number) {
		if (page < 1 || page > getVisualizationsTotalPages()) return;
		visualizationsCurrentPage = page;
		fetchVisualizations();
	}

	function connectSSE() {
		// Close existing connection first
		disconnectSSE();

		try {
			eventSource = new EventSource('/api/visualization-transforms/stream');

			eventSource.addEventListener('heartbeat', () => {
				// Keep connection alive
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// If this is an update for our transform, refresh stats and visualizations
					// API sends transform_id (generic) not visualization_transform_id
					if (statusUpdate.transform_id === visualizationTransformId) {
						fetchStats();
						fetchVisualizations();
					}
				} catch (e) {
					console.error('Failed to parse SSE status event:', e);
				}
			});

			eventSource.onerror = () => {
				eventSource?.close();
				eventSource = null;
				reconnectSSE();
			};

			reconnectAttempts = 0;
		} catch (e) {
			console.error('Failed to connect to SSE stream:', e);
			reconnectSSE();
		}
	}

	function reconnectSSE() {
		if (!isMounted) {
			// Component has been unmounted, don't attempt reconnection
			return;
		}

		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error('Max SSE reconnection attempts reached');
			return;
		}

		const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 60000);
		reconnectAttempts++;

		reconnectTimer = setTimeout(() => {
			if (isMounted) {
				connectSSE();
			}
		}, delay);
	}

	function disconnectSSE() {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}
		reconnectAttempts = 0;
	}

	onMount(async () => {
		isMounted = true;
		loading = true;
		await Promise.all([fetchTransform(), fetchStats(), fetchVisualizations()]);
		loading = false;
		connectSSE();
	});

	onDestroy(() => {
		isMounted = false;
		disconnectSSE();
	});
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Visualization Transform Details"
		description="View detailed information, clustering progress, and statistics for this visualization transform."
	/>

	<div class="mb-6">
		<button
			onclick={onBack}
			class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white transition-colors flex items-center gap-2"
		>
			← Back to Visualization Transforms
		</button>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading visualization transform details...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if transform}
		<!-- Transform Info Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<div class="flex justify-between items-start mb-4">
				<div>
					<Heading tag="h2" class="text-2xl font-bold mb-2">{transform.title}</Heading>
					<p class="text-sm text-gray-500 dark:text-gray-400">
						Created {formatDate(transform.created_at)}
					</p>
				</div>
				<span
					class={transform.is_enabled
						? 'px-3 py-1 rounded-full text-sm font-semibold bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
						: 'px-3 py-1 rounded-full text-sm font-semibold bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'}
				>
					{transform.is_enabled ? 'Enabled' : 'Disabled'}
				</span>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Embedded Dataset</p>
					{#if embeddedDataset}
						<button
							onclick={() =>
								onNavigate?.('embedded-dataset-detail', {
									embeddedDatasetId: transform?.embedded_dataset_id,
								})}
							class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
						>
							{embeddedDataset.title}
						</button>
					{:else}
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							ID: {transform.embedded_dataset_id}
						</p>
					{/if}
				</div>
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Owner</p>
					<p class="text-lg font-medium text-gray-900 dark:text-white">{transform.owner}</p>
				</div>
			</div>
		</div>

		<!-- Configuration Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<Heading tag="h3" class="text-lg font-bold mb-4">Visualization Configuration</Heading>
			<div class="space-y-2">
				{#if transform.visualization_config}
					{#each Object.entries(transform.visualization_config) as [key, value] (key)}
						<div class="flex justify-between">
							<span class="text-sm text-gray-500 dark:text-gray-400">{key}:</span>
							<span class="text-sm font-medium text-gray-900 dark:text-white">
								{typeof value === 'object' ? JSON.stringify(value) : value}
							</span>
						</div>
					{/each}
				{:else}
					<p class="text-sm text-gray-500 dark:text-gray-400">No visualization configuration set</p>
				{/if}
			</div>
		</div>

		<!-- Additional Fields -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<Heading tag="h3" class="text-lg font-bold mb-4">Run Status</Heading>
			<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Last Run Status</p>
					<p class="text-lg font-medium text-gray-900 dark:text-white">
						{transform.last_run_status || 'Never run'}
					</p>
				</div>
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Last Run At</p>
					<p class="text-lg font-medium text-gray-900 dark:text-white">
						{transform.last_run_at ? formatDate(transform.last_run_at) : 'N/A'}
					</p>
				</div>
				{#if transform.last_error}
					<div class="md:col-span-2">
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Last Error</p>
						<p class="text-lg font-medium text-red-600 dark:text-red-400">{transform.last_error}</p>
					</div>
				{/if}
				{#if transform.reduced_collection_name}
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Reduced Collection</p>
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							{transform.reduced_collection_name}
						</p>
					</div>
				{/if}
				{#if transform.topics_collection_name}
					<div>
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Topics Collection</p>
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							{transform.topics_collection_name}
						</p>
					</div>
				{/if}
			</div>
		</div>

		<!-- Stats Card -->
		{#if stats}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<Heading tag="h3" class="text-xl font-bold mb-4">Transformation Statistics</Heading>
				<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
					<div class="text-center">
						<p class="text-3xl font-bold text-blue-600 dark:text-blue-400">
							{stats.total_runs}
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Total Runs</p>
					</div>
					<div class="text-center">
						<p class="text-3xl font-bold text-green-600 dark:text-green-400">
							{stats.successful_runs}
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Successful</p>
					</div>
					<div class="text-center">
						<p class="text-3xl font-bold text-red-600 dark:text-red-400">
							{stats.failed_runs}
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Failed</p>
					</div>
					<div class="text-center">
						<p class="text-3xl font-bold text-purple-600 dark:text-purple-400">
							{stats.total_runs > 0
								? ((stats.successful_runs / stats.total_runs) * 100).toFixed(1)
								: 0}%
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Success Rate</p>
					</div>
				</div>
				{#if stats.latest_visualization}
					<div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
						<p class="text-sm text-gray-500 dark:text-gray-400 mb-2">Latest Visualization</p>
						<p class="font-medium text-gray-900 dark:text-white">
							{`${transform?.title || 'Visualization'} - ${new Date(stats.latest_visualization.created_at).toISOString().split('T')[0]}`}
						</p>
						<p class="text-xs text-gray-500 dark:text-gray-400">
							Created {formatDate(stats.latest_visualization.created_at)}
						</p>
					</div>
				{/if}
			</div>
		{/if}

		<!-- Generated Visualizations Table -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
			<Heading tag="h3" class="text-xl font-bold mb-4">Generated Visualizations</Heading>

			{#if visualizations.length === 0}
				<p class="text-center text-gray-500 dark:text-gray-400 py-8">
					No visualizations have been generated yet.
				</p>
			{:else}
				<div class="overflow-x-auto">
					<table class="w-full text-sm text-left text-gray-600 dark:text-gray-400">
						<thead
							class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700"
						>
							<tr>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Title</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Status</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Embeddings</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Clusters</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Duration</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Started At</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Completed At</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Created</th>
							</tr>
						</thead>
						<tbody>
							{#each visualizations as visualization (visualization.visualization_id)}
								<tr
									class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
								>
									<td class="px-4 py-3 font-medium">
										<a
											href={`#/visualizations/${visualization.visualization_id}/details`}
											class="text-blue-600 dark:text-blue-400 hover:underline"
										>
											{visualization.title}
										</a>
									</td>
									<td class="px-4 py-3">
										<span
											class={visualization.status === 'completed'
												? 'px-2 py-1 rounded-full text-xs font-semibold bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
												: visualization.status === 'failed'
													? 'px-2 py-1 rounded-full text-xs font-semibold bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400'
													: visualization.status === 'processing'
														? 'px-2 py-1 rounded-full text-xs font-semibold bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400'
														: 'px-2 py-1 rounded-full text-xs font-semibold bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'}
										>
											{visualization.status}
										</span>
									</td>
									<td class="px-4 py-3">{visualization.embedding_count}</td>
									<td class="px-4 py-3">{visualization.cluster_count}</td>
									<td class="px-4 py-3">
										{#if visualization.started_at && visualization.completed_at}
											{Math.round(
												new Date(visualization.completed_at).getTime() -
													new Date(visualization.started_at).getTime()
											)}ms
										{:else}
											-
										{/if}
									</td>
									<td class="px-4 py-3">
										{visualization.started_at ? formatDate(visualization.started_at) : '-'}
									</td>
									<td class="px-4 py-3">
										{visualization.completed_at ? formatDate(visualization.completed_at) : '-'}
									</td>
									<td class="px-4 py-3">{formatDate(visualization.created_at)}</td>
								</tr>
								{#if visualization.error_message}
									<tr
										class="bg-red-50 dark:bg-red-900/10 border-b border-gray-200 dark:border-gray-700"
									>
										<td colspan="8" class="px-4 py-2 text-xs text-red-600 dark:text-red-400">
											Error: {visualization.error_message}
										</td>
									</tr>
								{/if}
							{/each}
						</tbody>
					</table>
				</div>

				<!-- Pagination -->
				<div class="mt-4 flex items-center justify-between">
					<div class="text-sm text-gray-600 dark:text-gray-400">
						Showing {(visualizationsCurrentPage - 1) * visualizationsPageSize + 1} to {Math.min(
							visualizationsCurrentPage * visualizationsPageSize,
							totalVisualizationsCount
						)} of {totalVisualizationsCount} visualizations
					</div>
					<div class="flex gap-2">
						<button
							onclick={() => handleVisualizationsPageChange(visualizationsCurrentPage - 1)}
							disabled={visualizationsCurrentPage === 1}
							class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
						>
							Previous
						</button>
						<div class="flex items-center gap-1">
							{#each Array.from({ length: getVisualizationsTotalPages() }, (_, i) => i + 1) as page (page)}
								{#if page === 1 || page === getVisualizationsTotalPages() || (page >= visualizationsCurrentPage - 1 && page <= visualizationsCurrentPage + 1)}
									<button
										onclick={() => handleVisualizationsPageChange(page)}
										class={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
											visualizationsCurrentPage === page
												? 'bg-blue-600 text-white'
												: 'border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
										}`}
									>
										{page}
									</button>
								{:else if page === visualizationsCurrentPage - 2 || page === visualizationsCurrentPage + 2}
									<span class="px-2 py-2 text-gray-500">...</span>
								{/if}
							{/each}
						</div>
						<button
							onclick={() => handleVisualizationsPageChange(visualizationsCurrentPage + 1)}
							disabled={visualizationsCurrentPage === getVisualizationsTotalPages()}
							class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
						>
							Next
						</button>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
