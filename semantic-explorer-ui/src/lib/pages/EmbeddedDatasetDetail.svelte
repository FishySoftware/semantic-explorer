<script lang="ts">
	import { Button, Spinner } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import TransformsList from '../components/TransformsList.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import type { VisualizationTransform } from '../types/visualizations';

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		dataset_transform_id: number;
		source_dataset_id: number;
		embedder_id: number;
		owner: string;
		collection_name: string;
		created_at: string;
		updated_at: string;
		source_dataset_title?: string;
		embedder_name?: string;
	}

	interface EmbeddedDatasetStats {
		embedded_dataset_id: number;
		total_batches_processed: number;
		successful_batches: number;
		failed_batches: number;
		processing_batches: number;
		total_chunks_embedded: number;
		total_chunks_failed: number;
		total_chunks_processing: number;
		last_run_at?: string;
		first_processing_at?: string;
		avg_processing_duration_ms?: number;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
		dimensions: number;
		config: {
			model?: string;
			[key: string]: any;
		};
	}

	interface QdrantPoint {
		id: string;
		payload: Record<string, any>;
		vector?: number[];
	}

	interface PointsResponse {
		points: QdrantPoint[];
		total_count: number;
		next_offset?: string;
	}

	interface Props {
		embeddedDatasetId: number;
		onBack: () => void;
	}

	let { embeddedDatasetId, onBack }: Props = $props();

	let embeddedDataset = $state<EmbeddedDataset | null>(null);
	let stats = $state<EmbeddedDatasetStats | null>(null);
	let embedder = $state<Embedder | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let visualizationTransforms = $state<VisualizationTransform[]>([]);
	let transformsLoading = $state(false);
	let visualizationTransformStatsMap = $state<Map<number, any>>(new Map());

	let points = $state<QdrantPoint[]>([]);
	let pointsLoading = $state(false);
	let pointsTotalCount = $state(0);
	let currentPointsPage = $state(0);
	let pointsPageSize = $state(10);

	let expandedPointId = $state<string | null>(null);
	let loadingVectorForPoint = $state<string | null>(null);
	let vectorCache = $state<Record<string, number[]>>({});

	let activeTab = $state('overview');

	const tabs = [
		{ id: 'overview', label: 'Overview', icon: '📊' },
		{ id: 'points', label: 'Points', icon: '📍' },
		{ id: 'transforms', label: 'Visualizations', icon: '🎨' },
	];

	async function fetchEmbeddedDataset() {
		try {
			loading = true;
			error = null;

			const response = await fetch(`/api/embedded-datasets/${embeddedDatasetId}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded dataset: ${response.statusText}`);
			}
			embeddedDataset = await response.json();

			// Fetch stats
			const statsResponse = await fetch(`/api/embedded-datasets/${embeddedDatasetId}/stats`);
			if (statsResponse.ok) {
				stats = await statsResponse.json();
			}

			// Fetch embedder info if we have the embedder_id
			if (embeddedDataset && embeddedDataset.embedder_id) {
				const embedderResponse = await fetch('/api/embedders');
				if (embedderResponse.ok) {
					const embedders: Embedder[] = await embedderResponse.json();
					embedder = embedders.find((e) => e.embedder_id === embeddedDataset!.embedder_id) || null;
				}
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to fetch embedded dataset';
			toastStore.error(formatError(e, 'Failed to fetch embedded dataset'));
		} finally {
			loading = false;
		}
	}

	async function fetchVisualizationTransforms() {
		if (!embeddedDataset) return;

		try {
			transformsLoading = true;
			const response = await fetch('/api/visualization-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			const allTransforms: VisualizationTransform[] = await response.json();
			visualizationTransforms = allTransforms.filter(
				(t) => t.embedded_dataset_id === embeddedDataset!.embedded_dataset_id
			);

			// Fetch stats for each transform
			for (const transform of visualizationTransforms) {
				try {
					const statsResponse = await fetch(
						`/api/visualization-transforms/${transform.visualization_transform_id}/stats`
					);
					if (statsResponse.ok) {
						const stats = await statsResponse.json();
						visualizationTransformStatsMap.set(transform.visualization_transform_id, stats);
					}
				} catch (e) {
					console.error(
						`Failed to fetch stats for transform ${transform.visualization_transform_id}:`,
						e
					);
				}
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to fetch visualization transforms'));
		} finally {
			transformsLoading = false;
		}
	}

	async function fetchPoints(page: number = 0) {
		if (!embeddedDataset) return;

		try {
			pointsLoading = true;
			const offset = page * pointsPageSize;
			const response = await fetch(
				`/api/embedded-datasets/${embeddedDatasetId}/points?limit=${pointsPageSize}&offset=${offset}`
			);
			if (!response.ok) {
				throw new Error(`Failed to fetch points: ${response.statusText}`);
			}
			const data: PointsResponse = await response.json();
			points = data.points;
			pointsTotalCount = data.total_count;

			currentPointsPage = page;
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to fetch points'));
		} finally {
			pointsLoading = false;
		}
	}

	async function loadVectorForPoint(pointId: string) {
		if (vectorCache[pointId]) {
			// Already loaded
			return;
		}

		try {
			loadingVectorForPoint = pointId;
			const response = await fetch(
				`/api/embedded-datasets/${embeddedDatasetId}/points/${encodeURIComponent(pointId)}/vector`
			);
			if (!response.ok) {
				throw new Error(`Failed to fetch vector: ${response.statusText}`);
			}
			const data: QdrantPoint = await response.json();
			if (data.vector) {
				vectorCache[pointId] = data.vector;
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to fetch vector'));
		} finally {
			loadingVectorForPoint = null;
		}
	}

	function togglePointExpansion(pointId: string) {
		if (expandedPointId === pointId) {
			expandedPointId = null;
		} else {
			expandedPointId = pointId;
		}
	}

	function toggleVectorView(pointId: string) {
		if (!vectorCache[pointId]) {
			loadVectorForPoint(pointId);
		} else {
			// Remove from cache to hide it
			vectorCache = Object.fromEntries(
				Object.entries(vectorCache).filter(([key]) => key !== pointId)
			);
		}
	}

	function handleTabChange(tabId: string) {
		activeTab = tabId;
		if (tabId === 'transforms' && visualizationTransforms.length === 0) {
			fetchVisualizationTransforms();
		} else if (tabId === 'points' && points.length === 0) {
			fetchPoints(0);
		}
	}

	function navigateToChat() {
		window.location.hash = `#/chat?embedded_dataset_id=${embeddedDatasetId}`;
	}

	function navigateToSearch() {
		window.location.hash = `#/search?embedded_dataset_ids=${embeddedDatasetId}`;
	}

	function handleCreateVisualizationTransform() {
		// Redirect to visualization-transforms page with embedded dataset ID
		window.location.hash = `#/visualization-transforms?embedded_dataset_id=${embeddedDatasetId}&create=true`;
	}

	function handleViewVisualizationTransform(transform: any) {
		window.location.hash = `#/visualization-transforms/${transform.visualization_transform_id}`;
	}

	function handleEditVisualizationTransform(_transform: any) {
		// TODO: Implement edit modal
		toastStore.info('Edit functionality coming soon');
	}

	function handleTriggerVisualizationTransform(transform: any) {
		// Trigger the transform
		fetch(`/api/visualization-transforms/${transform.visualization_transform_id}/trigger`, {
			method: 'POST',
		})
			.then((response) => {
				if (!response.ok) {
					throw new Error(`Failed to trigger visualization transform: ${response.statusText}`);
				}
				toastStore.success('Visualization transform triggered');
				fetchVisualizationTransforms();
			})
			.catch((e) => {
				toastStore.error(formatError(e, 'Failed to trigger visualization transform'));
			});
	}

	function handleDeleteVisualizationTransform(transform: any) {
		if (!confirm(`Are you sure you want to delete "${transform.title}"?`)) {
			return;
		}

		fetch(`/api/visualization-transforms/${transform.visualization_transform_id}`, {
			method: 'DELETE',
		})
			.then((response) => {
				if (!response.ok) {
					throw new Error(`Failed to delete visualization transform: ${response.statusText}`);
				}
				toastStore.success('Visualization transform deleted');
				fetchVisualizationTransforms();
			})
			.catch((e) => {
				toastStore.error(formatError(e, 'Failed to delete visualization transform'));
			});
	}

	function nextPointsPage() {
		if ((currentPointsPage + 1) * pointsPageSize < pointsTotalCount) {
			fetchPoints(currentPointsPage + 1);
		}
	}

	function prevPointsPage() {
		if (currentPointsPage > 0) {
			fetchPoints(currentPointsPage - 1);
		}
	}

	function formatDate(dateString: string): string {
		const date = new Date(dateString);
		return date.toLocaleDateString('en-US', {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit',
		});
	}

	function getSuccessRate(): string {
		if (!stats) return 'N/A';
		const total = stats.successful_batches + stats.failed_batches;
		if (total === 0) return 'N/A';
		const rate = (stats.successful_batches / total) * 100;
		return `${rate.toFixed(1)}%`;
	}

	onMount(() => {
		fetchEmbeddedDataset();
	});
</script>

<div class="max-w-7xl mx-auto">
	{#if loading}
		<div class="flex items-center justify-center py-12">
			<Spinner size="12" />
		</div>
	{:else if error}
		<div class="text-red-600 dark:text-red-400 py-8 text-center">
			<p class="text-xl mb-4">⚠️ {error}</p>
			<Button color="light" onclick={onBack}>Go Back</Button>
		</div>
	{:else if embeddedDataset}
		<!-- Header -->
		<div class="mb-6">
			<button
				onclick={onBack}
				class="text-blue-600 dark:text-blue-400 hover:underline mb-4 inline-flex items-center gap-2"
			>
				← Back to Embedded Datasets
			</button>
			<div class="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
				<div class="flex-1">
					<h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
						{embeddedDataset.title}
					</h1>
					<p class="text-gray-600 dark:text-gray-400">
						Embedded Dataset #{embeddedDataset.embedded_dataset_id}
					</p>
				</div>
				<div class="flex gap-2">
					<Button color="blue" onclick={navigateToChat}>💬 Chat</Button>
					<Button color="purple" onclick={navigateToSearch}>🔍 Search</Button>
				</div>
			</div>
		</div>

		<!-- Metadata Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Metadata</h2>
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				<div>
					<p class="text-sm text-gray-600 dark:text-gray-400">Source Dataset</p>
					<button
						onclick={() =>
							embeddedDataset &&
							(window.location.hash = `#/datasets/${embeddedDataset.source_dataset_id}`)}
						class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline text-left"
					>
						{embeddedDataset.source_dataset_title ||
							`Dataset #${embeddedDataset.source_dataset_id}`}
					</button>
				</div>
				<div>
					<p class="text-sm text-gray-600 dark:text-gray-400">Embedder</p>
					<button
						onclick={() =>
							embeddedDataset &&
							(window.location.hash = `#/embedders/${embeddedDataset.embedder_id}/details`)}
						class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline text-left"
					>
						{embeddedDataset.embedder_name || `Embedder #${embeddedDataset.embedder_id}`}
					</button>
				</div>
				{#if embedder}
					<div>
						<p class="text-sm text-gray-600 dark:text-gray-400">Dimension</p>
						<p class="text-lg font-medium text-gray-900 dark:text-white">{embedder.dimensions}</p>
					</div>
					<div>
						<p class="text-sm text-gray-600 dark:text-gray-400">Embedding Model</p>
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							{embedder.config?.model || embedder.name}
						</p>
					</div>
				{/if}
				<div>
					<p class="text-sm text-gray-600 dark:text-gray-400">Collection Name</p>
					<button
						onclick={() =>
							embeddedDataset &&
							(window.location.hash = `#/collections?search=${encodeURIComponent(embeddedDataset.collection_name)}`)}
						class="font-mono text-blue-600 dark:text-blue-400 hover:underline text-sm break-all text-left"
					>
						{embeddedDataset.collection_name}
					</button>
				</div>
				<div>
					<p class="text-sm text-gray-600 dark:text-gray-400">Owner</p>
					<p class="text-lg font-medium text-gray-900 dark:text-white">{embeddedDataset.owner}</p>
				</div>
			</div>

			{#if stats}
				<div class="mt-6 pt-6 border-t border-gray-200 dark:border-gray-700">
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Statistics</h3>
					<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Total Chunks</p>
							<p class="text-2xl font-bold text-gray-900 dark:text-white">
								{stats.total_chunks_embedded.toLocaleString()}
							</p>
						</div>
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Success Rate</p>
							<p class="text-2xl font-bold text-green-600 dark:text-green-400">
								{getSuccessRate()}
							</p>
						</div>
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Batches Processed</p>
							<p class="text-2xl font-bold text-gray-900 dark:text-white">
								{stats.total_batches_processed}
							</p>
						</div>
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Failed Chunks</p>
							<p
								class="text-2xl font-bold"
								class:text-red-600={stats.total_chunks_failed > 0}
								class:dark:text-red-400={stats.total_chunks_failed > 0}
								class:text-gray-400={stats.total_chunks_failed === 0}
							>
								{stats.total_chunks_failed}
							</p>
						</div>
					</div>
				</div>
			{/if}
		</div>

		<!-- Tabs -->
		<TabPanel {tabs} activeTabId={activeTab} onChange={handleTabChange}>
			{#snippet children(tabId)}
				{#if tabId === 'overview'}
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
						<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Overview</h3>
						<p class="text-gray-600 dark:text-gray-400 mb-4">
							Embedded datasets contain vector embeddings generated from a source dataset using a
							specified embedder.
						</p>

						<div class="space-y-4">
							<div>
								<p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
									Created: {embeddedDataset ? formatDate(embeddedDataset.created_at) : 'N/A'}
								</p>
								<p class="text-sm font-medium text-gray-700 dark:text-gray-300">
									Last Updated: {embeddedDataset ? formatDate(embeddedDataset.updated_at) : 'N/A'}
								</p>
							</div>

							{#if stats?.last_run_at}
								<div>
									<p class="text-sm font-medium text-gray-700 dark:text-gray-300">
										Last Processed: {formatDate(stats.last_run_at)}
									</p>
								</div>
							{/if}

							{#if stats && stats.avg_processing_duration_ms}
								<div>
									<p class="text-sm font-medium text-gray-700 dark:text-gray-300">
										Average Processing Time: {(stats.avg_processing_duration_ms / 1000).toFixed(2)}s
									</p>
								</div>
							{/if}
						</div>
					</div>
				{:else if tabId === 'transforms'}
					<div>
						{#if transformsLoading}
							<div class="flex items-center justify-center py-8">
								<Spinner size="8" />
							</div>
						{:else if visualizationTransforms.length === 0}
							<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-8 text-center">
								<p class="text-gray-600 dark:text-gray-400 mb-4">
									No visualization transforms found.
								</p>
								<button
									onclick={handleCreateVisualizationTransform}
									class="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium"
								>
									Create Visualization Transform
								</button>
							</div>
						{:else}
							<div class="flex justify-between items-center mb-4">
								<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
									Visualization Transforms
								</h3>
								<button
									onclick={handleCreateVisualizationTransform}
									class="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium"
								>
									<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M12 4v16m8-8H4"
										></path>
									</svg>
									Create Visualization Transform
								</button>
							</div>
							<TransformsList
								transforms={visualizationTransforms.map((t) => ({
									...t,
									last_run_stats: visualizationTransformStatsMap.get(t.visualization_transform_id),
								}))}
								type="visualization"
								loading={transformsLoading}
								onView={handleViewVisualizationTransform}
								onEdit={handleEditVisualizationTransform}
								onTrigger={handleTriggerVisualizationTransform}
								onDelete={handleDeleteVisualizationTransform}
							/>
						{/if}
					</div>
				{:else if tabId === 'points'}
					<div>
						<div class="flex justify-between items-center mb-4">
							<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
								Qdrant Points ({pointsTotalCount.toLocaleString()} total)
							</h3>
						</div>

						{#if pointsLoading}
							<div class="flex items-center justify-center py-8">
								<Spinner size="8" />
							</div>
						{:else if points.length === 0}
							<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-8 text-center">
								<p class="text-gray-600 dark:text-gray-400">
									No points found in this embedded dataset.
								</p>
							</div>
						{:else}
							<div class="space-y-4">
								{#each points as point (point.id)}
									<div
										class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 border border-gray-200 dark:border-gray-700"
									>
										<div class="flex justify-between items-start mb-2">
											<div class="flex-1">
												<p class="text-sm font-mono text-gray-600 dark:text-gray-400 mb-2">
													Point ID: {point.id}
												</p>
												<button
													onclick={() => togglePointExpansion(point.id)}
													class="text-blue-600 dark:text-blue-400 hover:underline text-sm font-medium"
												>
													{expandedPointId === point.id ? '▼ Hide Details' : '▶ Show Details'}
												</button>
											</div>
											<button
												onclick={() => toggleVectorView(point.id)}
												class="px-3 py-1 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 text-sm"
												disabled={loadingVectorForPoint === point.id}
											>
												{#if loadingVectorForPoint === point.id}
													<Spinner size="4" />
												{:else if vectorCache[point.id]}
													Hide Vector
												{:else}
													View Vector
												{/if}
											</button>
										</div>

										{#if expandedPointId === point.id}
											<div class="mt-4 p-4 bg-gray-50 dark:bg-gray-900 rounded">
												<p class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
													Payload:
												</p>
												<pre
													class="text-xs bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 p-3 rounded overflow-x-auto border border-gray-200 dark:border-gray-700">{JSON.stringify(
														point.payload,
														null,
														2
													)}</pre>
											</div>
										{/if}

										{#if vectorCache[point.id]}
											<div class="mt-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded">
												<p class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
													Vector ({vectorCache[point.id].length} dimensions):
												</p>
												<pre
													class="text-xs bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100 p-3 rounded overflow-x-auto max-h-64">{JSON.stringify(
														vectorCache[point.id],
														null,
														2
													)}</pre>
											</div>
										{/if}
									</div>
								{/each}

								<!-- Pagination -->
								<div class="flex justify-between items-center pt-4">
									<Button color="light" onclick={prevPointsPage} disabled={currentPointsPage === 0}>
										← Previous
									</Button>
									<span class="text-gray-600 dark:text-gray-400">
										Page {currentPointsPage + 1} of {Math.ceil(pointsTotalCount / pointsPageSize)}
									</span>
									<Button
										color="light"
										onclick={nextPointsPage}
										disabled={(currentPointsPage + 1) * pointsPageSize >= pointsTotalCount}
									>
										Next →
									</Button>
								</div>
							</div>
						{/if}
					</div>
				{/if}
			{/snippet}
		</TabPanel>
	{/if}
</div>
