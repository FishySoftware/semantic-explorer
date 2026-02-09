<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

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

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete state
	let transformPendingDelete = $state<CollectionTransform | null>(null);
	let deleting = $state(false);

	// Pagination for processed files
	let filesCurrentPage = $state(1);
	let filesPageSize = $state(10);

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let isMounted = false; // Track if component is still mounted

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
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}`, {
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
			toastStore.success('Collection transform updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update collection transform');
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function toggleEnabled() {
		if (!transform) return;

		try {
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}`, {
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
				`Collection transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle collection transform'));
		}
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) return;

		transformPendingDelete = null;

		try {
			deleting = true;
			const response = await fetch(`/api/collection-transforms/${collectionTransformId}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete transform: ${response.statusText}`);
			}

			toastStore.success('Collection transform deleted');
			onBack();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete collection transform'));
		} finally {
			deleting = false;
		}
	}

	function connectSSE() {
		// Close existing connection first
		disconnectSSE();

		try {
			eventSource = new EventSource('/api/collection-transforms/stream');

			eventSource.addEventListener('heartbeat', () => {
				// Keep connection alive
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// If this is an update for our transform, refresh stats and files
					// API sends transform_id (generic) not collection_transform_id
					if (statusUpdate.transform_id === collectionTransformId) {
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
		if (!isMounted) {
			// Component has been unmounted, don't attempt reconnection
			return;
		}

		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error('Max SSE reconnection attempts reached');
			return;
		}

		const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 60000);
		reconnectAttempts++;

		reconnectTimer = setTimeout(() => {
			if (isMounted) {
				connectSSE();
			}
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
		isMounted = true;
		loading = true;
		await Promise.all([fetchTransform(), fetchStats(), fetchProcessedFiles()]);
		loading = false;
		connectSSE();
	});

	onDestroy(() => {
		isMounted = false;
		disconnectSSE();
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
	{:else if transform}
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
								>#{transform.collection_transform_id}</span
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
			</div>
		</div>

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
						Last run: {formatDate(stats.last_run_at)}
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
									<td class="px-4 py-3">{formatDate(file.processed_at)}</td>
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

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete Collection Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteTransform}
	onCancel={() => (transformPendingDelete = null)}
/>
