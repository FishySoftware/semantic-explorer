<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import PageHeader from '../components/PageHeader.svelte';

	interface Props {
		collectionTransformId: number;
		onBack: () => void;
		onNavigate?: (_page: string, _params?: Record<string, unknown>) => void;
	}

	let { collectionTransformId, onBack, onNavigate }: Props = $props();

	interface CollectionTransform {
		collection_transform_id: number;
		title: string;
		collection_id: number;
		dataset_id: number;
		owner: string;
		is_enabled: boolean;
		chunk_size: number;
		job_config: any;
		created_at: string;
		updated_at: string;
	}

	interface Collection {
		collection_id: number;
		title: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface Stats {
		collection_transform_id: number;
		total_files_processed: number;
		successful_files: number;
		failed_files: number;
		total_items_created: number;
		last_run_at: string | null;
	}

	interface ProcessedFile {
		id: number;
		transform_type: string;
		transform_id: number;
		file_key: string;
		processed_at: string;
		item_count: number;
		process_status: string;
		process_error: string | null;
		processing_duration_ms: number | null;
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
	let stats = $state<Stats | null>(null);
	let processedFiles = $state<ProcessedFile[]>([]);
	let totalFilesCount = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Pagination for processed files
	let filesCurrentPage = $state(1);
	let filesPageSize = $state(10);

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

	async function fetchTransform() {
		try {
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}`, {
				credentials: 'include',
			});

			if (!response.ok) {
				throw new Error(`Failed to fetch collection transform: ${response.statusText}`);
			}

			transform = await response.json();

			// Fetch related resources
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
			const response = await fetch(
				`/api/collection-transforms/${collectionTransformId}/processed-files?limit=${filesPageSize}&offset=${offset}`,
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

	function getFilesTotalPages(): number {
		if (totalFilesCount <= 0 || filesPageSize <= 0) return 1;
		return Math.ceil(totalFilesCount / filesPageSize);
	}

	function handleFilesPageChange(page: number) {
		if (page < 1 || page > getFilesTotalPages()) return;
		filesCurrentPage = page;
		fetchProcessedFiles();
	}

	function connectSSE() {
		try {
			const user = 'default-user'; // This should come from auth context
			eventSource = new EventSource(
				`/api/collection-transforms/stream?owner=${encodeURIComponent(user)}`,
				{ withCredentials: true }
			);

			eventSource.addEventListener('heartbeat', () => {
				// Keep connection alive
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// If this is an update for our transform, refresh stats and files
					if (statusUpdate.collection_transform_id === collectionTransformId) {
						fetchStats();
						fetchProcessedFiles();
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
		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error('Max SSE reconnection attempts reached');
			return;
		}

		const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 60000);
		reconnectAttempts++;

		reconnectTimer = setTimeout(() => {
			connectSSE();
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
		loading = true;
		await Promise.all([fetchTransform(), fetchStats(), fetchProcessedFiles()]);
		loading = false;
		connectSSE();
	});

	onDestroy(() => {
		disconnectSSE();
	});
</script>

<div class="max-w-7xl mx-auto">
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
	{:else if transform}
		<!-- Transform Info Card -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<div class="flex justify-between items-start mb-4">
				<div>
					<Heading tag="h2" class="text-2xl font-bold mb-2">{transform.title}</Heading>
					<p class="text-sm text-gray-500 dark:text-gray-400">
						Created {new Date(transform.created_at).toLocaleString()}
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
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Collection</p>
					{#if collection}
						<button
							onclick={() =>
								onNavigate?.('collection-detail', { collectionId: transform?.collection_id })}
							class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
						>
							{collection.title}
						</button>
					{:else}
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							ID: {transform.collection_id}
						</p>
					{/if}
				</div>
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Target Dataset</p>
					{#if dataset}
						<button
							onclick={() => onNavigate?.('dataset-detail', { datasetId: transform?.dataset_id })}
							class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
						>
							{dataset.title}
						</button>
					{:else}
						<p class="text-lg font-medium text-gray-900 dark:text-white">
							ID: {transform.dataset_id}
						</p>
					{/if}
				</div>
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Chunk Size</p>
					<p class="text-lg font-medium text-gray-900 dark:text-white">{transform.chunk_size}</p>
				</div>
				<div>
					<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">Owner</p>
					<p class="text-lg font-medium text-gray-900 dark:text-white">{transform.owner}</p>
				</div>
			</div>
		</div>

		<!-- Stats Card -->
		{#if stats}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
				<Heading tag="h3" class="text-xl font-bold mb-4">Processing Statistics</Heading>
				<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
					<div class="text-center">
						<p class="text-3xl font-bold text-blue-600 dark:text-blue-400">
							{stats.total_files_processed}
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Total Files</p>
					</div>
					<div class="text-center">
						<p class="text-3xl font-bold text-green-600 dark:text-green-400">
							{stats.successful_files}
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Successful</p>
					</div>
					<div class="text-center">
						<p class="text-3xl font-bold text-red-600 dark:text-red-400">{stats.failed_files}</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Failed</p>
					</div>
					<div class="text-center">
						<p class="text-3xl font-bold text-purple-600 dark:text-purple-400">
							{stats.total_items_created}
						</p>
						<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Items Created</p>
					</div>
				</div>
				{#if stats.last_run_at}
					<p class="text-xs text-gray-500 dark:text-gray-400 mt-4">
						Last run: {new Date(stats.last_run_at).toLocaleString()}
					</p>
				{/if}
			</div>
		{/if}

		<!-- Processed Files Table -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
			<Heading tag="h3" class="text-xl font-bold mb-4">Processed Files History</Heading>

			{#if processedFiles.length === 0}
				<p class="text-center text-gray-500 dark:text-gray-400 py-8">
					No files have been processed yet.
				</p>
			{:else}
				<div class="overflow-x-auto">
					<table class="w-full text-sm text-left text-gray-600 dark:text-gray-400">
						<thead
							class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700"
						>
							<tr>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Item Name</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Total Chunks</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Status</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Duration</th>
								<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Processed At</th>
							</tr>
						</thead>
						<tbody>
							{#each processedFiles as file (file.id)}
								<tr
									class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
								>
									<td class="px-4 py-3 font-medium text-gray-900 dark:text-white">
										{file.file_key}
									</td>
									<td class="px-4 py-3">
										<span
											class="inline-flex items-center px-2.5 py-0.5 rounded-full text-sm font-semibold bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300"
										>
											{file.item_count}
										</span>
									</td>
									<td class="px-4 py-3">
										<span
											class={file.process_status === 'success'
												? 'px-2 py-1 rounded-full text-xs font-semibold bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
												: file.process_status === 'completed'
													? 'px-2 py-1 rounded-full text-xs font-semibold bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
													: file.process_status === 'failed'
														? 'px-2 py-1 rounded-full text-xs font-semibold bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400'
														: 'px-2 py-1 rounded-full text-xs font-semibold bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'}
										>
											{file.process_status}
										</span>
									</td>
									<td class="px-4 py-3">
										{file.processing_duration_ms ? `${file.processing_duration_ms}ms` : '-'}
									</td>
									<td class="px-4 py-3">{new Date(file.processed_at).toLocaleString()}</td>
								</tr>
								{#if file.process_error}
									<tr
										class="bg-red-50 dark:bg-red-900/10 border-b border-gray-200 dark:border-gray-700"
									>
										<td colspan="5" class="px-4 py-2 text-xs text-red-600 dark:text-red-400">
											Error: {file.process_error}
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
						Showing {(filesCurrentPage - 1) * filesPageSize + 1} to {Math.min(
							filesCurrentPage * filesPageSize,
							totalFilesCount
						)} of {totalFilesCount} files
					</div>
					<div class="flex gap-2">
						<button
							onclick={() => handleFilesPageChange(filesCurrentPage - 1)}
							disabled={filesCurrentPage === 1}
							class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
						>
							Previous
						</button>
						<div class="flex items-center gap-1">
							{#each Array.from({ length: getFilesTotalPages() }, (_, i) => i + 1) as page (page)}
								{#if page === 1 || page === getFilesTotalPages() || (page >= filesCurrentPage - 1 && page <= filesCurrentPage + 1)}
									<button
										onclick={() => handleFilesPageChange(page)}
										class={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
											filesCurrentPage === page
												? 'bg-blue-600 text-white'
												: 'border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
										}`}
									>
										{page}
									</button>
								{:else if page === filesCurrentPage - 2 || page === filesCurrentPage + 2}
									<span class="px-2 py-2 text-gray-500">...</span>
								{/if}
							{/each}
						</div>
						<button
							onclick={() => handleFilesPageChange(filesCurrentPage + 1)}
							disabled={filesCurrentPage === getFilesTotalPages()}
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
