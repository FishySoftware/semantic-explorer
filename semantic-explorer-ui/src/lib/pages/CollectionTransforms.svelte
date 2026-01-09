<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateCollectionTransformModal from '../components/CreateCollectionTransformModal.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

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

	let transforms = $state<CollectionTransform[]>([]);
	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Failed files modal state
	let showFailedFilesModal = $state(false);
	let failedFilesTransformTitle = $state('');
	let failedFiles = $state<ProcessedFile[]>([]);
	let loadingFailedFiles = $state(false);

	let searchQuery = $state('');

	// Modal state
	let showCreateModal = $state(false);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<CollectionTransform | null>(null);

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/collection-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch collection transforms: ${response.statusText}`);
			}
			transforms = await response.json();
			for (const transform of transforms) {
				fetchStatsForTransform(transform.collection_transform_id);
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch collection transforms');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function openFailedFilesModal(transform: CollectionTransform) {
		failedFilesTransformTitle = transform.title;
		showFailedFilesModal = true;
		loadingFailedFiles = true;
		failedFiles = [];

		try {
			const response = await fetch(
				`/api/collection-transforms/${transform.collection_transform_id}/processed-files`
			);
			if (response.ok) {
				const allFiles: ProcessedFile[] = await response.json();
				// Filter to only failed files
				failedFiles = allFiles.filter((f) => f.process_status === 'failed');
			}
		} catch (e) {
			console.error(
				`Failed to fetch processed files for transform ${transform.collection_transform_id}:`,
				e
			);
			toastStore.error('Failed to fetch failed files');
		} finally {
			loadingFailedFiles = false;
		}
	}

	function closeFailedFilesModal() {
		showFailedFilesModal = false;
		failedFilesTransformTitle = '';
		failedFiles = [];
	}

	async function fetchCollections() {
		try {
			const response = await fetch('/api/collections');
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			collections = await response.json();
		} catch (e) {
			console.error('Failed to fetch collections:', e);
		}
	}

	async function fetchDatasets() {
		try {
			const response = await fetch('/api/datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch datasets: ${response.statusText}`);
			}
			datasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch datasets:', e);
		}
	}

	async function toggleEnabled(transform: CollectionTransform) {
		try {
			const response = await fetch(
				`/api/collection-transforms/${transform.collection_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: !transform.is_enabled,
					}),
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const updated = await response.json();
			transforms = transforms.map((t) =>
				t.collection_transform_id === updated.collection_transform_id ? updated : t
			);

			toastStore.success(
				`Collection transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle collection transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({ collection_transform_id: transformId }),
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger transform: ${response.statusText}`);
			}

			toastStore.success('Collection transform triggered successfully');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger collection transform'));
		}
	}

	function requestDeleteTransform(transform: CollectionTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.collection_transform_id;
			const response = await fetch(`/api/collection-transforms/${target.collection_transform_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete collection transform: ${response.statusText}`);
			}

			transforms = transforms.filter(
				(t) => t.collection_transform_id !== target.collection_transform_id
			);
			toastStore.success('Collection transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete collection transform'));
		} finally {
			deleting = null;
		}
	}

	onMount(async () => {
		await Promise.all([fetchTransforms(), fetchCollections(), fetchDatasets()]);
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);

			const action = urlParams.get('action');
			const collectionIdParam = urlParams.get('collection_id');

			if (action === 'create' && collectionIdParam) {
				const collectionId = parseInt(collectionIdParam, 10);
				if (!isNaN(collectionId)) {
					showCreateModal = true;
				}
			}

			const basePath = hashParts[0];
			window.history.replaceState(
				null,
				'',
				window.location.pathname + window.location.search + basePath
			);
		}
	});

	let filteredTransforms = $derived(
		transforms.filter((t) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return t.title.toLowerCase().includes(query) || t.owner.toLowerCase().includes(query);
		})
	);

	function getCollectionTitle(collectionId: number): string {
		const collection = collections.find((c) => c.collection_id === collectionId);
		return collection ? collection.title : `Collection ${collectionId}`;
	}

	function getDatasetTitle(datasetId: number): string {
		const dataset = datasets.find((d) => d.dataset_id === datasetId);
		return dataset ? dataset.title : `Dataset ${datasetId}`;
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Collection Transforms"
		description="Process files from Collections into Dataset items. Collection transforms extract text from files, chunk them into manageable pieces, and create Dataset items ready for embedding."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Collection Transforms</h1>
		<button
			onclick={() => (showCreateModal = true)}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			Create Collection Transform
		</button>
	</div>

	<CreateCollectionTransformModal
		bind:open={showCreateModal}
		onSuccess={() => {
			fetchTransforms();
		}}
	/>

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
			placeholder="Search collection transforms..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading collection transforms...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if filteredTransforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No collection transforms found matching your search.'
					: 'No collection transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredTransforms as transform (transform.collection_transform_id)}
				{@const stats = statsMap.get(transform.collection_transform_id)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h3 class="text-xl font-semibold text-gray-900 dark:text-white">
									{transform.title}
								</h3>
								<span class="text-sm text-gray-500 dark:text-gray-400">
									#{transform.collection_transform_id}
								</span>
							</div>
							<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
								<p>
									<strong>Collection:</strong>
									<a
										href="#/collections/{transform.collection_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{getCollectionTitle(transform.collection_id)}
									</a>
								</p>
								<p>
									<strong>Dataset:</strong>
									<a
										href="#/datasets/{transform.dataset_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{getDatasetTitle(transform.dataset_id)}
									</a>
								</p>
								<p><strong>Chunk Size:</strong> {transform.chunk_size}</p>
								<p><strong>Owner:</strong> {transform.owner}</p>
								<p>
									<strong>Status:</strong>
									<span
										class={transform.is_enabled
											? 'text-green-600 dark:text-green-400'
											: 'text-gray-500 dark:text-gray-400'}
									>
										{transform.is_enabled ? 'Enabled' : 'Disabled'}
									</span>
								</p>
								<p><strong>Created:</strong> {new Date(transform.created_at).toLocaleString()}</p>
								<p><strong>Updated:</strong> {new Date(transform.updated_at).toLocaleString()}</p>
							</div>
						</div>
						<div class="flex flex-col gap-2">
							<button
								onclick={() => toggleEnabled(transform)}
								class="px-3 py-1 text-sm rounded-lg {transform.is_enabled
									? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400'
									: 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400'}"
							>
								{transform.is_enabled ? 'Disable' : 'Enable'}
							</button>
							<button
								onclick={() => triggerTransform(transform.collection_transform_id)}
								class="px-3 py-1 text-sm bg-blue-100 text-blue-700 hover:bg-blue-200 rounded-lg dark:bg-blue-900/20 dark:text-blue-400"
							>
								Trigger
							</button>
							<button
								onclick={() => requestDeleteTransform(transform)}
								disabled={deleting === transform.collection_transform_id}
								class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{deleting === transform.collection_transform_id ? 'Deleting...' : 'Delete'}
							</button>
						</div>
					</div>

					{#if stats}
						<div
							class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-4 gap-4"
						>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Files Processed</p>
								<p class="text-lg font-semibold text-gray-900 dark:text-white">
									{stats.total_files_processed}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Successful</p>
								<p class="text-lg font-semibold text-green-600 dark:text-green-400">
									{stats.successful_files}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Failed</p>
								{#if stats.failed_files > 0}
									<button
										onclick={() => openFailedFilesModal(transform)}
										class="text-lg font-semibold text-red-600 dark:text-red-400 hover:underline cursor-pointer"
										title="Click to view failed files"
									>
										{stats.failed_files}
									</button>
								{:else}
									<p class="text-lg font-semibold text-green-600 dark:text-green-400">0</p>
								{/if}
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Items Created</p>
								<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
									{stats.total_items_created}
								</p>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>

<!-- Failed Files Modal -->
{#if showFailedFilesModal}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
		<div
			class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-4xl w-full mx-4 max-h-[80vh] flex flex-col"
		>
			<div
				class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center"
			>
				<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
					Failed Files - {failedFilesTransformTitle}
				</h3>
				<button
					onclick={closeFailedFilesModal}
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

			<div class="p-6 overflow-y-auto flex-1">
				{#if loadingFailedFiles}
					<div class="flex justify-center py-8">
						<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
					</div>
				{:else if failedFiles.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-center py-8">No failed files found.</p>
				{:else}
					<div class="space-y-4">
						{#each failedFiles as file (file.file_key)}
							<div
								class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
							>
								<div class="flex items-start justify-between">
									<div class="flex-1 min-w-0">
										<p
											class="font-mono text-sm text-gray-900 dark:text-white truncate"
											title={file.file_key}
										>
											{file.file_key.split('/').pop() || file.file_key}
										</p>
										<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
											Processed: {new Date(file.processed_at).toLocaleString()}
										</p>
									</div>
								</div>
								{#if file.process_error}
									<div class="mt-3 bg-red-100 dark:bg-red-900/40 rounded p-3">
										<p class="text-xs font-semibold text-red-700 dark:text-red-300 mb-1">Error:</p>
										<pre
											class="text-xs text-red-600 dark:text-red-400 whitespace-pre-wrap wrap-break-words font-mono">{file.process_error}</pre>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
				<button
					onclick={closeFailedFilesModal}
					class="px-4 py-2 bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
				>
					Close
				</button>
			</div>
		</div>
	</div>
{/if}
