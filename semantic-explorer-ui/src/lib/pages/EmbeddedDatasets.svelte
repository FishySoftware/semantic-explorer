<script lang="ts">
	import { onMount } from 'svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import ActionMenu from '../components/ActionMenu.svelte';
	import { Table, TableBody, TableHead, TableHeadCell, TableBodyCell } from 'flowbite-svelte';
	import { formatError, toastStore } from '../utils/notifications';

	let { onNavigate, onViewDataset } = $props<{
		onNavigate: (_path: string) => void;
		onViewDataset: (_datasetId: number) => void;
	}>();

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
		// These will be populated after fetching
		source_dataset_title?: string;
		embedder_name?: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
		details: string | null;
		owner: string;
		tags: string[];
		item_count?: number;
		total_chunks?: number;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
		owner: string;
		provider: string;
		base_url: string;
		api_key: string | null;
		config: Record<string, any>;
		max_batch_size?: number;
		dimensions?: number;
		collection_name: string;
		created_at: string;
		updated_at: string;
	}

	interface Stats {
		embedded_dataset_id: number;
		total_batches_processed: number;
		successful_batches: number;
		failed_batches: number;
		total_chunks_embedded: number;
		total_chunks_failed: number;
	}

	interface ProcessedBatch {
		id: number;
		embedded_dataset_id: number;
		file_key: string;
		processed_at: string;
		item_count: number;
		process_status: string;
		process_error: string | null;
		processing_duration_ms: number | null;
	}

	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let datasetsCache = $state<Map<number, Dataset>>(new Map());
	let embeddersCache = $state<Map<number, Embedder>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	// Failed batches modal state
	let showFailedBatchesModal = $state(false);
	let failedBatchesDatasetTitle = $state('');
	let failedBatches = $state<ProcessedBatch[]>([]);
	let loadingFailedBatches = $state(false);

	async function fetchDataset(datasetId: number): Promise<Dataset | null> {
		if (datasetsCache.has(datasetId)) {
			return datasetsCache.get(datasetId) || null;
		}
		try {
			const response = await fetch(`/api/datasets/${datasetId}`);
			if (response.ok) {
				const dataset = await response.json();
				datasetsCache.set(datasetId, dataset);
				datasetsCache = datasetsCache; // Trigger reactivity
				return dataset;
			}
		} catch (e) {
			console.error(`Failed to fetch dataset ${datasetId}:`, e);
		}
		return null;
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
				embeddersCache = embeddersCache; // Trigger reactivity
				return embedder;
			}
		} catch (e) {
			console.error(`Failed to fetch embedder ${embedderId}:`, e);
		}
		return null;
	}

	async function fetchEmbeddedDatasets() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/embedded-datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			embeddedDatasets = await response.json();

			// Fetch related datasets and embedders
			for (const dataset of embeddedDatasets) {
				const sourceDataset = await fetchDataset(dataset.source_dataset_id);
				if (sourceDataset) {
					dataset.source_dataset_title = sourceDataset.title;
				}

				const embedder = await fetchEmbedder(dataset.embedder_id);
				if (embedder) {
					dataset.embedder_name = embedder.name;
				}

				await fetchStatsForEmbeddedDataset(dataset.embedded_dataset_id);
			}

			// Trigger reactivity
			embeddedDatasets = embeddedDatasets;
		} catch (e) {
			const message = formatError(e, 'Failed to fetch embedded datasets');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchStatsForEmbeddedDataset(embeddedDatasetId: number) {
		try {
			const response = await fetch(`/api/embedded-datasets/${embeddedDatasetId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(embeddedDatasetId, stats);
				statsMap = statsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(`Failed to fetch stats for embedded dataset ${embeddedDatasetId}:`, e);
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
				// Filter to only failed batches
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

	async function deleteEmbeddedDataset(dataset: EmbeddedDataset) {
		if (
			!confirm(
				`Are you sure you want to delete "${dataset.title}"?\n\nThis will permanently delete:\n- The database record\n- The Qdrant collection: ${dataset.collection_name}\n\nThis action cannot be undone.`
			)
		) {
			return;
		}

		try {
			const response = await fetch(`/api/embedded-datasets/${dataset.embedded_dataset_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || `Failed to delete: ${response.statusText}`);
			}

			toastStore.success(`Successfully deleted embedded dataset "${dataset.title}"`);
			// Remove from local list
			embeddedDatasets = embeddedDatasets.filter(
				(d) => d.embedded_dataset_id !== dataset.embedded_dataset_id
			);
			statsMap.delete(dataset.embedded_dataset_id);
			statsMap = statsMap; // Trigger reactivity
		} catch (e) {
			const message = formatError(e, 'Failed to delete embedded dataset');
			toastStore.error(message);
		}
	}

	onMount(() => {
		fetchEmbeddedDatasets();
	});

	let filteredDatasets = $derived(
		embeddedDatasets.filter((d) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				d.title.toLowerCase().includes(query) ||
				(d.source_dataset_title?.toLowerCase().includes(query) ?? false) ||
				(d.embedder_name?.toLowerCase().includes(query) ?? false) ||
				d.owner.toLowerCase().includes(query) ||
				d.collection_name.toLowerCase().includes(query)
			);
		})
	);
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Embedded Datasets"
		description="Embedded Datasets contain vector embeddings stored in Qdrant collections. They are automatically created when Dataset Transforms are executed. Each Embedded Dataset represents one embedder applied to a source dataset, ready for semantic search and visualization."
	/>

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
			placeholder="Search embedded datasets..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading embedded datasets...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if filteredDatasets.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-4 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No embedded datasets found matching your search.'
					: 'No embedded datasets yet. Create a Dataset Transform to generate Embedded Datasets.'}
			</p>
		</div>
	{:else}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Title</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Source Dataset</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Embedder</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Success Rate</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Chunks</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredDatasets as dataset (dataset.embedded_dataset_id)}
						{@const stats = statsMap.get(dataset.embedded_dataset_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3">
								<span class="font-semibold text-gray-900 dark:text-white">
									{dataset.title}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								{#if dataset.source_dataset_title}
									<button
										onclick={() => onViewDataset(dataset.source_dataset_id)}
										class="text-blue-600 dark:text-blue-400 hover:underline font-semibold"
									>
										{dataset.source_dataset_title}
									</button>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">Loading...</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								{#if dataset.embedder_name}
									<button
										onclick={() =>
											onNavigate(
												`/embedders?search=${encodeURIComponent(dataset.embedder_name ?? '')}`
											)}
										class="text-blue-600 dark:text-blue-400 hover:underline font-semibold text-sm"
									>
										{dataset.embedder_name}
									</button>
								{:else}
									<span class="text-gray-500 dark:text-gray-400 text-sm">Loading...</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if stats}
									<span
										class="inline-block px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-sm font-medium"
									>
										{stats.total_batches_processed > 0
											? Math.round((stats.successful_batches / stats.total_batches_processed) * 100)
											: 0}%
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if stats}
									<span class="text-gray-700 dark:text-gray-300 text-sm">
										{stats.total_chunks_embedded}
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								<ActionMenu
									actions={[
										...(stats && stats.failed_batches > 0
											? [
													{
														label: 'View Failed',
														handler: () => openFailedBatchesModal(dataset),
													},
												]
											: []),
										{
											label: 'Delete',
											handler: () => deleteEmbeddedDataset(dataset),
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
											Processed: {new Date(batch.processed_at).toLocaleString()} | Items: {batch.item_count}
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
