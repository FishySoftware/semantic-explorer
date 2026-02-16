<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import SortableTable from '../components/SortableTable.svelte';
	import StatsGrid from '../components/StatsGrid.svelte';
	import TransformDetailHeader from '../components/TransformDetailHeader.svelte';
	import TransformProgressPanel from '../components/TransformProgressPanel.svelte';
	import type {
		CollectionTransform,
		CollectionTransformStats,
		ProcessedFile,
	} from '../types/models';
	import { toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface Props {
		collectionTransformId: number;
		onBack: () => void;
		onNavigate?: (_page: string, _params?: Record<string, unknown>) => void;
	}

	let { collectionTransformId, onBack, onNavigate }: Props = $props();

	interface Collection {
		collection_id: number;
		title: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface PaginatedFilesResponse {
		items: ProcessedFile[];
		total_count: number;
		limit: number;
		offset: number;
	}

	let transform = $state<CollectionTransform | null>(null);
	let collection = $state<Collection | null>(null);
	let dataset = $state<Dataset | null>(null);
	let stats = $state<CollectionTransformStats | null>(null);
	let processedFiles = $state<ProcessedFile[]>([]);
	let totalFilesCount = $state(0);
	let collectionFileCount = $state<number | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let processingDismissed = $state(false);
	let triggering = $state(false);
	let retrying = $state(false);
	let showRetryConfirm = $state(false);

	// Pagination for processed files
	let filesCurrentPage = $state(1);
	let filesPageSize = $state(10);

	// Sort state
	let sortBy = $state('processed_at');
	let sortDirection = $state<'asc' | 'desc'>('desc');

	// Polling interval for auto-refresh
	let pollTimer: ReturnType<typeof setInterval> | null = null;
	let isPolling = false;
	const POLL_INTERVAL_MS = 5000;

	// Derived: progress panel
	let progressStatus = $derived.by<
		'idle' | 'processing' | 'completed' | 'completed_with_errors' | 'failed'
	>(() => {
		if (!stats || collectionFileCount === null) return 'idle';
		if (stats.failed_files > 0 && stats.total_files_processed >= collectionFileCount)
			return 'completed_with_errors';
		if (stats.total_files_processed >= collectionFileCount && collectionFileCount > 0)
			return 'completed';
		if (stats.total_files_processed > 0 || collectionFileCount > 0) return 'processing';
		return 'idle';
	});

	let showProgressPanel = $derived(
		!processingDismissed &&
			stats !== null &&
			collectionFileCount !== null &&
			collectionFileCount > 0
	);

	// Stats grid data
	let statsGridItems = $derived.by(() => {
		if (!stats) return [];
		return [
			{
				label: 'Total Files',
				value: stats.total_files_processed,
				color: 'blue' as const,
				icon: '📄',
			},
			{ label: 'Successful', value: stats.successful_files, color: 'green' as const, icon: '✓' },
			{ label: 'Failed', value: stats.failed_files, color: 'red' as const, icon: '✗' },
			{
				label: 'Items Created',
				value: stats.total_items_created,
				color: 'purple' as const,
				icon: '📦',
			},
		];
	});

	// SortableTable column defs
	const fileColumns = [
		{ key: 'file_key', label: 'Item Name', sortable: true, renderer: 'text' as const },
		{ key: 'item_count', label: 'Total Chunks', sortable: true, renderer: 'number' as const },
		{ key: 'process_status', label: 'Status', sortable: true, renderer: 'status' as const },
		{
			key: 'processing_duration_ms',
			label: 'Duration',
			sortable: true,
			renderer: 'duration' as const,
		},
		{ key: 'processed_at', label: 'Processed At', sortable: true, renderer: 'date' as const },
	];

	// Transform info for the header component
	let transformInfo = $derived(
		transform
			? {
					id: transform.collection_transform_id,
					title: transform.title,
					is_enabled: transform.is_enabled,
					created_at: transform.created_at,
					updated_at: transform.updated_at,
				}
			: null
	);

	let resourceLinks = $derived.by(() => {
		const links: {
			label: string;
			title: string;
			navigatePage: string;
			navigateParams: Record<string, unknown>;
		}[] = [];
		if (collection && transform) {
			links.push({
				label: 'Collection',
				title: collection.title,
				navigatePage: 'collection-detail',
				navigateParams: { collectionId: transform.collection_id },
			});
		}
		if (dataset && transform) {
			links.push({
				label: 'Target Dataset',
				title: dataset.title,
				navigatePage: 'dataset-detail',
				navigateParams: { datasetId: transform.dataset_id },
			});
		}
		return links;
	});

	let extraFields = $derived(
		transform ? [{ label: 'Chunk Size', value: transform.chunk_size }] : []
	);

	async function fetchTransform() {
		try {
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}`, {
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to fetch collection transform: ${response.statusText}`);
			}

			transform = await response.json();

			if (transform?.collection_id) {
				await fetchCollection(transform.collection_id);
			}
			if (transform?.dataset_id) {
				await fetchDataset(transform.dataset_id);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Unknown error occurred';
			console.error('Error fetching collection transform:', e);
		}
	}

	async function fetchCollection(id: number) {
		try {
			const response = await fetch(`/api/collections/${id}`, {
				credentials: 'include',
			});

			if (response.ok) {
				collection = await response.json();
			}

			const filesResponse = await fetch(`/api/collections/${id}/files?page_size=1`, {
				credentials: 'include',
			});
			if (filesResponse.ok) {
				const filesData = await filesResponse.json();
				collectionFileCount = filesData.total_count ?? 0;
			}
		} catch (e) {
			console.error('Error fetching collection:', e);
		}
	}

	async function fetchDataset(id: number) {
		try {
			const response = await fetch(`/api/datasets/${id}`, {
				credentials: 'include',
			});

			if (response.ok) {
				dataset = await response.json();
			}
		} catch (e) {
			console.error('Error fetching dataset:', e);
		}
	}

	async function fetchStats() {
		try {
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}/stats`, {
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to fetch stats: ${response.statusText}`);
			}

			stats = await response.json();
		} catch (e) {
			console.error('Error fetching stats:', e);
		}
	}

	async function fetchProcessedFiles() {
		try {
			const offset = (filesCurrentPage - 1) * filesPageSize;
			const params = new URLSearchParams({
				limit: filesPageSize.toString(),
				offset: offset.toString(),
				sort_by: sortBy,
				sort_direction: sortDirection,
			});
			const response = await fetch(
				`/api/collection-transforms/${collectionTransformId}/processed-files?${params}`,
				{
					credentials: 'include',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to fetch processed files: ${response.statusText}`);
			}

			const data: PaginatedFilesResponse = await response.json();
			processedFiles = data.items ?? [];
			totalFilesCount = data.total_count ?? 0;
		} catch (e) {
			console.error('Error fetching processed files:', e);
		}
	}

	async function triggerTransform() {
		triggering = true;
		try {
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}/trigger`, {
				method: 'POST',
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger: ${response.statusText}`);
			}

			toastStore.success('Collection transform triggered');
			processingDismissed = false;
			await Promise.all([fetchStats(), fetchProcessedFiles()]);
		} catch (e) {
			toastStore.error(e instanceof Error ? e.message : 'Failed to trigger');
		} finally {
			triggering = false;
		}
	}

	async function retryFailed() {
		retrying = true;
		showRetryConfirm = false;
		try {
			const response = await fetch(
				`/api/collection-transforms/${collectionTransformId}/retry-failed`,
				{
					method: 'POST',
					credentials: 'include',
				}
			);
			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to retry: ${errorText}`);
			}
			const result = await response.json();
			if (result.retried_count > 0) {
				toastStore.success(
					`${result.retried_count} file(s) re-submitted for processing.`,
					'Retry Started'
				);
			} else {
				toastStore.info('No failed files to retry.', 'Nothing to Retry');
			}
			await Promise.all([fetchStats(), fetchProcessedFiles()]);
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			toastStore.error(msg, 'Retry Failed');
		} finally {
			retrying = false;
		}
	}

	function handlePageChange(page: number) {
		filesCurrentPage = page;
		fetchProcessedFiles();
	}

	function handleSort(field: string) {
		if (sortBy === field) {
			sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
		} else {
			sortBy = field;
			sortDirection = 'desc';
		}
		filesCurrentPage = 1;
		fetchProcessedFiles();
	}

	onMount(async () => {
		loading = true;
		await Promise.all([fetchTransform(), fetchStats(), fetchProcessedFiles()]);
		loading = false;

		pollTimer = setInterval(async () => {
			if (isPolling) return;
			isPolling = true;
			try {
				await Promise.all([fetchStats(), fetchProcessedFiles()]);
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
		title="Collection Transform Details"
		description="View detailed information, processing history, and statistics for this collection transform."
	/>

	<div class="mb-6">
		<button
			onclick={onBack}
			class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white transition-colors flex items-center gap-2"
		>
			← Back to Collection Transforms
		</button>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading collection transform details...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if transform && transformInfo}
		<!-- Processing Progress Panel -->
		{#if showProgressPanel}
			<TransformProgressPanel
				status={progressStatus}
				title={transform.title}
				subtitle={collection?.title ?? `Collection #${transform.collection_id}`}
				totalItems={collectionFileCount ?? 0}
				processedItems={stats?.total_files_processed ?? 0}
				failedItems={stats?.failed_files ?? 0}
				onDismiss={() => {
					processingDismissed = true;
				}}
			/>
		{/if}

		<!-- Transform Info Header -->
		<TransformDetailHeader
			transform={transformInfo}
			transformType="collection"
			apiBasePath="/api/collection-transforms"
			{resourceLinks}
			{extraFields}
			{onNavigate}
			onTransformUpdated={(updated) => {
				transform = updated;
			}}
			onDeleted={onBack}
			{triggering}
			onTrigger={triggerTransform}
		/>

		<!-- Job Configuration Card -->
		{#if transform.job_config && Object.keys(transform.job_config).length > 0}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<Heading tag="h3" class="text-lg font-bold mb-4">Job Configuration</Heading>
				<div class="space-y-4">
					{#each Object.entries(transform.job_config) as [key, value] (key)}
						<div>
							<h4 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-1">{key}</h4>
							{#if typeof value === 'object'}
								<pre
									class="text-sm font-mono bg-gray-50 dark:bg-gray-900 rounded-lg p-3 overflow-x-auto text-gray-900 dark:text-gray-100">{JSON.stringify(
										value,
										null,
										2
									)}</pre>
							{:else}
								<p class="text-sm font-medium text-gray-900 dark:text-white">{value}</p>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Stats -->
		{#if stats}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<div class="flex items-center justify-between mb-4">
					<Heading tag="h3" class="text-xl font-bold">Processing Statistics</Heading>
					<div class="flex items-center gap-2">
						{#if stats.failed_files > 0}
							<button
								onclick={() => (showRetryConfirm = true)}
								disabled={retrying}
								class="inline-flex items-center gap-1.5 px-3 py-1 text-sm font-medium rounded-full bg-orange-100 text-orange-700 hover:bg-orange-200 dark:bg-orange-900/20 dark:text-orange-400 dark:hover:bg-orange-900/40 disabled:opacity-50 transition-colors"
							>
								{#if retrying}
									<svg class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
										<circle
											class="opacity-25"
											cx="12"
											cy="12"
											r="10"
											stroke="currentColor"
											stroke-width="4"
										></circle>
										<path
											class="opacity-75"
											fill="currentColor"
											d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
										></path>
									</svg>
									Retrying…
								{:else}
									↻ Retry Failed ({stats.failed_files})
								{/if}
							</button>
						{/if}
					</div>
				</div>
				<StatsGrid stats={statsGridItems} columns={4} />
				{#if stats.last_run_at}
					<p class="text-xs text-gray-500 dark:text-gray-400 mt-4">
						Last run: {formatDate(stats.last_run_at)}
					</p>
				{/if}
			</div>
		{/if}

		<!-- Processed Files Table -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
			<Heading tag="h3" class="text-xl font-bold mb-4">Processed Files History</Heading>
			<SortableTable
				columns={fileColumns}
				rows={processedFiles}
				totalCount={totalFilesCount}
				currentPage={filesCurrentPage}
				pageSize={filesPageSize}
				{sortBy}
				{sortDirection}
				onPageChange={handlePageChange}
				onSort={handleSort}
				rowKey={(row) => row.id}
				getRowError={(row) => row.process_error}
				itemLabel="files"
				emptyMessage="No files have been processed yet."
			/>
		</div>
	{/if}
</div>

<ConfirmDialog
	open={showRetryConfirm}
	title="Retry Failed Files"
	message={`This will retry ${stats?.failed_files ?? 0} failed file(s). They will be re-submitted for processing.`}
	confirmLabel="Retry Failed"
	variant="warning"
	onConfirm={retryFailed}
	onCancel={() => (showRetryConfirm = false)}
/>
