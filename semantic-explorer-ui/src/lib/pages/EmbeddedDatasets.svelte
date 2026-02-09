<script lang="ts">
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ActionMenu from '../components/ActionMenu.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import SearchInput from '../components/SearchInput.svelte';
	import type {
		Dataset,
		EmbeddedDataset,
		Embedder,
		PaginatedEmbeddedDatasetList,
		ProcessedBatch,
		EmbeddedDatasetStats as Stats,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate, formatNumber } from '../utils/ui-helpers';

	let { onNavigate, onViewDataset } = $props<{
		onNavigate: (_path: string) => void;
		onViewDataset: (_datasetId: number) => void;
	}>();

	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let datasetsCache = $state<Map<number, Dataset>>(new Map());
	let embeddersCache = $state<Map<number, Embedder>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);
	let totalCount = $state(0);
	let currentOffset = $state(0);
	const pageSize = 20;

	let searchQuery = $state('');

	// Failed batches modal state
	let showFailedBatchesModal = $state(false);
	let failedBatchesDatasetTitle = $state('');
	let failedBatches = $state<ProcessedBatch[]>([]);
	let loadingFailedBatches = $state(false);

	// Rename modal state
	let showRenameModal = $state(false);
	let renamingDatasetId = $state<number | null>(null);
	let newTitle = $state('');
	let renaming = $state(false);

	// Create standalone modal state
	let showCreateStandaloneModal = $state(false);
	let standaloneTitle = $state('');
	let standaloneDimensions = $state(1536);
	let creatingStandalone = $state(false);

	// Delete confirmation state
	let datasetPendingDelete = $state<EmbeddedDataset | null>(null);

	// Selection state for bulk operations
	let selected = new SvelteSet<number>();
	let selectAll = $state(false);
	let datasetsPendingBulkDelete = $state<EmbeddedDataset[]>([]);

	let filteredDatasets = $derived(embeddedDatasets);

	async function fetchDataset(datasetId: number): Promise<Dataset | 'not_found' | null> {
		if (datasetsCache.has(datasetId)) {
			return datasetsCache.get(datasetId) || null;
		}
		try {
			const response = await fetch(`/api/datasets/${datasetId}`);
			if (response.ok) {
				const dataset = await response.json();
				datasetsCache.set(datasetId, dataset);
				datasetsCache = datasetsCache;
				return dataset;
			}
			if (response.status === 404) {
				return 'not_found';
			}
		} catch (e) {
			console.error(`Failed to fetch dataset ${datasetId}:`, e);
		}
		return 'not_found';
	}

	async function fetchEmbedder(embedderId: number): Promise<Embedder | null> {
		if (embeddersCache.has(embedderId)) {
			return embeddersCache.get(embedderId) || null;
		}
		try {
			const response = await fetch(`/api/embedders/${embedderId}`);
			if (response.ok) {
				const embedder = await response.json();
				embeddersCache.set(embedderId, embedder);
				embeddersCache = embeddersCache;
				return embedder;
			}
		} catch (e) {
			console.error(`Failed to fetch embedder ${embedderId}:`, e);
		}
		return null;
	}

	async function fetchEmbeddedDatasets(showLoading = true) {
		try {
			if (showLoading) loading = true;
			error = null;
			const params = new SvelteURLSearchParams();
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			params.append('limit', pageSize.toString());
			params.append('offset', currentOffset.toString());
			const url = `/api/embedded-datasets?${params.toString()}`;
			const response = await fetch(url);
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			const data: PaginatedEmbeddedDatasetList = await response.json();
			embeddedDatasets = data.embedded_datasets;
			totalCount = data.total_count;

			// Fetch all stats in one batch request
			const embeddedDatasetIds = embeddedDatasets.map((d) => d.embedded_dataset_id);
			if (embeddedDatasetIds.length > 0) {
				await fetchBatchStats(embeddedDatasetIds);
			}

			// Fetch related datasets and embedders
			for (const dataset of embeddedDatasets) {
				const standalone =
					dataset.is_standalone === true ||
					(dataset.dataset_transform_id === 0 &&
						dataset.source_dataset_id === 0 &&
						dataset.embedder_id === 0);

				if (!standalone) {
					const sourceDataset = await fetchDataset(dataset.source_dataset_id);
					if (sourceDataset && sourceDataset !== 'not_found') {
						dataset.source_dataset_title = sourceDataset.title;
					} else {
						dataset.source_dataset_title = 'N/A (deleted)';
					}

					const embedder = await fetchEmbedder(dataset.embedder_id);
					if (embedder) {
						dataset.embedder_name = embedder.name;
					} else {
						dataset.embedder_name = 'N/A (deleted)';
					}
				}
			}

			embeddedDatasets = embeddedDatasets;
		} catch (e) {
			const message = formatError(e, 'Failed to fetch embedded datasets');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchBatchStats(embeddedDatasetIds: number[]) {
		try {
			const response = await fetch('/api/embedded-datasets/batch-stats', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ embedded_dataset_ids: embeddedDatasetIds }),
			});
			if (response.ok) {
				const batchStats: Record<number, Stats> = await response.json();
				for (const [idStr, stats] of Object.entries(batchStats)) {
					const id = parseInt(idStr, 10);
					statsMap.set(id, stats);
				}
				statsMap = statsMap;
			}
		} catch (e) {
			console.error('Failed to fetch batch stats:', e);
		}
	}

	async function openFailedBatchesModal(dataset: EmbeddedDataset) {
		failedBatchesDatasetTitle = dataset.title;
		showFailedBatchesModal = true;
		loadingFailedBatches = true;
		failedBatches = [];

		try {
			const response = await fetch(
				`/api/embedded-datasets/${dataset.embedded_dataset_id}/processed-batches`
			);
			if (response.ok) {
				const allBatches: ProcessedBatch[] = await response.json();
				failedBatches = allBatches.filter((b) => b.process_status === 'failed');
			}
		} catch (e) {
			console.error(
				`Failed to fetch processed batches for embedded dataset ${dataset.embedded_dataset_id}:`,
				e
			);
			toastStore.error('Failed to fetch failed batches');
		} finally {
			loadingFailedBatches = false;
		}
	}

	function closeFailedBatchesModal() {
		showFailedBatchesModal = false;
		failedBatchesDatasetTitle = '';
		failedBatches = [];
	}

	function requestDeleteEmbeddedDataset(dataset: EmbeddedDataset) {
		datasetPendingDelete = dataset;
	}

	async function confirmDeleteEmbeddedDataset() {
		if (!datasetPendingDelete) return;

		const target = datasetPendingDelete;
		datasetPendingDelete = null;

		try {
			const response = await fetch(`/api/embedded-datasets/${target.embedded_dataset_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || `Failed to delete: ${response.statusText}`);
			}

			toastStore.success(`Successfully deleted embedded dataset "${target.title}"`);
			embeddedDatasets = embeddedDatasets.filter(
				(d) => d.embedded_dataset_id !== target.embedded_dataset_id
			);
			statsMap.delete(target.embedded_dataset_id);
			statsMap = statsMap;
		} catch (e) {
			const message = formatError(e, 'Failed to delete embedded dataset');
			toastStore.error(message);
		}
	}

	async function renameEmbeddedDataset() {
		if (!renamingDatasetId) return;

		if (!newTitle.trim()) {
			toastStore.error('Title cannot be empty');
			return;
		}

		try {
			renaming = true;
			const response = await fetch(`/api/embedded-datasets/${renamingDatasetId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ title: newTitle.trim() }),
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || `Failed to rename: ${response.statusText}`);
			}

			const updatedDataset = await response.json();
			const index = embeddedDatasets.findIndex((d) => d.embedded_dataset_id === renamingDatasetId);
			if (index !== -1) {
				embeddedDatasets[index].title = updatedDataset.title;
				embeddedDatasets = embeddedDatasets;
			}

			toastStore.success(`Successfully renamed to "${newTitle}"`);
			showRenameModal = false;
			renamingDatasetId = null;
			newTitle = '';
		} catch (e) {
			const message = formatError(e, 'Failed to rename embedded dataset');
			toastStore.error(message);
		} finally {
			renaming = false;
		}
	}

	function openRenameModal(dataset: EmbeddedDataset) {
		renamingDatasetId = dataset.embedded_dataset_id;
		newTitle = dataset.title;
		showRenameModal = true;
	}

	function openCreateStandaloneModal() {
		standaloneTitle = '';
		standaloneDimensions = 1536;
		showCreateStandaloneModal = true;
	}

	function closeCreateStandaloneModal() {
		showCreateStandaloneModal = false;
		standaloneTitle = '';
		standaloneDimensions = 1536;
	}

	async function createStandaloneEmbeddedDataset() {
		if (!standaloneTitle.trim()) {
			toastStore.error('Title is required');
			return;
		}

		if (standaloneDimensions < 1 || standaloneDimensions > 65536) {
			toastStore.error('Dimensions must be between 1 and 65536');
			return;
		}

		try {
			creatingStandalone = true;
			const response = await fetch('/api/embedded-datasets/standalone', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: standaloneTitle.trim(),
					dimensions: standaloneDimensions,
				}),
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || `Failed to create: ${response.statusText}`);
			}

			const newDataset = await response.json();
			toastStore.success(`Successfully created standalone embedded dataset "${standaloneTitle}"`);
			closeCreateStandaloneModal();

			await fetchEmbeddedDatasets();
			onNavigate(`/embedded-datasets/${newDataset.embedded_dataset_id}/details`);
		} catch (e) {
			const message = formatError(e, 'Failed to create standalone embedded dataset');
			toastStore.error(message);
		} finally {
			creatingStandalone = false;
		}
	}

	function isStandalone(dataset: EmbeddedDataset): boolean {
		return (
			dataset.is_standalone === true ||
			(dataset.dataset_transform_id === 0 &&
				dataset.source_dataset_id === 0 &&
				dataset.embedder_id === 0)
		);
	}

	function toggleSelectAll() {
		selectAll = !selectAll;
		if (selectAll) {
			selected.clear();
			for (const d of filteredDatasets) {
				selected.add(d.embedded_dataset_id);
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
		const toDelete: EmbeddedDataset[] = [];
		for (const id of selected) {
			const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === id);
			if (dataset) {
				toDelete.push(dataset);
			}
		}
		if (toDelete.length > 0) {
			datasetsPendingBulkDelete = toDelete;
		}
	}

	async function confirmBulkDelete() {
		const toDelete = datasetsPendingBulkDelete;
		datasetsPendingBulkDelete = [];

		for (const dataset of toDelete) {
			try {
				const response = await fetch(`/api/embedded-datasets/${dataset.embedded_dataset_id}`, {
					method: 'DELETE',
				});

				if (!response.ok) {
					const errorData = await response.json();
					throw new Error(errorData.error || `Failed to delete: ${response.statusText}`);
				}

				embeddedDatasets = embeddedDatasets.filter(
					(d) => d.embedded_dataset_id !== dataset.embedded_dataset_id
				);
				statsMap.delete(dataset.embedded_dataset_id);
			} catch (e) {
				toastStore.error(formatError(e, `Failed to delete "${dataset.title}"`));
			}
		}

		statsMap = statsMap;
		selected.clear();
		selectAll = false;
		toastStore.success(
			`Deleted ${toDelete.length} embedded dataset${toDelete.length !== 1 ? 's' : ''}`
		);
	}

	function goToPreviousPage() {
		currentOffset = Math.max(0, currentOffset - pageSize);
		fetchEmbeddedDatasets();
	}

	function goToNextPage() {
		if (currentOffset + pageSize < totalCount) {
			currentOffset += pageSize;
			fetchEmbeddedDatasets();
		}
	}

	// Debounce search to avoid spamming API on every keystroke
	let searchDebounceTimeout: ReturnType<typeof setTimeout> | null = null;
	$effect(() => {
		if (searchQuery !== undefined) {
			currentOffset = 0;
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
			searchDebounceTimeout = setTimeout(() => {
				fetchEmbeddedDatasets();
			}, 300);
		}
		return () => {
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
		};
	});

	// Auto-refresh every 5 seconds
	let refreshInterval: ReturnType<typeof setInterval> | null = null;
	$effect(() => {
		refreshInterval = setInterval(() => {
			fetchEmbeddedDatasets(false);
		}, 5000);
		return () => {
			if (refreshInterval) {
				clearInterval(refreshInterval);
			}
		};
	});
</script>

<div class="mx-auto">
	<PageHeader
		title="Embedded Datasets"
		description="Vector embeddings stored in Qdrant collections, used for semantic search, chat, and visualizations. Created automatically via Dataset Transforms, or manually as standalone datasets where you push vectors directly via API."
	/>

	<div class="flex justify-between items-center mb-4">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Embedded Datasets</h1>
		<button onclick={openCreateStandaloneModal} class="btn-primary"> Create Standalone </button>
	</div>

	{#if !showCreateStandaloneModal}
		<SearchInput
			bind:value={searchQuery}
			placeholder="Search embedded datasets by title, source, embedder, or owner..."
		/>
	{/if}

	{#if loading}
		<LoadingState message="Loading embedded datasets..." />
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
			<button
				onclick={() => fetchEmbeddedDatasets()}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if embeddedDatasets.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">
				{searchQuery ? 'No embedded datasets match your search' : 'No embedded datasets yet'}
			</p>
			{#if searchQuery}
				<button
					onclick={() => (searchQuery = '')}
					class="text-blue-600 dark:text-blue-400 hover:underline"
				>
					Clear search
				</button>
			{:else}
				<p class="text-gray-400 dark:text-gray-500 text-sm mb-4">
					Create a Dataset Transform to generate embedded datasets, or create a standalone dataset
					to push vectors via API.
				</p>
				<button
					onclick={openCreateStandaloneModal}
					class="text-blue-600 dark:text-blue-400 hover:underline"
				>
					Create your first standalone embedded dataset
				</button>
			{/if}
		</div>
	{:else}
		{#if selected.size > 0}
			<div
				class="mb-4 flex items-center gap-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4"
			>
				<span class="text-sm text-blue-700 dark:text-blue-300 flex-1">
					{selected.size} embedded dataset{selected.size !== 1 ? 's' : ''} selected
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
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Source</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Embedder</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Dimensions</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Vectors</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Status</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredDatasets as dataset (dataset.embedded_dataset_id)}
						{@const stats = statsMap.get(dataset.embedded_dataset_id)}
						{@const standalone = isStandalone(dataset)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(dataset.embedded_dataset_id)}
									onchange={() => toggleSelect(dataset.embedded_dataset_id)}
									class="cursor-pointer"
								/>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<div class="flex items-center gap-2">
									<button
										onclick={() =>
											onNavigate(`/embedded-datasets/${dataset.embedded_dataset_id}/details`)}
										class="font-semibold text-blue-600 dark:text-blue-400 hover:underline text-left"
									>
										{dataset.title}
									</button>
									{#if standalone}
										<span
											class="px-1.5 py-0.5 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-xs font-medium"
										>
											Standalone
										</span>
									{/if}
								</div>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								{#if standalone}
									<span class="text-gray-400 dark:text-gray-500 text-sm italic">—</span>
								{:else if dataset.source_dataset_title === 'N/A (deleted)'}
									<span class="text-gray-400 dark:text-gray-500 text-sm italic">Deleted</span>
								{:else if dataset.source_dataset_title}
									<button
										onclick={() => onViewDataset(dataset.source_dataset_id)}
										class="text-blue-600 dark:text-blue-400 hover:underline text-sm font-medium"
									>
										{dataset.source_dataset_title}
									</button>
								{:else}
									<span class="text-gray-500 dark:text-gray-400 text-sm">Loading...</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								{#if standalone}
									<span class="text-gray-400 dark:text-gray-500 text-sm italic">—</span>
								{:else if dataset.embedder_name === 'N/A (deleted)'}
									<span class="text-gray-400 dark:text-gray-500 text-sm italic">Deleted</span>
								{:else if dataset.embedder_name}
									<button
										onclick={() => onNavigate(`/embedders/${dataset.embedder_id}/details`)}
										class="text-blue-600 dark:text-blue-400 hover:underline text-sm font-medium"
									>
										{dataset.embedder_name}
									</button>
								{:else}
									<span class="text-gray-500 dark:text-gray-400 text-sm">Loading...</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if standalone && dataset.dimensions}
									<span
										class="inline-block px-2 py-1 bg-indigo-100 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-300 rounded text-sm font-medium"
									>
										{dataset.dimensions}
									</span>
								{:else}
									{@const embedder = embeddersCache.get(dataset.embedder_id)}
									{#if embedder?.dimensions}
										<span
											class="inline-block px-2 py-1 bg-indigo-100 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-300 rounded text-sm font-medium"
										>
											{embedder.dimensions}
										</span>
									{:else}
										<span class="text-gray-500 dark:text-gray-400">—</span>
									{/if}
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if standalone}
									{#if dataset.active_point_count !== undefined && dataset.active_point_count !== null}
										<span
											class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-sm font-medium"
										>
											{formatNumber(dataset.active_point_count)}
										</span>
									{:else}
										<span class="text-gray-400 dark:text-gray-500 text-xs italic">Push via API</span
										>
									{/if}
								{:else if stats}
									<span
										class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-sm font-medium"
										title="Total chunks embedded"
									>
										{formatNumber(stats.total_chunks_embedded)}
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if standalone}
									<span
										class="inline-block px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-xs font-medium"
									>
										Ready
									</span>
								{:else if stats}
									{@const successRate =
										stats.total_batches_processed > 0
											? Math.round((stats.successful_batches / stats.total_batches_processed) * 100)
											: 0}
									<div class="flex gap-1 justify-center flex-wrap">
										{#if stats.processing_batches > 0}
											<span
												class="inline-block px-2 py-1 bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300 rounded text-xs font-medium"
												title="Processing batches"
											>
												Processing
											</span>
										{:else if stats.failed_batches > 0 && stats.successful_batches === 0}
											<span
												class="inline-block px-2 py-1 bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 rounded text-xs font-medium"
												title="All batches failed"
											>
												Failed
											</span>
										{:else if stats.failed_batches > 0}
											<span
												class="inline-block px-2 py-1 bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300 rounded text-xs font-medium"
												title="{successRate}% success rate, {stats.failed_batches} failed"
											>
												Partial ({successRate}%)
											</span>
										{:else if stats.total_batches_processed > 0}
											<span
												class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-xs font-medium"
												title="All batches successful"
											>
												Complete
											</span>
										{:else}
											<span
												class="inline-block px-2 py-1 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded text-xs font-medium"
											>
												Pending
											</span>
										{/if}
									</div>
								{:else}
									<span class="text-gray-500 dark:text-gray-400 text-xs">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-2 text-center">
								<ActionMenu
									actions={[
										{
											label: 'View Details',
											handler: () =>
												onNavigate(`/embedded-datasets/${dataset.embedded_dataset_id}/details`),
										},
										...(stats && stats.failed_batches > 0
											? [
													{
														label: 'View Failed Batches',
														handler: () => openFailedBatchesModal(dataset),
													},
												]
											: []),
										{
											label: 'Rename',
											handler: () => openRenameModal(dataset),
										},
										{
											label: 'Delete',
											handler: () => requestDeleteEmbeddedDataset(dataset),
											isDangerous: true,
										},
									]}
								/>
							</TableBodyCell>
						</tr>
					{/each}
				</TableBody>
			</Table>

			<!-- Pagination Controls -->
			<div class="mt-6 px-4 pb-4 flex items-center justify-between">
				<div class="text-sm text-gray-600 dark:text-gray-400">
					Showing {currentOffset + 1}-{Math.min(currentOffset + pageSize, totalCount)} of {totalCount}
					embedded datasets
				</div>
				<div class="flex gap-2">
					<button
						onclick={goToPreviousPage}
						disabled={currentOffset === 0}
						class="px-4 py-2 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-100 dark:hover:bg-gray-700"
					>
						Previous
					</button>
					<button
						onclick={goToNextPage}
						disabled={currentOffset + pageSize >= totalCount}
						class="px-4 py-2 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-100 dark:hover:bg-gray-700"
					>
						Next
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>

<!-- Failed Batches Modal -->
{#if showFailedBatchesModal}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
		<div
			class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-4xl w-full mx-4 max-h-[80vh] flex flex-col"
		>
			<div
				class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center"
			>
				<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
					Failed Batches - {failedBatchesDatasetTitle}
				</h3>
				<button
					onclick={closeFailedBatchesModal}
					class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
					aria-label="Close modal"
				>
					<svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M6 18L18 6M6 6l12 12"
						/>
					</svg>
				</button>
			</div>

			<div class="p-4 overflow-y-auto flex-1">
				{#if loadingFailedBatches}
					<div class="flex justify-center py-8">
						<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
					</div>
				{:else if failedBatches.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-center py-8">No failed batches found.</p>
				{:else}
					<div class="space-y-4">
						{#each failedBatches as batch (batch.file_key)}
							<div
								class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
							>
								<div class="flex items-start justify-between">
									<div class="flex-1 min-w-0">
										<p
											class="font-mono text-sm text-gray-900 dark:text-white truncate"
											title={batch.file_key}
										>
											{batch.file_key.split('/').pop() || batch.file_key}
										</p>
										<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
											Processed: {formatDate(batch.processed_at)} | Items: {batch.item_count}
										</p>
									</div>
								</div>
								{#if batch.process_error}
									<div class="mt-3 bg-red-100 dark:bg-red-900/40 rounded p-3">
										<p class="text-xs font-semibold text-red-700 dark:text-red-300 mb-1">Error:</p>
										<pre
											class="text-xs text-red-600 dark:text-red-400 whitespace-pre-wrap wrap-break-word font-mono">{batch.process_error}</pre>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
				<button onclick={closeFailedBatchesModal} class="btn-secondary"> Close </button>
			</div>
		</div>
	</div>
{/if}

<!-- Rename Modal -->
{#if showRenameModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-lg max-w-md w-full mx-4">
			<div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
				<h2 class="text-lg font-semibold text-gray-900 dark:text-white">Rename Embedded Dataset</h2>
			</div>

			<div class="px-6 py-4">
				<label
					for="rename-title"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
				>
					Title
				</label>
				<input
					id="rename-title"
					type="text"
					bind:value={newTitle}
					placeholder="Enter new title"
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
					disabled={renaming}
				/>
			</div>

			<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
				<button
					onclick={() => {
						showRenameModal = false;
						renamingDatasetId = null;
						newTitle = '';
					}}
					class="btn-secondary"
					disabled={renaming}
				>
					Cancel
				</button>
				<button onclick={renameEmbeddedDataset} class="btn-primary" disabled={renaming}>
					{renaming ? 'Renaming...' : 'Rename'}
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Create Standalone Modal -->
{#if showCreateStandaloneModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-lg max-w-md w-full mx-4">
			<div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
				<h2 class="text-lg font-semibold text-gray-900 dark:text-white">
					Create Standalone Embedded Dataset
				</h2>
				<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
					Standalone datasets allow you to push vectors directly. They can be used in visualizations
					but not in search/chat.
				</p>
			</div>

			<div class="px-6 py-4 space-y-4">
				<div>
					<label
						for="standalone-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Title
					</label>
					<input
						id="standalone-title"
						type="text"
						bind:value={standaloneTitle}
						placeholder="My Embedded Dataset"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
						disabled={creatingStandalone}
					/>
				</div>

				<div>
					<label
						for="standalone-dimensions"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Vector Dimensions
					</label>
					<input
						id="standalone-dimensions"
						type="number"
						bind:value={standaloneDimensions}
						min="1"
						max="65536"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
						disabled={creatingStandalone}
					/>
					<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
						Common sizes: 384 (MiniLM), 768 (BERT), 1024 (Cohere), 1536 (OpenAI Ada), 3072 (OpenAI
						Large)
					</p>
				</div>
			</div>

			<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
				<button
					onclick={closeCreateStandaloneModal}
					class="btn-secondary"
					disabled={creatingStandalone}
				>
					Cancel
				</button>
				<button
					onclick={createStandaloneEmbeddedDataset}
					class="btn-primary"
					disabled={creatingStandalone}
				>
					{creatingStandalone ? 'Creating...' : 'Create'}
				</button>
			</div>
		</div>
	</div>
{/if}

<ConfirmDialog
	open={datasetPendingDelete !== null}
	title="Delete embedded dataset"
	message={datasetPendingDelete
		? `Are you sure you want to delete "${datasetPendingDelete.title}"?\n\nThis will permanently delete the database record and the Qdrant collection: ${datasetPendingDelete.collection_name}\n\nThis action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteEmbeddedDataset}
	onCancel={() => (datasetPendingDelete = null)}
/>

<ConfirmDialog
	open={datasetsPendingBulkDelete.length > 0}
	title="Delete Embedded Datasets"
	message={`Are you sure you want to delete ${datasetsPendingBulkDelete.length} embedded dataset${datasetsPendingBulkDelete.length !== 1 ? 's' : ''}? This will permanently delete the database records and Qdrant collections. This action cannot be undone.`}
	confirmLabel="Delete All"
	variant="danger"
	onConfirm={confirmBulkDelete}
	onCancel={() => (datasetsPendingBulkDelete = [])}
/>
