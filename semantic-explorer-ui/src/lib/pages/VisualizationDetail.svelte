<script lang="ts">
	import {
		Badge,
		Button,
		Spinner,
		Table,
		TableBody,
		TableBodyCell,
		TableHead,
		TableHeadCell,
	} from 'flowbite-svelte';
	import { ArrowLeftOutline, DownloadOutline, EyeOutline } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import type {
		Visualization,
		VisualizationTransform,
		EmbeddedDataset,
		Dataset,
		Embedder,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { createSSEConnection, type SSEConnection } from '../utils/sse';
	import { formatDate, formatNumber } from '../utils/ui-helpers';

	interface Props {
		visualizationTransformId: number;
		onBack: () => void;
	}

	let { visualizationTransformId, onBack }: Props = $props();

	let transform = $state<VisualizationTransform | null>(null);
	let embeddedDataset = $state<EmbeddedDataset | null>(null);
	let sourceDataset = $state<Dataset | null>(null);
	let embedder = $state<Embedder | null>(null);
	let visualizations = $state<Visualization[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let selectedVisualization = $state<Visualization | null>(null);
	let htmlContent = $state<string | null>(null);
	let activeTab = $state('overview');

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete state
	let transformPendingDelete = $state(false);

	// Tabs
	const tabs = $derived([
		{ id: 'overview', label: 'Overview', icon: '📊' },
		{ id: 'visualizations', label: 'Visualizations', icon: '🎨' },
	]);

	// SSE connection for real-time status updates
	let sseConnection: SSEConnection | null = null;

	onMount(async () => {
		await loadTransform();
		await loadRelatedData();
		await loadVisualizations();
		connectSSE();
	});

	onDestroy(() => {
		sseConnection?.disconnect();
	});

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
				body: JSON.stringify({
					title: editTitle.trim(),
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to update visualization transform: ${response.statusText}`);
			}

			const updatedTransform = await response.json();
			transform = updatedTransform;
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

	async function confirmDeleteTransform() {
		if (!transform) return;

		transformPendingDelete = false;

		try {
			const response = await fetch(
				`/api/visualization-transforms/${transform.visualization_transform_id}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to delete visualization transform: ${errorText}`);
			}

			toastStore.success('Visualization transform deleted successfully');
			onBack();
		} catch (e) {
			const message = formatError(e, 'Failed to delete visualization transform');
			toastStore.error(message);
		}
	}

	async function loadRelatedData() {
		if (!transform) return;

		try {
			// Load embedded dataset
			const embeddedResponse = await fetch(
				`/api/embedded-datasets/${transform.embedded_dataset_id}`
			);
			if (embeddedResponse.ok) {
				embeddedDataset = await embeddedResponse.json();

				// Load source dataset if not standalone
				if (
					embeddedDataset &&
					embeddedDataset.source_dataset_id &&
					embeddedDataset.source_dataset_id !== 0
				) {
					const datasetResponse = await fetch(`/api/datasets/${embeddedDataset.source_dataset_id}`);
					if (datasetResponse.ok) {
						sourceDataset = await datasetResponse.json();
					}
				}

				// Load embedder info
				if (embeddedDataset && embeddedDataset.embedder_id && embeddedDataset.embedder_id !== 0) {
					const embedderResponse = await fetch('/api/embedders');
					if (embedderResponse.ok) {
						const data = await embedderResponse.json();
						const embedders = data.items || [];
						embedder =
							embedders.find((e: Embedder) => e.embedder_id === embeddedDataset!.embedder_id) ||
							null;
					}
				}
			}
		} catch (e) {
			console.error('Failed to load related data:', e);
		}
	}

	function navigateToEmbeddedDataset() {
		if (embeddedDataset) {
			window.location.hash = `#/embedded-datasets/${embeddedDataset.embedded_dataset_id}/details`;
		}
	}

	function navigateToDataset() {
		if (sourceDataset) {
			window.location.hash = `#/datasets/${sourceDataset.dataset_id}/details`;
		}
	}

	function navigateToEmbedder() {
		if (embedder) {
			window.location.hash = `#/embedders/${embedder.embedder_id}/details`;
		}
	}

	function connectSSE() {
		// Connect to visualization transforms SSE for real-time updates
		sseConnection = createSSEConnection({
			url: `/api/visualization-transforms/stream?visualization_transform_id=${visualizationTransformId}`,
			onStatus: (data: unknown) => {
				const status = data as {
					visualization_transform_id?: number;
					visualization_id?: number;
				};
				// Reload visualizations when we get a status update for this transform
				if (status.visualization_transform_id === visualizationTransformId) {
					loadVisualizations();
				}
			},
			onMaxRetriesReached: () => {
				console.warn('SSE connection lost for visualization transform');
			},
		});
	}

	async function loadTransform() {
		try {
			loading = true;
			error = null;

			const response = await fetch(`/api/visualization-transforms/${visualizationTransformId}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transform: ${response.statusText}`);
			}
			transform = await response.json();
		} catch (err) {
			error = formatError(err);
			toastStore.error(error);
		} finally {
			loading = false;
		}
	}

	async function loadVisualizations() {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${visualizationTransformId}/visualizations`
			);
			if (!response.ok) {
				throw new Error(`Failed to fetch visualizations: ${response.statusText}`);
			}
			const newVisualizations: Visualization[] = await response.json();
			visualizations = newVisualizations;

			// If we have a selected visualization, update it with the latest data
			if (selectedVisualization) {
				const updatedViz = newVisualizations.find(
					(v) => v.visualization_id === selectedVisualization!.visualization_id
				);
				if (updatedViz) {
					selectedVisualization = updatedViz;
					// Reload HTML if status changed to completed
					if (updatedViz.status === 'completed' && updatedViz.html_s3_key && !htmlContent) {
						await loadHtml(updatedViz);
					}
				}
			}
		} catch (err) {
			console.error('Failed to load visualizations:', err);
		}
	}

	async function viewVisualization(visualization: Visualization) {
		selectedVisualization = visualization;
		htmlContent = null;
		activeTab = 'overview';
		if (visualization.status === 'completed' && visualization.html_s3_key) {
			await loadHtml(visualization);
		}
	}

	async function loadHtml(visualization: Visualization) {
		if (!visualization.html_s3_key) {
			toastStore.error('No HTML file available for this visualization');
			return;
		}

		try {
			const response = await fetch(
				`/api/visualization-transforms/${visualizationTransformId}/visualizations/${visualization.visualization_id}/download`
			);

			if (!response.ok) {
				throw new Error(`Failed to download HTML: ${response.statusText}`);
			}

			htmlContent = await response.text();
		} catch (err) {
			toastStore.error(formatError(err, 'Failed to load visualization HTML'));
		}
	}

	async function downloadHtml(visualization: Visualization) {
		if (!visualization.html_s3_key) {
			toastStore.error('No HTML file available to download');
			return;
		}

		try {
			const response = await fetch(
				`/api/visualization-transforms/${visualizationTransformId}/visualizations/${visualization.visualization_id}/download`
			);

			if (!response.ok) {
				throw new Error(`Failed to download HTML: ${response.statusText}`);
			}

			// Read as text since the response is HTML, then create a blob for download
			const text = await response.text();
			const blob = new Blob([text], { type: 'text/html' });
			const url = window.URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `visualization-${visualizationTransformId}-${visualization.visualization_id}.html`;
			document.body.appendChild(a);
			a.click();
			window.URL.revokeObjectURL(url);
			document.body.removeChild(a);

			toastStore.success('HTML file downloaded');
		} catch (err) {
			toastStore.error(formatError(err));
		}
	}

	function getStatusBadge(status: string): {
		label: string;
		color: 'green' | 'blue' | 'red' | 'yellow' | 'gray';
	} {
		switch (status.toLowerCase()) {
			case 'completed':
				return { label: 'Completed', color: 'green' };
			case 'processing':
				return { label: 'Processing', color: 'blue' };
			case 'failed':
				return { label: 'Failed', color: 'red' };
			case 'pending':
				return { label: 'Pending', color: 'yellow' };
			default:
				return { label: status, color: 'gray' };
		}
	}

	function getProgressMessage(visualization: Visualization): string {
		if (visualization.status === 'processing') {
			const progressData = visualization.stats_json as {
				stage?: string;
				progress_percent?: number;
			};
			if (progressData?.stage) {
				const stage = progressData.stage.replace(/_/g, ' ');
				const percent = progressData.progress_percent || 0;
				return `${stage}: ${percent}%`;
			}
			return 'Processing...';
		}
		return '';
	}

	function getProgressPercent(visualization: Visualization): number {
		if (visualization.status !== 'processing') return 0;
		const progressData = visualization.stats_json as { progress_percent?: number };
		return progressData?.progress_percent || 0;
	}

	function closeVisualization() {
		selectedVisualization = null;
		htmlContent = null;
	}

	const statsComputed = $derived(() => {
		if (!visualizations.length) return null;
		return {
			total: visualizations.length,
			completed: visualizations.filter((v) => v.status === 'completed').length,
			processing: visualizations.filter((v) => v.status === 'processing' || v.status === 'pending')
				.length,
			failed: visualizations.filter((v) => v.status === 'failed').length,
		};
	});
</script>

<div class="mx-auto">
	<div class="mb-4">
		<button onclick={onBack} class="mb-4 btn-secondary inline-flex items-center gap-2">
			<ArrowLeftOutline class="w-5 h-5" />
			Back to Visualizations
		</button>

		{#if loading && !transform}
			<LoadingState message="Loading visualization transform..." />
		{:else if error}
			<div
				class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
			>
				<p class="text-red-700 dark:text-red-400">{error}</p>
				<button
					onclick={loadTransform}
					class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
				>
					Try again
				</button>
			</div>
		{:else if transform}
			<!-- Header -->
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-4">
				{#if editMode}
					<!-- Edit Mode -->
					<div class="flex justify-between items-center mb-4">
						<h2 class="text-xl font-semibold text-gray-900 dark:text-white">
							Edit Visualization Transform
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
							placeholder="Enter visualization transform title"
						/>
					</div>
				{:else}
					<!-- View Mode -->
					<div class="flex justify-between items-center">
						<div class="flex items-baseline gap-3">
							<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
								{transform.title}
							</h1>
							<span class="text-sm text-gray-500 dark:text-gray-400">
								#{transform.visualization_transform_id}
							</span>
						</div>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={startEdit}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
								title="Edit visualization transform"
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
								onclick={() => (transformPendingDelete = true)}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
								title="Delete visualization transform"
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
						</div>
					</div>
				{/if}
			</div>

			<!-- Relationships Card -->
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-4">
				<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Relationships</h2>
				<div class="space-y-4">
					{#if embeddedDataset}
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Embedded Dataset</p>
							<button
								onclick={navigateToEmbeddedDataset}
								class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline"
							>
								{embeddedDataset.title} #{embeddedDataset.embedded_dataset_id}
							</button>
						</div>
					{/if}

					{#if sourceDataset}
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Source Dataset</p>
							<button
								onclick={navigateToDataset}
								class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline"
							>
								{sourceDataset.title} #{sourceDataset.dataset_id}
							</button>
						</div>
					{/if}

					{#if embedder}
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Embedder</p>
							<button
								onclick={navigateToEmbedder}
								class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline"
							>
								{embedder.name} #{embedder.embedder_id}
							</button>
						</div>
					{/if}

					{#if !embeddedDataset && !sourceDataset && !embedder}
						<p class="text-gray-500 dark:text-gray-400 text-sm">Loading relationships...</p>
					{/if}
				</div>
			</div>
		{/if}
	</div>

	{#if transform}
		<!-- Tabs Panel -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
			<TabPanel {tabs} activeTabId={activeTab} onChange={(tabId: string) => (activeTab = tabId)}>
				{#snippet children(tabId)}
					{#if tabId === 'overview'}
						<!-- Overview Tab -->
						<div class="animate-fadeIn">
							<!-- Selected Visualization Display -->
							{#if selectedVisualization}
								<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4 mb-4">
									<div class="flex items-center justify-between mb-4">
										<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
											Visualization #{selectedVisualization.visualization_id}
										</h3>
										<div class="flex gap-2">
											{#if selectedVisualization.status === 'completed' && selectedVisualization.html_s3_key}
												<Button
													size="sm"
													color="blue"
													onclick={() => downloadHtml(selectedVisualization!)}
												>
													<DownloadOutline class="w-4 h-4 mr-2" />
													Download
												</Button>
											{/if}
											<Button size="sm" color="light" onclick={closeVisualization}>Close</Button>
										</div>
									</div>

									{#if selectedVisualization.status === 'processing'}
										<div class="mb-4">
											<div class="flex justify-between items-center mb-2">
												<span class="text-sm text-gray-700 dark:text-gray-300"
													>{getProgressMessage(selectedVisualization)}</span
												>
												<span class="text-sm font-medium text-blue-600 dark:text-blue-400">
													{getProgressPercent(selectedVisualization)}%
												</span>
											</div>
											<div class="w-full bg-gray-200 rounded-full h-2.5 dark:bg-gray-700">
												<div
													class="bg-blue-600 h-2.5 rounded-full transition-all duration-300"
													style="width: {getProgressPercent(selectedVisualization)}%"
												></div>
											</div>
										</div>
										<div class="flex flex-col items-center justify-center py-12">
											<Spinner size="12" />
											<p class="mt-4 text-gray-600 dark:text-gray-400">
												Generating visualization...
											</p>
										</div>
									{:else if selectedVisualization.status === 'failed'}
										<div class="p-4 bg-red-50 dark:bg-red-900/20 rounded-lg">
											<p class="text-sm text-red-600 dark:text-red-400">
												{selectedVisualization.error_message || 'Unknown error occurred'}
											</p>
										</div>
									{:else if htmlContent}
										<div class="space-y-4">
											<div class="grid grid-cols-2 gap-4">
												<div>
													<span class="text-sm text-gray-600 dark:text-gray-400">Points:</span>
													<span class="ml-2 text-sm font-medium text-gray-900 dark:text-white">
														{formatNumber(selectedVisualization.point_count) || 'N/A'}
													</span>
												</div>
												<div>
													<span class="text-sm text-gray-600 dark:text-gray-400">Clusters:</span>
													<span class="ml-2 text-sm font-medium text-gray-900 dark:text-white">
														{formatNumber(selectedVisualization.cluster_count) || 'N/A'}
													</span>
												</div>
											</div>
											<div class="w-full" style="height: 800px;">
												<iframe
													title="Visualization"
													srcdoc={htmlContent}
													class="w-full h-full border-0 rounded-lg"
													sandbox="allow-scripts allow-same-origin allow-popups allow-forms allow-modals allow-pointer-lock allow-presentation"
												></iframe>
											</div>
										</div>
									{:else}
										<div class="flex justify-center items-center py-12">
											<Spinner size="8" />
										</div>
									{/if}
								</div>
							{/if}

							<!-- Stats Card -->
							{#if statsComputed()}
								<div class="mb-4">
									<h3 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white">
										Statistics
									</h3>
									<div class="grid grid-cols-4 gap-4">
										<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">
												Total Visualizations
											</p>
											<p class="text-2xl font-bold text-gray-900 dark:text-white">
												{statsComputed()!.total}
											</p>
										</div>
										<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Completed</p>
											<p class="text-2xl font-bold text-green-600 dark:text-green-400">
												{statsComputed()!.completed}
											</p>
										</div>
										<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Processing</p>
											<p class="text-2xl font-bold text-blue-600 dark:text-blue-400">
												{statsComputed()!.processing}
											</p>
										</div>
										<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Failed</p>
											<p class="text-2xl font-bold text-red-600 dark:text-red-400">
												{statsComputed()!.failed}
											</p>
										</div>
									</div>
								</div>
							{/if}

							<!-- Transform Metadata -->
							<div>
								<h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
									Transform Details
								</h2>
								<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
									<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
										<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Created</p>
										<p class="text-lg font-medium text-gray-900 dark:text-white">
											{formatDate(transform?.created_at || '')}
										</p>
									</div>
									<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
										<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Updated</p>
										<p class="text-lg font-medium text-gray-900 dark:text-white">
											{formatDate(transform?.updated_at || '')}
										</p>
									</div>
									{#if transform?.last_run_at}
										<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Last Run</p>
											<p class="text-lg font-medium text-gray-900 dark:text-white">
												{formatDate(transform.last_run_at)}
											</p>
										</div>
									{/if}
									{#if transform?.last_run_status}
										<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
											<p class="text-sm text-gray-600 dark:text-gray-400 mb-1">Last Status</p>
											<Badge color={getStatusBadge(transform.last_run_status).color}>
												{getStatusBadge(transform.last_run_status).label}
											</Badge>
										</div>
									{/if}
								</div>
							</div>
						</div>
					{:else if tabId === 'visualizations'}
						<!-- Visualizations List Tab -->
						<div class="animate-fadeIn">
							<div class="flex justify-between items-center mb-4">
								<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
									Visualizations ({visualizations.length})
								</h3>
							</div>

							{#if visualizations.length === 0}
								<div class="text-center py-12">
									<svg
										class="w-16 h-16 mx-auto mb-4 text-gray-400"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
									>
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01"
										></path>
									</svg>
									<p class="text-gray-500 dark:text-gray-400 mb-4">
										No visualizations yet. Trigger the transform to generate one.
									</p>
								</div>
							{:else}
								<div class="overflow-x-auto">
									<Table hoverable>
										<TableHead>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">ID</TableHeadCell>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">Status</TableHeadCell>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">Progress</TableHeadCell
											>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">Points</TableHeadCell>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">Clusters</TableHeadCell
											>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">Created</TableHeadCell>
											<TableHeadCell class="px-4 py-3 text-sm font-semibold">Actions</TableHeadCell>
										</TableHead>
										<TableBody>
											{#each visualizations as visualization (visualization.visualization_id)}
												<tr
													class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50"
												>
													<TableBodyCell class="px-4 py-2"
														>#{visualization.visualization_id}</TableBodyCell
													>
													<TableBodyCell class="px-4 py-2">
														<Badge color={getStatusBadge(visualization.status).color}>
															{getStatusBadge(visualization.status).label}
														</Badge>
													</TableBodyCell>
													<TableBodyCell class="px-4 py-2">
														{#if visualization.status === 'processing'}
															<div class="space-y-1">
																<div class="text-xs text-gray-600 dark:text-gray-400">
																	{getProgressMessage(visualization)}
																</div>
																<div class="w-full bg-gray-200 rounded-full h-1.5 dark:bg-gray-700">
																	<div
																		class="bg-blue-600 h-1.5 rounded-full transition-all duration-300"
																		style="width: {getProgressPercent(visualization)}%"
																	></div>
																</div>
															</div>
														{:else if visualization.status === 'failed'}
															<span class="text-xs text-red-600 dark:text-red-400">
																{visualization.error_message?.substring(0, 50) || 'Error'}
															</span>
														{:else}
															<span class="text-xs text-gray-500 dark:text-gray-400">—</span>
														{/if}
													</TableBodyCell>
													<TableBodyCell class="px-4 py-2">
														{formatNumber(visualization.point_count) || '—'}
													</TableBodyCell>
													<TableBodyCell class="px-4 py-2">
														{formatNumber(visualization.cluster_count) || '—'}
													</TableBodyCell>
													<TableBodyCell class="px-4 py-2">
														<span class="text-sm">{formatDate(visualization.created_at)}</span>
													</TableBodyCell>
													<TableBodyCell class="px-4 py-2">
														<div class="flex gap-2">
															{#if visualization.status === 'completed' && visualization.html_s3_key}
																<Button
																	size="xs"
																	color="blue"
																	onclick={() => viewVisualization(visualization)}
																>
																	<EyeOutline class="w-3 h-3 mr-1" />
																	View
																</Button>
																<Button
																	size="xs"
																	color="light"
																	onclick={() => downloadHtml(visualization)}
																>
																	<DownloadOutline class="w-3 h-3 mr-1" />
																	Download
																</Button>
															{:else if visualization.status === 'processing' || visualization.status === 'pending'}
																<Button size="xs" color="light" disabled>Processing...</Button>
															{/if}
														</div>
													</TableBodyCell>
												</tr>
											{/each}
										</TableBody>
									</Table>
								</div>
							{/if}
						</div>
					{/if}
				{/snippet}
			</TabPanel>
		</div>
	{/if}

	<!-- Delete Confirmation Dialog -->
	<ConfirmDialog
		open={transformPendingDelete}
		title="Delete Visualization Transform"
		message="Are you sure you want to delete this visualization transform? This will delete all associated visualizations and cannot be undone."
		confirmLabel="Delete"
		variant="danger"
		onConfirm={confirmDeleteTransform}
		onCancel={() => (transformPendingDelete = false)}
	/>
</div>
