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
	import { DownloadOutline, EyeOutline, ArrowLeftOutline } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import type { Visualization, VisualizationTransform } from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { createSSEConnection, type SSEConnection } from '../utils/sse';
	import { formatDate, formatNumber } from '../utils/ui-helpers';

	interface Props {
		visualizationTransformId: number;
		onBack: () => void;
	}

	let { visualizationTransformId, onBack }: Props = $props();

	let transform = $state<VisualizationTransform | null>(null);
	let visualizations = $state<Visualization[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let selectedVisualization = $state<Visualization | null>(null);
	let htmlContent = $state<string | null>(null);

	// SSE connection for real-time status updates
	let sseConnection: SSEConnection | null = null;

	onMount(async () => {
		await loadTransform();
		await loadVisualizations();
		connectSSE();
	});

	onDestroy(() => {
		sseConnection?.disconnect();
	});

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

			const blob = await response.blob();
			htmlContent = await blob.text();
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

			// Create a blob and download it
			const blob = await response.blob();
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

<div class="max-w-7xl mx-auto">
	<div class="mb-4">
		<button onclick={onBack} class="mb-4 btn-secondary inline-flex items-center gap-2">
			<ArrowLeftOutline class="w-5 h-5" />
			Back to Visualizations
		</button>

		{#if loading && !transform}
			<LoadingState message="Loading visualization..." />
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
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
				<div class="flex justify-between items-start mb-2">
					<div class="flex-1">
						<div class="flex items-baseline gap-3 mb-2">
							<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
								{transform.title}
							</h1>
							<span class="text-sm text-gray-500 dark:text-gray-400">
								#{transform.visualization_transform_id}
							</span>
						</div>
					</div>
				</div>
			</div>

			<!-- Selected Visualization Display -->
			{#if selectedVisualization}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
					<div class="flex items-center justify-between mb-4">
						<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
							Visualization #{selectedVisualization.visualization_id}
						</h3>
						<div class="flex gap-2">
							{#if selectedVisualization.status === 'completed' && selectedVisualization.html_s3_key}
								<Button size="sm" color="blue" onclick={() => downloadHtml(selectedVisualization!)}>
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
							<p class="mt-4 text-gray-600 dark:text-gray-400">Generating visualization...</p>
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
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
					<h3 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white">Overview</h3>
					<div class="grid grid-cols-4 gap-4">
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Total Visualizations</p>
							<p class="text-2xl font-bold text-gray-900 dark:text-white">
								{statsComputed()!.total}
							</p>
						</div>
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Completed</p>
							<p class="text-2xl font-bold text-green-600 dark:text-green-400">
								{statsComputed()!.completed}
							</p>
						</div>
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Processing</p>
							<p class="text-2xl font-bold text-blue-600 dark:text-blue-400">
								{statsComputed()!.processing}
							</p>
						</div>
						<div>
							<p class="text-sm text-gray-600 dark:text-gray-400">Failed</p>
							<p class="text-2xl font-bold text-red-600 dark:text-red-400">
								{statsComputed()!.failed}
							</p>
						</div>
					</div>
				</div>
			{/if}

			<!-- Visualizations List -->
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
				<h3 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white">
					Visualizations ({visualizations.length})
				</h3>

				{#if visualizations.length === 0}
					<p class="text-sm text-gray-600 dark:text-gray-400">
						No visualizations yet. Trigger the transform to generate one.
					</p>
				{:else}
					<div class="overflow-x-auto">
						<Table hoverable>
							<TableHead>
								<TableHeadCell class="px-4 py-3 text-sm font-semibold">ID</TableHeadCell>
								<TableHeadCell class="px-4 py-3 text-sm font-semibold">Status</TableHeadCell>
								<TableHeadCell class="px-4 py-3 text-sm font-semibold">Progress</TableHeadCell>
								<TableHeadCell class="px-4 py-3 text-sm font-semibold">Points</TableHeadCell>
								<TableHeadCell class="px-4 py-3 text-sm font-semibold">Clusters</TableHeadCell>
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
	</div>
</div>
