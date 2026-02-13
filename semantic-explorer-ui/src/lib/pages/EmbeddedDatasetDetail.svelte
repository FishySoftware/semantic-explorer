<script lang="ts">
	import { Button, Spinner } from 'flowbite-svelte';
	import { PlusOutline } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import ApiExamples from '../ApiExamples.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import TransformsList from '../components/TransformsList.svelte';
	import type {
		EmbeddedDataset,
		EmbeddedDatasetStats,
		Embedder,
		VisualizationTransform,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { createSSEConnection, type SSEConnection } from '../utils/sse';
	import { formatDate } from '../utils/ui-helpers';

	interface PaginatedEmbedderList {
		items: Embedder[];
		total_count: number;
		limit: number;
		offset: number;
	}

	interface PaginatedResponse<T> {
		items: T[];
		total_count: number;
		limit: number;
		offset: number;
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

	// SSE connection for real-time visualization transform status updates
	let sseConnection: SSEConnection | null = null;

	let points = $state<QdrantPoint[]>([]);
	let pointsLoading = $state(false);
	let pointsTotalCount = $state(0);
	let currentPointsPage = $state(0);
	let pointsPageSize = $state(10);

	let expandedPointId = $state<string | null>(null);
	let loadingVectorForPoint = $state<string | null>(null);
	let vectorCache = $state<Record<string, number[]>>({});

	let activeTab = $state('overview');

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete state
	let embeddedDatasetPendingDelete = $state(false);

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
					const data = (await embedderResponse.json()) as PaginatedEmbedderList;
					const embedders = data.items;
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

	function startEdit() {
		if (!embeddedDataset) return;
		editMode = true;
		editTitle = embeddedDataset.title;
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editTitle = '';
		editError = null;
	}

	async function saveEdit() {
		if (!embeddedDataset) return;

		if (!editTitle.trim()) {
			editError = 'Title is required';
			return;
		}

		try {
			saving = true;
			editError = null;
			const response = await fetch(`/api/embedded-datasets/${embeddedDatasetId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: editTitle.trim(),
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to update embedded dataset: ${response.statusText}`);
			}

			const updatedEmbeddedDataset = await response.json();
			embeddedDataset = updatedEmbeddedDataset;
			editMode = false;
			toastStore.success('Embedded dataset updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update embedded dataset');
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function confirmDeleteEmbeddedDataset() {
		if (!embeddedDataset) return;

		embeddedDatasetPendingDelete = false;

		try {
			const response = await fetch(
				`/api/embedded-datasets/${embeddedDataset.embedded_dataset_id}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to delete embedded dataset: ${errorText}`);
			}

			toastStore.success('Embedded dataset deleted successfully');
			onBack();
		} catch (e) {
			const message = formatError(e, 'Failed to delete embedded dataset');
			toastStore.error(message);
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
			const transformData = (await response.json()) as PaginatedResponse<VisualizationTransform>;
			const allTransforms = transformData.items;
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
		if (isStandalone()) {
			toastStore.error('Standalone datasets cannot be used for chat (no embedder configured)');
			return;
		}
		window.location.hash = `#/chat?embedded_dataset_id=${embeddedDatasetId}`;
	}

	function navigateToSearch() {
		if (isStandalone()) {
			toastStore.error('Standalone datasets cannot be used for search (no embedder configured)');
			return;
		}
		window.location.hash = `#/search?embedded_dataset_ids=${embeddedDatasetId}`;
	}

	function isStandalone(): boolean {
		return (
			embeddedDataset?.is_standalone === true ||
			(embeddedDataset?.dataset_transform_id === 0 &&
				embeddedDataset?.source_dataset_id === 0 &&
				embeddedDataset?.embedder_id === 0)
		);
	}

	function handleCreateVisualizationTransform() {
		// Redirect to visualization-transforms page with embedded dataset ID
		window.location.hash = `#/visualization-transforms?embedded_dataset_id=${embeddedDatasetId}&create=true`;
	}

	function handleViewVisualizationTransform(transform: any) {
		window.location.hash = `#/visualization-transforms/${transform.visualization_transform_id}`;
	}

	function handleEditVisualizationTransform(transform: any) {
		// Navigate to visualization-transforms page with edit parameter
		window.location.hash = `#/visualization-transforms?edit=${transform.visualization_transform_id}`;
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

	function getSuccessRate(): string {
		if (!stats) return 'N/A';
		const total = stats.successful_batches + stats.failed_batches;
		if (total === 0) return 'N/A';
		const rate = (stats.successful_batches / total) * 100;
		return `${rate.toFixed(1)}%`;
	}

	function connectSSE() {
		sseConnection = createSSEConnection({
			url: `/api/visualization-transforms/stream?embedded_dataset_id=${embeddedDatasetId}`,
			onStatus: () => {
				// Refresh visualization transforms and their stats
				fetchVisualizationTransforms();
			},
			onMaxRetriesReached: () => {
				console.warn('SSE connection lost for visualization transforms');
			},
		});
	}

	onMount(() => {
		fetchEmbeddedDataset();
		connectSSE();
	});

	onDestroy(() => {
		sseConnection?.disconnect();
	});
</script>

<div class=" mx-auto">
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

			{#if editMode}
				<!-- Edit Mode -->
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-center mb-4">
						<h2 class="text-xl font-semibold text-gray-900 dark:text-white">
							Edit Embedded Dataset
						</h2>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={cancelEdit}
								disabled={saving}
								class="px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors disabled:opacity-50"
							>
								Cancel
							</button>
							<button
								type="button"
								onclick={saveEdit}
								disabled={saving}
								class="px-3 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors disabled:opacity-50"
							>
								{saving ? 'Saving...' : 'Save Changes'}
							</button>
						</div>
					</div>

					{#if editError}
						<div
							class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3 mb-4"
						>
							<p class="text-sm text-red-700 dark:text-red-400">{editError}</p>
						</div>
					{/if}

					<div>
						<label
							for="edit-title"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Title <span class="text-red-500">*</span>
						</label>
						<input
							id="edit-title"
							type="text"
							bind:value={editTitle}
							disabled={saving}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
							placeholder="Enter embedded dataset title"
						/>
					</div>
				</div>
			{:else}
				<!-- View Mode -->
				<div class="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
					<div class="flex-1">
						<div class="flex items-center gap-2 mb-2">
							<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
								{embeddedDataset.title}
							</h1>
							{#if isStandalone()}
								<span
									class="px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-xs font-medium"
								>
									Standalone
								</span>
							{/if}
						</div>
						<p class="text-gray-600 dark:text-gray-400">
							Embedded Dataset #{embeddedDataset.embedded_dataset_id}
							{#if isStandalone()}
								<span class="text-sm"> · Push vectors directly via API</span>
							{/if}
						</p>
					</div>
					<div class="flex gap-2">
						<button
							type="button"
							onclick={startEdit}
							class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
							title="Edit embedded dataset"
						>
							<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
							type="button"
							onclick={() => (embeddedDatasetPendingDelete = true)}
							class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
							title="Delete embedded dataset"
						>
							<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
								/>
							</svg>
							Delete
						</button>
						{#if isStandalone()}
							<Button color="light" disabled title="Standalone datasets cannot be used for chat"
								>💬 Chat</Button
							>
							<Button color="light" disabled title="Standalone datasets cannot be used for search"
								>🔍 Search</Button
							>
						{:else}
							<Button color="blue" onclick={navigateToChat}>💬 Chat</Button>
							<Button color="purple" onclick={navigateToSearch}>🔍 Search</Button>
						{/if}
					</div>
				</div>
			{/if}
		</div>

		<!-- Metadata Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Metadata</h2>
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				{#if isStandalone()}
					<div>
						<p class="text-sm text-gray-600 dark:text-gray-400">Type</p>
						<p class="text-lg font-medium text-purple-600 dark:text-purple-400">
							Standalone Dataset
						</p>
					</div>
					<div>
						<p class="text-sm text-gray-600 dark:text-gray-400">Dimensions</p>
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							{embeddedDataset.dimensions || 'N/A'}
						</p>
					</div>
				{:else}
					<div>
						<p class="text-sm text-gray-600 dark:text-gray-400">Source Dataset</p>
						<button
							onclick={() =>
								embeddedDataset &&
								(window.location.hash = `#/datasets/${embeddedDataset.source_dataset_id}/details`)}
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
					{/if}
				{/if}
				<div>
					<p class="text-sm text-gray-600 dark:text-gray-400">Collection</p>
					{#if embeddedDataset.collection_id}
						<button
							onclick={() =>
								embeddedDataset &&
								(window.location.hash = `#/collections/${embeddedDataset.collection_id}/details`)}
							class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline text-left"
						>
							{embeddedDataset.collection_title || `Collection #${embeddedDataset.collection_id}`}
						</button>
					{:else}
						<p class="text-lg font-medium text-gray-500 dark:text-gray-400">None</p>
					{/if}
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
					{#if isStandalone()}
						<!-- Standalone Dataset: Show prominent API integration instructions -->
						<div
							class="bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-800 rounded-lg p-6 mb-6"
						>
							<div class="flex items-start gap-3">
								<div class="shrink-0 mt-0.5">
									<span class="text-2xl">🚀</span>
								</div>
								<div class="flex-1">
									<h3 class="text-lg font-semibold text-purple-900 dark:text-purple-100 mb-2">
										How to Populate This Dataset
									</h3>
									<p class="text-sm text-purple-800 dark:text-purple-200 mb-4">
										This is a <strong>standalone embedded dataset</strong>. Unlike transform-based
										datasets, you populate it by pushing vectors directly via the API. This is the
										only way to add data to this dataset.
									</p>
								</div>
							</div>
						</div>

						<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
							<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
								Push Vectors API
							</h3>

							<p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
								Send a POST request with an array of vector points. Each point must contain:
							</p>

							<ul
								class="list-disc list-inside text-sm text-gray-600 dark:text-gray-400 space-y-1 mb-4"
							>
								<li>
									<strong>id</strong>: String - Unique identifier for this point (UUID or any
									string)
								</li>
								<li>
									<strong>vector</strong>: Array of floats - The embedding vector (must be {embeddedDataset?.dimensions ||
										'N/A'} dimensions)
								</li>
								<li>
									<strong>payload</strong>: Object - Metadata for this point (any JSON object)
								</li>
							</ul>

							<ApiExamples
								endpoint={`/api/embedded-datasets/${embeddedDatasetId}/push-vectors`}
								method="POST"
								body={{
									points: [
										{
											id: 'unique-id-1',
											vector: Array(Math.min(embeddedDataset?.dimensions || 4, 4))
												.fill(0.1)
												.concat(
													embeddedDataset?.dimensions && embeddedDataset.dimensions > 4
														? ['... (' + (embeddedDataset.dimensions - 4) + ' more values)']
														: []
												),
											payload: {
												title: 'Example Document',
												text: 'The text content of this document',
												category: 'example',
											},
										},
									],
								}}
							/>
						</div>

						<div
							class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg p-4 mb-6"
						>
							<h4 class="text-sm font-semibold text-yellow-900 dark:text-yellow-300 mb-2">
								Important Notes
							</h4>
							<ul
								class="list-disc list-inside text-sm text-yellow-800 dark:text-yellow-400 space-y-1"
							>
								<li>
									Each vector must have exactly <strong
										>{embeddedDataset?.dimensions || 'N/A'}</strong
									> dimensions
								</li>
								<li>Maximum 1000 points per request</li>
								<li>Points with existing IDs will be updated (upsert behavior)</li>
								<li>
									Authentication is required via the <code>Authorization: Bearer</code> header
								</li>
								<li>
									Standalone datasets cannot be used for Search or Chat (no embedder for query
									embedding)
								</li>
								<li>Use Visualization Transforms to visualize the data in 2D/3D</li>
							</ul>
						</div>

						<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
							<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Dataset Info</h3>
							<div class="space-y-2">
								<p class="text-sm text-gray-700 dark:text-gray-300">
									<span class="font-medium">Created:</span>
									{embeddedDataset ? formatDate(embeddedDataset.created_at) : 'N/A'}
								</p>
								<p class="text-sm text-gray-700 dark:text-gray-300">
									<span class="font-medium">Last Updated:</span>
									{embeddedDataset ? formatDate(embeddedDataset.updated_at) : 'N/A'}
								</p>
								<p class="text-sm text-gray-700 dark:text-gray-300">
									<span class="font-medium">Dimensions:</span>
									{embeddedDataset?.dimensions || 'N/A'}
								</p>
								<p class="text-sm text-gray-700 dark:text-gray-300">
									<span class="font-medium">Collection:</span>
									<code class="bg-gray-100 dark:bg-gray-700 px-1 py-0.5 rounded text-xs"
										>{embeddedDataset?.collection_name}</code
									>
								</p>
							</div>
						</div>
					{:else}
						<!-- Transform-based Dataset: Show original overview -->
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
											Average Processing Time: {(stats.avg_processing_duration_ms / 1000).toFixed(
												2
											)}s
										</p>
									</div>
								{/if}
							</div>
						</div>
					{/if}
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
									<PlusOutline class="w-4 h-4" />
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

<ConfirmDialog
	open={embeddedDatasetPendingDelete}
	title="Delete Embedded Dataset?"
	message="Are you sure you want to delete this embedded dataset? This action cannot be undone."
	confirmLabel="Delete"
	cancelLabel="Cancel"
	onConfirm={confirmDeleteEmbeddedDataset}
	onCancel={() => (embeddedDatasetPendingDelete = false)}
	variant="danger"
/>
