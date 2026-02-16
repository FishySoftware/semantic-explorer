<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import VisualizationProgressBanner from '../components/VisualizationProgressBanner.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate, formatDuration } from '../utils/ui-helpers';

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
	let progressDismissed = $state(false);

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete state
	let transformPendingDelete = $state<VisualizationTransform | null>(null);
	let deleting = $state(false);

	// Pagination for visualizations
	let visualizationsCurrentPage = $state(1);
	let visualizationsPageSize = $state(20);

	// Derived: in-progress visualization runs
	let processingRuns = $derived(
		visualizations.filter((v) => v.status === 'pending' || v.status === 'processing')
	);

	let isTransformProcessing = $derived(
		transform?.last_run_status === 'pending' || transform?.last_run_status === 'processing'
	);

	let showProgressBanner = $derived(
		!progressDismissed && (isTransformProcessing || processingRuns.length > 0)
	);

	/** Convert a snake_case key to a human-readable Title Case label. */
	function formatStatLabel(key: string): string {
		return key
			.split('_')
			.map((w) => w.charAt(0).toUpperCase() + w.slice(1))
			.join(' ');
	}

	// Polling interval for auto-refresh
	let pollTimer: ReturnType<typeof setInterval> | null = null;
	let isPolling = false;
	const POLL_INTERVAL_MS = 5000;

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
				title: `visualization-${v.visualization_transform_id}-${new Date(v.created_at).toISOString().split('T')[0]}`,
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

	function startEdit() {
		if (!transform) return;
		editMode = true;
		editTitle = transform.title;
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editTitle = '';
		editError = null;
	}

	async function saveEdit() {
		if (!transform) return;
		if (!editTitle.trim()) {
			editError = 'Title is required';
			return;
		}

		try {
			saving = true;
			editError = null;
			const response = await fetch(`/api/visualization-transforms/${visualizationTransformId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ title: editTitle.trim() }),
			});

			if (!response.ok) {
				throw new Error(`Failed to update transform: ${response.statusText}`);
			}

			const updated = await response.json();
			transform = updated;
			editMode = false;
			toastStore.success('Visualization transform updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update visualization transform');
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function toggleEnabled() {
		if (!transform) return;

		try {
			const response = await fetch(`/api/visualization-transforms/${visualizationTransformId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ is_enabled: !transform.is_enabled }),
			});

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const updated = await response.json();
			transform = updated;
			toastStore.success(
				`Visualization transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle visualization transform'));
		}
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) return;

		transformPendingDelete = null;

		try {
			deleting = true;
			const response = await fetch(`/api/visualization-transforms/${visualizationTransformId}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete transform: ${response.statusText}`);
			}

			toastStore.success('Visualization transform deleted');
			onBack();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete visualization transform'));
		} finally {
			deleting = false;
		}
	}

	onMount(async () => {
		loading = true;
		await Promise.all([fetchTransform(), fetchStats(), fetchVisualizations()]);
		loading = false;

		// Auto-refresh stats and visualizations every 5 seconds, skipping if already in-flight
		pollTimer = setInterval(async () => {
			if (isPolling) return;
			isPolling = true;
			try {
				await Promise.all([fetchTransform(), fetchStats(), fetchVisualizations()]);
			} finally {
				isPolling = false;
			}
		}, POLL_INTERVAL_MS);
	});

	onDestroy(() => {
		if (pollTimer) {
			clearInterval(pollTimer);
			pollTimer = null;
		}
	});
</script>

<div class="mx-auto">
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
		<!-- Processing Progress Banner -->
		{#if showProgressBanner}
			<VisualizationProgressBanner
				lastRunStatus={transform.last_run_status}
				lastRunAt={transform.last_run_at}
				lastError={transform.last_error}
				{processingRuns}
				onDismiss={() => (progressDismissed = true)}
			/>
		{/if}

		<!-- Transform Info Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<div class="flex justify-between items-start mb-4">
				<div class="flex-1">
					{#if editMode}
						<form
							onsubmit={(e) => {
								e.preventDefault();
								saveEdit();
							}}
							class="flex items-center gap-2 mb-2"
						>
							<input
								type="text"
								bind:value={editTitle}
								placeholder="Enter transform title"
								class="text-2xl font-bold px-3 py-1 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white flex-1"
								required
							/>
							<button
								type="submit"
								disabled={saving}
								class="px-3 py-1.5 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{saving ? 'Saving...' : 'Save'}
							</button>
							<button
								type="button"
								onclick={cancelEdit}
								class="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
							>
								Cancel
							</button>
						</form>
						{#if editError}
							<p class="text-sm text-red-600 dark:text-red-400 mt-1">{editError}</p>
						{/if}
					{:else}
						<div class="flex items-baseline gap-3 mb-2">
							<Heading tag="h2" class="text-2xl font-bold">{transform.title}</Heading>
							<span class="text-sm text-gray-500 dark:text-gray-400"
								>#{transform.visualization_transform_id}</span
							>
						</div>
					{/if}
					<p class="text-sm text-gray-500 dark:text-gray-400">
						Created {formatDate(transform.created_at)}
						{#if transform.updated_at && transform.updated_at !== transform.created_at}
							&middot; Updated {formatDate(transform.updated_at)}
						{/if}
					</p>
				</div>
				<div class="flex items-center gap-2 ml-4">
					{#if !editMode}
						<button
							onclick={startEdit}
							title="Edit title"
							class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300 transition-colors"
						>
							Edit
						</button>
					{/if}
					<button
						onclick={toggleEnabled}
						title={transform.is_enabled ? 'Disable transform' : 'Enable transform'}
						class={transform.is_enabled
							? 'px-3 py-1 text-sm rounded-lg bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400 transition-colors'
							: 'px-3 py-1 text-sm rounded-lg bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400 transition-colors'}
					>
						{transform.is_enabled ? 'Disable' : 'Enable'}
					</button>
					<button
						onclick={() => (transformPendingDelete = transform)}
						disabled={deleting}
						title="Delete transform"
						class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
					>
						{deleting ? 'Deleting...' : 'Delete'}
					</button>
				</div>
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
			</div>
		</div>

		<!-- Configuration Card -->
		{#if transform.visualization_config && Object.keys(transform.visualization_config).length > 0}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<Heading tag="h3" class="text-lg font-bold mb-4">Visualization Configuration</Heading>
				<pre
					class="text-sm font-mono bg-gray-50 dark:bg-gray-900 rounded-lg p-4 overflow-auto max-h-[60vh] whitespace-pre-wrap text-gray-900 dark:text-gray-100">{JSON.stringify(
						transform.visualization_config,
						null,
						2
					)}</pre>
			</div>
		{/if}

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
			{#if transform.last_run_stats && Object.keys(transform.last_run_stats).length > 0}
				<div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
					<p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Last Run Stats</p>
					<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
						{#each Object.entries(transform.last_run_stats) as [key, value] (key)}
							<div>
								<p class="text-xs text-gray-500 dark:text-gray-400">{formatStatLabel(key)}</p>
								<p class="text-sm font-semibold text-gray-900 dark:text-white">
									{typeof value === 'object' ? JSON.stringify(value) : value}
								</p>
							</div>
						{/each}
					</div>
				</div>
			{/if}
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
							{`visualization-${transform?.visualization_transform_id || 'unknown'}-${new Date(stats.latest_visualization.created_at).toISOString().split('T')[0]}`}
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
										{#if visualization.stats_json?.processing_duration_ms != null}
											{formatDuration(visualization.stats_json.processing_duration_ms as number)}
										{:else if visualization.started_at && visualization.completed_at}
											{formatDuration(
												new Date(visualization.completed_at).getTime() -
													new Date(visualization.started_at).getTime()
											)}
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

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete Visualization Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteTransform}
	onCancel={() => (transformPendingDelete = null)}
/>
