<script lang="ts">
	import {
		Badge,
		Modal,
		Spinner,
		Table,
		TableBody,
		TableBodyCell,
		TableHead,
		TableHeadCell,
	} from 'flowbite-svelte';
	import { ChevronDownOutline, ChevronRightOutline, InfoCircleSolid } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteMap, SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ActionMenu from '../components/ActionMenu.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import SearchInput from '../components/SearchInput.svelte';
	import type {
		EmbeddedDataset,
		LLM,
		PaginatedResponse,
		Visualization,
		VisualizationTransform,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface Props {
		onViewVisualization?: (_id: number) => void;
	}

	import { createPollingInterval } from '../utils/polling';

	let { onViewVisualization }: Props = $props();

	let transforms = $state<VisualizationTransform[]>([]);
	let recentVisualizations = $state.raw(new SvelteMap<number, Visualization[]>());
	let embeddedDatasetCache = $state.raw(new SvelteMap<number, EmbeddedDataset>());
	let llmCache = $state.raw(new SvelteMap<number, LLM>());
	let loading = $state(true);
	let error = $state<string | null>(null);
	let searchQuery = $state('');
	let transformPendingDelete = $state<VisualizationTransform | null>(null);

	// Selection state for bulk operations
	let selected = new SvelteSet<number>();
	let selectAll = $state(false);
	let transformsPendingBulkDelete = $state<VisualizationTransform[]>([]);

	// Config modal state
	let configModalOpen = $state(false);
	let configModalTitle = $state('');
	let configModalJson = $state('');

	// Expand/collapse state for sub-rows
	let expandedTransforms = new SvelteSet<number>();

	// Run selection state for bulk compare
	let selectedRuns = new SvelteSet<string>(); // "transformId:vizId" keys

	let pollingController: ReturnType<typeof createPollingInterval> | null = null;
	let isLoadingTransforms = false;

	onMount(async () => {
		await loadTransforms();
		startPolling();
	});

	onDestroy(() => {
		stopPolling();
	});

	function startPolling() {
		// Create managed polling with deduplication
		pollingController = createPollingInterval(
			async () => {
				// Check if any transforms are processing or pending
				const hasProcessing = transforms.some(
					(t) => t.last_run_status === 'processing' || t.last_run_status === 'pending'
				);

				// Only load if there's processing work and we're not already loading
				if (hasProcessing && !isLoadingTransforms) {
					await loadTransforms();
				}
			},
			{
				interval: 3000,
				shouldContinue: () => true, // Always continue polling
				onError: (error) => {
					console.error('Polling error:', error);
					// Continue polling despite errors
				},
			}
		);
	}

	function stopPolling() {
		if (pollingController) {
			pollingController.stop();
			pollingController = null;
		}
	}

	async function loadTransforms() {
		const isInitialLoad = loading;
		if (isInitialLoad) {
			loading = true;
		}
		error = null;
		isLoadingTransforms = true;

		try {
			const params = new SvelteURLSearchParams();
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			const url = params.toString()
				? `/api/visualization-transforms?${params.toString()}`
				: '/api/visualization-transforms';
			const response = await fetch(url);
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			const data = (await response.json()) as PaginatedResponse<VisualizationTransform>;
			transforms = data.items;

			// Expand all transforms by default
			for (const t of transforms) {
				expandedTransforms.add(t.visualization_transform_id);
			}

			// Load completed visualizations and embedded dataset details for each transform
			await Promise.all([
				loadCompletedVisualizations(),
				loadEmbeddedDatasetDetails(),
				loadLlmDetails(),
			]);
		} catch (err) {
			error = formatError(err);
			if (isInitialLoad) {
				toastStore.error(error);
			}
		} finally {
			isLoadingTransforms = false;
			if (isInitialLoad) {
				loading = false;
			}
		}
	}

	async function loadCompletedVisualizations() {
		const newRecentVisualizations = new SvelteMap<number, Visualization[]>();

		for (const transform of transforms) {
			try {
				const response = await fetch(
					`/api/visualization-transforms/${transform.visualization_transform_id}/visualizations?limit=5`
				);
				if (response.ok) {
					const visualizations: Visualization[] = await response.json();
					if (visualizations.length > 0) {
						newRecentVisualizations.set(transform.visualization_transform_id, visualizations);
					}
				}
			} catch (err) {
				console.error(
					`Failed to load visualizations for transform ${transform.visualization_transform_id}:`,
					err
				);
			}
		}

		recentVisualizations = newRecentVisualizations;
	}

	async function loadEmbeddedDatasetDetails() {
		const newCache = new SvelteMap<number, EmbeddedDataset>();
		const idsToFetch = new SvelteSet<number>();

		for (const transform of transforms) {
			if (!embeddedDatasetCache.has(transform.embedded_dataset_id)) {
				idsToFetch.add(transform.embedded_dataset_id);
			} else {
				newCache.set(
					transform.embedded_dataset_id,
					embeddedDatasetCache.get(transform.embedded_dataset_id)!
				);
			}
		}

		const fetchPromises = Array.from(idsToFetch).map(async (id) => {
			try {
				const response = await fetch(`/api/embedded-datasets/${id}`);
				if (response.ok) {
					const ed: EmbeddedDataset = await response.json();
					newCache.set(id, ed);
				}
			} catch (err) {
				console.error(`Failed to load embedded dataset ${id}:`, err);
			}
		});

		await Promise.all(fetchPromises);
		embeddedDatasetCache = newCache;
	}

	async function loadLlmDetails() {
		const newCache = new SvelteMap<number, LLM>();
		const idsToFetch = new SvelteSet<number>();

		for (const transform of transforms) {
			const llmId = transform.visualization_config.topic_naming_llm_id;
			if (llmId != null) {
				if (!llmCache.has(llmId)) {
					idsToFetch.add(llmId);
				} else {
					newCache.set(llmId, llmCache.get(llmId)!);
				}
			}
		}

		if (idsToFetch.size === 0) {
			llmCache = newCache;
			return;
		}

		// Fetch all LLMs in one call and pick out the ones we need
		try {
			const response = await fetch('/api/llms?limit=1000');
			if (response.ok) {
				const data = await response.json();
				const llms: LLM[] = data.items ?? data;
				for (const llm of llms) {
					if (idsToFetch.has(llm.llm_id)) {
						newCache.set(llm.llm_id, llm);
					}
				}
			}
		} catch (err) {
			console.error('Failed to load LLMs:', err);
		}

		llmCache = newCache;
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

	function toggleSelectAll() {
		selectAll = !selectAll;
		if (selectAll) {
			selected.clear();
			for (const t of filteredTransforms) {
				selected.add(t.visualization_transform_id);
			}
		} else {
			selected.clear();
		}
	}

	function toggleSelect(id: number) {
		if (selected.has(id)) {
			selected.delete(id);
			selectAll = false;
		} else {
			selected.add(id);
		}
	}

	function bulkDelete() {
		const toDelete: VisualizationTransform[] = [];
		for (const id of selected) {
			const transform = transforms.find((t) => t.visualization_transform_id === id);
			if (transform) {
				toDelete.push(transform);
			}
		}
		if (toDelete.length > 0) {
			transformsPendingBulkDelete = toDelete;
		}
	}

	async function confirmBulkDelete() {
		const toDelete = transformsPendingBulkDelete;
		transformsPendingBulkDelete = [];

		for (const transform of toDelete) {
			try {
				const response = await fetch(
					`/api/visualization-transforms/${transform.visualization_transform_id}`,
					{
						method: 'DELETE',
					}
				);

				if (!response.ok) {
					const errorData = await response.json();
					throw new Error(errorData.error || `Failed to delete: ${response.statusText}`);
				}

				// Remove from local list
				transforms = transforms.filter(
					(t) => t.visualization_transform_id !== transform.visualization_transform_id
				);
			} catch (e) {
				toastStore.error(formatError(e, `Failed to delete "${transform.title}"`));
			}
		}

		selected.clear();
		selectAll = false;
		toastStore.success(
			`Deleted ${toDelete.length} visualization${toDelete.length !== 1 ? 's' : ''}`
		);
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

			// Read as text since the response is HTML, then create a blob for download
			const text = await downloadResponse.text();
			const blob = new Blob([text], { type: 'text/html' });
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

	// Refetch when search query changes
	$effect(() => {
		searchQuery;
		loadTransforms();
	});

	let filteredTransforms = $derived(
		transforms.filter((v) => {
			// Only show transforms that have at least one completed visualization
			const vizList = recentVisualizations.get(v.visualization_transform_id);
			if (!vizList?.some((vis) => vis.status === 'completed')) return false;
			return true;
		})
	);

	let pendingTransforms = $derived(
		transforms.filter((t) => {
			const hasPending = t.last_run_status === 'pending' || t.last_run_status === 'processing';
			const vizList = recentVisualizations.get(t.visualization_transform_id);
			return hasPending && !vizList?.some((vis) => vis.status === 'completed');
		})
	);

	let processingTransforms = $derived(
		transforms.filter((t) => {
			const vizList = recentVisualizations.get(t.visualization_transform_id);
			const completedViz = vizList?.find((vis) => vis.status === 'completed');
			if (!completedViz) return false;

			const isProcessing = t.last_run_status === 'pending' || t.last_run_status === 'processing';
			if (!isProcessing) return false;

			// Check if the completed visualization is from the current run
			// If completed_at is after last_run_at, the job is done (status may be stale)
			if (completedViz.completed_at && t.last_run_at) {
				const completedAt = new Date(completedViz.completed_at).getTime();
				const lastRunAt = new Date(t.last_run_at).getTime();
				// If the visualization completed after the run started, it's done
				if (completedAt >= lastRunAt) {
					return false;
				}
			}

			return true;
		})
	);

	function toggleExpand(transformId: number) {
		if (expandedTransforms.has(transformId)) {
			expandedTransforms.delete(transformId);
		} else {
			expandedTransforms.add(transformId);
		}
	}

	function getLatestCompleted(transformId: number): Visualization | undefined {
		const vizList = recentVisualizations.get(transformId);
		return vizList?.find((v) => v.status === 'completed');
	}

	function statusColor(status: string): 'green' | 'red' | 'blue' | 'yellow' | 'gray' {
		switch (status) {
			case 'completed':
				return 'green';
			case 'failed':
				return 'red';
			case 'processing':
				return 'blue';
			case 'pending':
				return 'yellow';
			default:
				return 'gray';
		}
	}

	function toggleRunSelection(transformId: number, vizId: number) {
		const key = `${transformId}:${vizId}`;
		if (selectedRuns.has(key)) {
			selectedRuns.delete(key);
		} else {
			selectedRuns.add(key);
		}
	}

	function openCompareView() {
		const ids = Array.from(selectedRuns).map((key) => {
			const [transformId, vizId] = key.split(':');
			return `${transformId}-${vizId}`;
		});
		window.location.hash = `/visualizations/compare?ids=${ids.join(',')}`;
	}
</script>

<div class="mx-auto">
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
					<InfoCircleSolid class="w-4 h-4" />
					Visualizations will appear in the list below once generation is complete.
				</span>
			</div>
		</div>
	{/if}

	{#if !loading}
		<div class="flex gap-3 items-start">
			<div class="flex-1">
				<SearchInput
					bind:value={searchQuery}
					placeholder="Search visualization transforms by title, owner, or embedded dataset..."
				/>
			</div>
			{#if selectedRuns.size > 0}
				<button
					onclick={openCompareView}
					class="whitespace-nowrap mt-0 px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-700 text-white transition-colors"
				>
					Compare {selectedRuns.size} Run{selectedRuns.size !== 1 ? 's' : ''}
				</button>
				<button
					onclick={() => selectedRuns.clear()}
					class="whitespace-nowrap mt-0 px-4 py-2 text-sm rounded-lg bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-300 transition-colors"
				>
					Clear
				</button>
			{/if}
			<button
				onclick={() => {
					window.location.hash = '/visualization-transforms?create=true';
				}}
				class="btn-primary whitespace-nowrap mt-0"
			>
				Create Visualization
			</button>
		</div>
	{/if}

	{#if loading}
		<LoadingState message="Loading visualizations..." />
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
		{#if selected.size > 0}
			<div
				class="mb-4 flex items-center gap-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4"
			>
				<span class="text-sm text-blue-700 dark:text-blue-300 flex-1">
					{selected.size} visualization{selected.size !== 1 ? 's' : ''} selected
				</span>
				<button
					onclick={() => bulkDelete()}
					class="text-sm px-3 py-1 rounded bg-red-600 hover:bg-red-700 text-white transition-colors"
				>
					Delete
				</button>
				<button
					onclick={() => {
						selected.clear();
						selectAll = false;
					}}
					class="text-sm px-3 py-1 rounded bg-gray-300 hover:bg-gray-400 dark:bg-gray-600 dark:hover:bg-gray-500 text-gray-900 dark:text-white transition-colors"
				>
					Clear
				</button>
			</div>
		{/if}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 w-12">
						<input
							type="checkbox"
							checked={selectAll}
							onchange={() => toggleSelectAll()}
							class="cursor-pointer"
						/>
					</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Title</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Embedded Dataset</TableHeadCell>
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
						{@const completedViz = getLatestCompleted(viz.visualization_transform_id)}
						{@const vizRuns = recentVisualizations.get(viz.visualization_transform_id) ?? []}
						{@const isExpanded = expandedTransforms.has(viz.visualization_transform_id)}
						{@const isProcessing =
							viz.last_run_status === 'processing' || viz.last_run_status === 'pending'}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-2 w-12">
								<input
									type="checkbox"
									checked={selected.has(viz.visualization_transform_id)}
									onchange={() => toggleSelect(viz.visualization_transform_id)}
									class="cursor-pointer"
								/>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2">
								<div class="flex items-center gap-1">
									<button
										onclick={() => toggleExpand(viz.visualization_transform_id)}
										class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 p-0.5 -ml-1"
										title={isExpanded ? 'Collapse runs' : 'Expand runs'}
									>
										{#if isExpanded}
											<ChevronDownOutline class="w-3.5 h-3.5" />
										{:else}
											<ChevronRightOutline class="w-3.5 h-3.5" />
										{/if}
									</button>
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
											<span class="text-gray-500 dark:text-gray-500"
												>{vizRuns.length} run{vizRuns.length !== 1 ? 's' : ''}</span
											>
										</div>
									</div>
								</div>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2">
								{@const ed = embeddedDatasetCache.get(viz.embedded_dataset_id)}
								{@const llm =
									viz.visualization_config.topic_naming_llm_id != null
										? llmCache.get(viz.visualization_config.topic_naming_llm_id)
										: null}
								<div class="text-sm space-y-1">
									<div>
										<a
											href="#/embedded-datasets/{viz.embedded_dataset_id}/details"
											class="font-medium text-purple-600 dark:text-purple-400 hover:underline"
										>
											{ed ? ed.title : `ED #${viz.embedded_dataset_id}`}
										</a>
									</div>
									{#if ed?.source_dataset_title && ed.source_dataset_id}
										<div class="text-xs text-gray-600 dark:text-gray-400">
											<span class="text-gray-500 dark:text-gray-500">Dataset:</span>
											<a
												href="#/datasets/{ed.source_dataset_id}/details"
												class="text-blue-600 dark:text-blue-400 hover:underline"
											>
												{ed.source_dataset_title}
											</a>
										</div>
									{/if}
									{#if ed?.embedder_name && ed.embedder_id}
										<div class="text-xs text-gray-600 dark:text-gray-400">
											<span class="text-gray-500 dark:text-gray-500">Embedder:</span>
											<a
												href="#/embedders/{ed.embedder_id}/details"
												class="text-blue-600 dark:text-blue-400 hover:underline"
											>
												{ed.embedder_name}
											</a>
										</div>
									{/if}
									{#if llm}
										<div class="text-xs text-gray-600 dark:text-gray-400">
											<span class="text-gray-500 dark:text-gray-500">LLM:</span>
											<a href="#/llms" class="text-blue-600 dark:text-blue-400 hover:underline">
												{llm.name}
											</a>
										</div>
									{:else if viz.visualization_config.topic_naming_llm_id != null}
										<div class="text-xs text-gray-500 dark:text-gray-500">
											LLM #{viz.visualization_config.topic_naming_llm_id}
										</div>
									{/if}
								</div>
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
									<button
										onclick={() => {
											configModalTitle = viz.title;
											configModalJson = JSON.stringify(viz.visualization_config, null, 2);
											configModalOpen = true;
										}}
										class="text-blue-600 dark:text-blue-400 hover:underline mt-0.5"
									>
										View all
									</button>
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
						<!-- Expandable sub-rows: latest visualization runs -->
						{#if isExpanded}
							{#each vizRuns as run (run.visualization_id)}
								<tr class="bg-gray-50 dark:bg-gray-900/40 border-b dark:border-gray-700">
									<TableBodyCell class="px-4 py-1.5 w-12">
										{#if run.status === 'completed' && run.html_s3_key}
											<div class="flex justify-center pl-3">
												<input
													type="checkbox"
													checked={selectedRuns.has(
														`${viz.visualization_transform_id}:${run.visualization_id}`
													)}
													onchange={() =>
														toggleRunSelection(
															viz.visualization_transform_id,
															run.visualization_id
														)}
													class="cursor-pointer w-3.5 h-3.5 accent-purple-600 rounded-sm"
													title="Select for comparison"
												/>
											</div>
										{/if}
									</TableBodyCell>
									<TableBodyCell class="px-4 py-1.5">
										<div class="flex items-center gap-2 pl-5">
											<span class="text-xs text-gray-700 dark:text-gray-300">
												Run #{run.visualization_id}
											</span>
										</div>
									</TableBodyCell>
									<TableBodyCell class="px-4 py-1.5">
										<span class="text-xs text-gray-500 dark:text-gray-500">
											{formatDate(run.created_at)}
										</span>
									</TableBodyCell>
									<TableBodyCell class="px-4 py-1.5 text-center">
										<Badge color={statusColor(run.status)} class="text-xs">
											{run.status.charAt(0).toUpperCase() + run.status.slice(1)}
										</Badge>
										{#if run.completed_at}
											<div class="text-xs text-gray-500 dark:text-gray-500 mt-0.5">
												{formatDate(run.completed_at)}
											</div>
										{/if}
									</TableBodyCell>
									<TableBodyCell class="px-4 py-1.5 text-center">
										{#if run.point_count != null || run.cluster_count != null}
											<div class="text-xs">
												{#if run.point_count != null}
													<span class="text-gray-700 dark:text-gray-300"
														>{run.point_count.toLocaleString()} pts</span
													>
												{/if}
												{#if run.cluster_count != null}
													<span class="text-gray-500 dark:text-gray-500">
														· {run.cluster_count} cl</span
													>
												{/if}
											</div>
										{:else}
											<span class="text-gray-400 text-xs">—</span>
										{/if}
									</TableBodyCell>
									<TableBodyCell class="px-4 py-1.5">
										{#if run.error_message}
											<span
												class="text-xs text-red-600 dark:text-red-400 truncate block max-w-50"
												title={run.error_message}
											>
												{run.error_message}
											</span>
										{:else if run.stats_json?.processing_duration_ms && typeof run.stats_json.processing_duration_ms === 'number'}
											<span class="text-xs text-gray-500 dark:text-gray-500">
												{(run.stats_json.processing_duration_ms / 1000).toFixed(1)}s
											</span>
										{/if}
									</TableBodyCell>
									<TableBodyCell class="px-4 py-1.5 text-center">
										{#if run.status === 'completed' && run.html_s3_key}
											<button
												onclick={async () => {
													try {
														const resp = await fetch(
															`/api/visualization-transforms/${viz.visualization_transform_id}/visualizations/${run.visualization_id}/download`
														);
														if (!resp.ok) throw new Error(resp.statusText);
														const text = await resp.text();
														const blob = new Blob([text], { type: 'text/html' });
														const url = window.URL.createObjectURL(blob);
														const a = document.createElement('a');
														a.href = url;
														a.download = `visualization-${viz.visualization_transform_id}-${run.visualization_id}.html`;
														document.body.appendChild(a);
														a.click();
														window.URL.revokeObjectURL(url);
														document.body.removeChild(a);
													} catch (e) {
														toastStore.error(formatError(e, 'Download failed'));
													}
												}}
												class="text-xs text-blue-600 dark:text-blue-400 hover:underline"
											>
												Download
											</button>
										{/if}
									</TableBodyCell>
								</tr>
							{/each}
						{/if}
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
	onConfirm={confirmDeleteTransform}
	onCancel={() => (transformPendingDelete = null)}
/>

<ConfirmDialog
	open={transformsPendingBulkDelete.length > 0}
	title="Delete Visualization Transforms"
	message={`Are you sure you want to delete ${transformsPendingBulkDelete.length} visualization transform${transformsPendingBulkDelete.length !== 1 ? 's' : ''}? This will also delete all associated visualizations. This action cannot be undone.`}
	confirmLabel="Delete All"
	variant="danger"
	onConfirm={confirmBulkDelete}
	onCancel={() => (transformsPendingBulkDelete = [])}
/>

<Modal
	bind:open={configModalOpen}
	size="lg"
	title="{configModalTitle} — Config"
	class="dark:bg-gray-800"
>
	<pre
		class="bg-gray-100 dark:bg-gray-900 text-gray-800 dark:text-gray-200 text-sm rounded-lg p-4 overflow-auto max-h-[70vh] whitespace-pre-wrap">{configModalJson}</pre>
</Modal>
