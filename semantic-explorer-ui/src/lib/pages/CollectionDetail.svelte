<script lang="ts">
	import { ArrowLeftOutline, ExpandOutline, UploadOutline } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import ApiIntegrationModal from '../components/ApiIntegrationModal.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateCollectionTransformModal from '../components/CreateCollectionTransformModal.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import TransformsList from '../components/TransformsList.svelte';
	import UploadProgressPanel from '../components/UploadProgressPanel.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { createSSEConnection, type SSEConnection } from '../utils/sse';
	import { formatDate, formatFileSize } from '../utils/ui-helpers';

	interface FileStatus {
		name: string;
		status: 'pending' | 'uploading' | 'completed' | 'failed';
		progress: number;
		error?: string;
	}

	interface CollectionFile {
		key: string;
		size: number;
		last_modified: string | null;
		content_type: string | null;
	}

	interface PaginatedFiles {
		files: CollectionFile[];
		page: number;
		page_size: number;
		has_more: boolean;
		continuation_token: string | null;
		total_count: number | null;
	}

	interface PaginatedResponse<T> {
		items: T[];
		total_count: number;
		limit: number;
		offset: number;
	}

	interface Collection {
		collection_id: number;
		title: string;
		details: string | null;
		owner: string;
		bucket: string;
		tags: string[];
		is_public: boolean;
	}

	interface CollectionTransform {
		collection_transform_id: number;
		title: string;
		collection_id: number;
		dataset_id: number;
		owner: string;
		is_enabled: boolean;
		chunk_size: number;
		job_config: Record<string, any>;
		created_at: string;
		updated_at: string;
		last_run_at?: string;
		last_run_status?: string;
		last_run_stats?: Record<string, any>;
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

	interface FailedFileWithTransform extends ProcessedFile {
		transform_title: string;
	}

	interface Props {
		collectionId: number;
		onBack: () => void;
	}

	let { collectionId, onBack }: Props = $props();

	let collection = $state<Collection | null>(null);
	let paginatedFiles = $state<PaginatedFiles | null>(null);
	let pageLoading = $state(true);
	let filesLoading = $state(true);
	let error = $state<string | null>(null);
	let currentPage = $state(0);
	let pageSize = $state(10);
	let deletingFile = $state<string | null>(null);
	let filePendingDelete = $state<CollectionFile | null>(null);
	let collectionPendingDelete = $state(false);

	let collectionTransforms = $state<CollectionTransform[]>([]);
	let transformsLoading = $state(false);
	let collectionTransformStatsMap = $state<Map<number, any>>(new Map());

	let transformModalOpen = $state(false);
	let activeTab = $state('files');
	let apiIntegrationModalOpen = $state(false);

	// Failed files state (transform processing failures from API)
	let failedFiles = $state<FailedFileWithTransform[]>([]);
	let failedFilesLoading = $state(false);
	let failedFilesTotalCount = $state(0);
	let failedFilesPage = $state(0);
	let failedFilesPageSize = $state(25);

	// Upload failures (files that failed during upload, not stored server-side)
	let uploadFailures = $state<{ name: string; error: string; timestamp: string }[]>([]);

	let failedFilesCount = $derived(failedFilesTotalCount + uploadFailures.length);

	const tabs = $derived([
		{ id: 'files', label: 'Files', icon: '📁' },
		{ id: 'transforms', label: 'Transforms', icon: '🔄' },
		{
			id: 'failed',
			label: `Failed Files${failedFilesCount > 0 ? ` (${failedFilesCount})` : ''}`,
			icon: '⚠️',
		},
	]);

	let paginationHistory = $state<(string | null)[]>([null]);
	let currentPageIndex = $state(0);
	let totalCount = $state<number | null>(null);

	function getInitialSearchQuery(): string {
		if (typeof window === 'undefined') return '';
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const params = new SvelteURLSearchParams(hashParts[1]);
			const searchParam = params.get('search');
			if (searchParam) {
				// Remove the search param from the URL
				params.delete('search');
				const newQueryString = params.toString();
				const hashBase = hashParts[0];
				const newHash = newQueryString ? `${hashBase}?${newQueryString}` : hashBase;
				window.history.replaceState(null, '', newHash);
				return decodeURIComponent(searchParam);
			}
		}
		return '';
	}

	let searchQuery = $state(getInitialSearchQuery());
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;

	let uploading = $state(false);
	let uploadProgress = $state<{
		completed: number;
		total: number;
		currentBatch: number;
		totalBatches: number;
	} | null>(null);
	let fileStatuses = $state<FileStatus[]>([]);
	let fileInputRef = $state<HTMLInputElement | undefined>();
	let updatingPublic = $state(false);
	let allowedFileTypes = $state<string>(''); // MIME types for file input accept attribute

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let editDetails = $state('');
	let editTags = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// SSE connection for real-time transform status updates
	let sseConnection: SSEConnection | null = null;

	onMount(() => {
		// Load initial data
		Promise.all([
			fetchCollection(),
			fetchFiles(),
			fetchCollectionTransforms(),
			fetchFailedFiles(),
			fetchAllowedFileTypes(),
		]);
		connectSSE();

		// Warn user if they try to leave while uploading
		const handleBeforeUnload = (event: Event) => {
			if (uploading) {
				event.preventDefault();
			}
		};

		window.addEventListener('beforeunload', handleBeforeUnload);

		return () => {
			window.removeEventListener('beforeunload', handleBeforeUnload);
		};
	});

	onDestroy(() => {
		sseConnection?.disconnect();
		if (searchTimeout) {
			clearTimeout(searchTimeout);
		}
	});

	function connectSSE() {
		sseConnection = createSSEConnection({
			url: `/api/collection-transforms/stream?collection_id=${collectionId}`,
			onStatus: (data: unknown) => {
				const status = data as { collection_transform_id?: number };
				if (status.collection_transform_id) {
					// Refresh stats for this transform
					fetchCollectionTransformStats(status.collection_transform_id);
					// Refresh failed files in case the status changed
					fetchFailedFiles();
				}
			},
			onMaxRetriesReached: () => {
				console.warn('SSE connection lost for collection transforms');
			},
		});
	}

	// Cleanup polling on unmount is handled by controller.stop()

	async function fetchCollectionTransforms() {
		try {
			transformsLoading = true;
			const response = await fetch(`/api/collections/${collectionId}/transforms`);
			if (response.ok) {
				const transforms: CollectionTransform[] = await response.json();
				collectionTransforms = transforms.sort(
					(a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
				);

				// Fetch stats for each collection transform
				for (const transform of collectionTransforms) {
					fetchCollectionTransformStats(transform.collection_transform_id);
				}
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to load transforms'));
		} finally {
			transformsLoading = false;
		}
	}

	async function fetchCollectionTransformStats(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				collectionTransformStatsMap.set(transformId, stats);
				collectionTransformStatsMap = collectionTransformStatsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(e);
		}
	}

	async function fetchFailedFiles() {
		try {
			failedFilesLoading = true;
			const offset = failedFilesPage * failedFilesPageSize;
			const response = await fetch(
				`/api/collections/${collectionId}/failed-files?limit=${failedFilesPageSize}&offset=${offset}`
			);
			if (response.ok) {
				const data = (await response.json()) as PaginatedResponse<FailedFileWithTransform>;
				failedFiles = data.items;
				failedFilesTotalCount = data.total_count;
			}
		} catch (e) {
			console.error('Failed to fetch failed files:', e);
		} finally {
			failedFilesLoading = false;
		}
	}

	async function fetchAllowedFileTypes() {
		try {
			const response = await fetch('/api/collections-allowed-file-types');
			if (response.ok) {
				const mimeTypes: string[] = await response.json();
				// Convert MIME types to file input accept format
				// Use MIME types directly as they're more reliable than extensions
				allowedFileTypes = mimeTypes.join(',');
			}
		} catch (e) {
			console.error('Failed to fetch allowed file types:', e);
			// Don't block upload if this fails, just skip the restriction
			allowedFileTypes = '';
		}
	}

	async function fetchCollection() {
		try {
			pageLoading = true;
			const response = await fetch('/api/collections');
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			const data = await response.json();
			const collections: Collection[] = data.collections ?? [];
			collection = collections.find((c) => c.collection_id === collectionId) || null;
			if (!collection) {
				throw new Error('Collection not found');
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch collection');
			error = message;
			toastStore.error(message);
		} finally {
			pageLoading = false;
		}
	}

	async function fetchFiles() {
		try {
			filesLoading = true;
			error = null;
			const continuationToken = paginationHistory[currentPageIndex];
			const params = new SvelteURLSearchParams({ page_size: pageSize.toString() });
			if (continuationToken) {
				params.append('continuation_token', continuationToken);
			}
			const response = await fetch(`/api/collections/${collectionId}/files?${params.toString()}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch files: ${response.statusText}`);
			}
			paginatedFiles = await response.json();

			if (
				paginatedFiles &&
				paginatedFiles.total_count !== null &&
				paginatedFiles.total_count !== undefined
			) {
				totalCount = paginatedFiles.total_count;
			}
			if (paginatedFiles && paginatedFiles.total_count === null && totalCount !== null) {
				paginatedFiles.total_count = totalCount;
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch files');
			error = message;
			toastStore.error(message);
		} finally {
			filesLoading = false;
		}
	}

	async function downloadFile(file: CollectionFile) {
		try {
			const url = `/api/collections/${collectionId}/files/${encodeURIComponent(file.key)}`;
			const a = document.createElement('a');
			a.href = url;
			a.download = file.key;
			document.body.appendChild(a);
			a.click();
			document.body.removeChild(a);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to download file'));
		}
	}

	function requestDeleteFile(file: CollectionFile) {
		filePendingDelete = file;
	}

	function cancelDeleteFile() {
		filePendingDelete = null;
	}

	async function confirmDeleteFile() {
		if (!filePendingDelete) {
			return;
		}

		const target = filePendingDelete;
		filePendingDelete = null;

		try {
			deletingFile = target.key;
			const response = await fetch(
				`/api/collections/${collectionId}/files/${encodeURIComponent(target.key)}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to delete file: ${response.statusText}`);
			}

			if (totalCount !== null) {
				totalCount = totalCount - 1;
			}

			await fetchFiles();
			toastStore.success('File deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete file'));
		} finally {
			deletingFile = null;
		}
	}

	async function confirmDeleteCollection() {
		if (!collection) return;

		collectionPendingDelete = false;

		try {
			const response = await fetch(`/api/collections/${collection.collection_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to delete collection: ${errorText}`);
			}

			toastStore.success('Collection deleted successfully');
			onBack();
		} catch (e) {
			const message = formatError(e, 'Failed to delete collection');
			toastStore.error(message);
		}
	}

	function goToNextPage() {
		if (!paginatedFiles?.has_more) return;

		if (paginatedFiles.continuation_token) {
			currentPageIndex++;
			if (currentPageIndex >= paginationHistory.length) {
				paginationHistory = [...paginationHistory, paginatedFiles.continuation_token];
			}
			currentPage++;
			fetchFiles();
		}
	}

	function goToPreviousPage() {
		if (currentPageIndex === 0) return;

		currentPageIndex--;
		currentPage--;
		fetchFiles();
	}

	let filteredFiles = $derived(
		(paginatedFiles?.files || []).filter((file) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				file.key.toLowerCase().includes(query) || file.content_type?.toLowerCase().includes(query)
			);
		})
	);

	// When there's a search filter, only show Next button if there are more filtered results than current page can show
	let canGoToNextPage = $derived.by(() => {
		if (searchQuery.trim()) {
			// If filtering, only allow next if we have more filtered items than current page size
			return filteredFiles.length > (currentPage + 1) * pageSize;
		}
		// Otherwise use server-side pagination indicator
		return paginatedFiles?.has_more ?? false;
	});

	function changePageSize(newSize: number) {
		pageSize = newSize;
		currentPage = 0;
		currentPageIndex = 0;
		paginationHistory = [null];
		totalCount = null;
		fetchFiles();
	}

	async function togglePublic() {
		if (!collection) return;

		try {
			updatingPublic = true;
			const response = await fetch(`/api/collections/${collectionId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: collection.title,
					details: collection.details,
					tags: collection.tags,
					is_public: !collection.is_public,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to update collection: ${response.statusText}`);
			}

			const updatedCollection = await response.json();
			collection = updatedCollection;
			toastStore.success(
				updatedCollection.is_public ? 'Collection is now public' : 'Collection is now private'
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to update collection visibility'));
		} finally {
			updatingPublic = false;
		}
	}

	function startEdit() {
		if (!collection) return;
		editMode = true;
		editTitle = collection.title;
		editDetails = collection.details || '';
		editTags = collection.tags.join(', ');
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editTitle = '';
		editDetails = '';
		editTags = '';
		editError = null;
	}

	async function saveEdit() {
		if (!collection) return;

		if (!editTitle.trim()) {
			editError = 'Title is required';
			return;
		}

		const tags = editTags
			.split(',')
			.map((tag) => tag.trim())
			.filter((tag) => tag.length > 0);

		try {
			saving = true;
			editError = null;
			const response = await fetch(`/api/collections/${collectionId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: editTitle.trim(),
					details: editDetails.trim() || null,
					tags,
					is_public: collection.is_public,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to update collection: ${response.statusText}`);
			}

			const updatedCollection = await response.json();
			collection = updatedCollection;
			editMode = false;
			toastStore.success('Collection updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update collection');
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function uploadFiles(files: FileList | null) {
		if (!files || files.length === 0) {
			return;
		}

		if (!collectionId || isNaN(collectionId)) {
			toastStore.error('Invalid collection ID');
			return;
		}

		const batchSize = 25; // Upload 25 files at a time
		uploading = true;
		uploadProgress = {
			completed: 0,
			total: files.length,
			currentBatch: 1,
			totalBatches: Math.ceil(files.length / batchSize),
		};

		// Initialize file statuses
		fileStatuses = Array.from(files).map((file) => ({
			name: file.name,
			status: 'pending',
			progress: 0,
		}));

		const fileArray = Array.from(files);
		const completedFiles: string[] = [];
		const uploadFailedFiles: { name: string; error: string }[] = [];

		try {
			// Upload files in batches of 25 to balance performance and real-time feedback
			for (let i = 0; i < fileArray.length; i += batchSize) {
				const currentBatch = Math.floor(i / batchSize) + 1;
				const batch = fileArray.slice(i, Math.min(i + batchSize, fileArray.length));

				// Upload all files in this batch in parallel
				const uploadPromises = batch.map(async (file) => {
					const fileIndex = fileStatuses.findIndex((f) => f.name === file.name);

					if (fileIndex >= 0) {
						fileStatuses[fileIndex].status = 'uploading';
						fileStatuses[fileIndex].progress = 0;
						fileStatuses = fileStatuses; // Trigger reactivity
					}

					const formData = new FormData();
					formData.append('files', file);

					try {
						const response = await fetch(`/api/collections/${collectionId}/files`, {
							method: 'POST',
							body: formData,
						});

						if (!response.ok) {
							throw new Error(`HTTP ${response.status}`);
						}

						const result = await response.json();

						if (result.completed && result.completed.length > 0) {
							completedFiles.push(...result.completed);
							if (fileIndex >= 0) {
								fileStatuses[fileIndex].status = 'completed';
								fileStatuses[fileIndex].progress = 100;
							}
						}
						if (result.failed && result.failed.length > 0) {
							for (const f of result.failed) {
								uploadFailedFiles.push({ name: f.name, error: f.error });
							}
							if (fileIndex >= 0) {
								fileStatuses[fileIndex].status = 'failed';
								fileStatuses[fileIndex].error = result.failed[0]?.error || 'Upload failed';
							}
						}
					} catch (e) {
						uploadFailedFiles.push({ name: file.name, error: formatError(e, 'Upload error') });
						if (fileIndex >= 0) {
							fileStatuses[fileIndex].status = 'failed';
							fileStatuses[fileIndex].error = formatError(e, 'Upload error');
						}
					}

					// Update file statuses after each individual file upload
					fileStatuses = fileStatuses; // Trigger reactivity
				});

				// Wait for all files in this batch to complete
				await Promise.all(uploadPromises);

				// Update progress after batch completes
				uploadProgress = {
					completed: completedFiles.length + uploadFailedFiles.length,
					total: fileArray.length,
					currentBatch: currentBatch,
					totalBatches: Math.ceil(fileArray.length / batchSize),
				};

				fileStatuses = fileStatuses; // Trigger reactivity
			}

			// Update counts and first page only after all uploads complete
			if (completedFiles.length > 0) {
				// Update total count to reflect newly uploaded files
				if (totalCount !== null) {
					totalCount += completedFiles.length;
				}

				// Only refresh the first page if we're not on page 1, otherwise files are already there
				if (currentPageIndex === 0) {
					// Refresh the first page to update file list
					await fetchFiles();
				} else {
					// If user is on a different page, just update the total count
					if (paginatedFiles) {
						paginatedFiles.total_count = totalCount;
					}
				}

				toastStore.success(
					`Successfully uploaded ${completedFiles.length} file${completedFiles.length === 1 ? '' : 's'}`
				);
			}

			if (uploadFailedFiles.length > 0) {
				// Add upload failures to the persistent list
				const now = new Date().toISOString();
				uploadFailures = [
					...uploadFailedFiles.map((f) => ({
						name: f.name,
						error: f.error,
						timestamp: now,
					})),
					...uploadFailures,
				];
				// Switch to the Failed tab so the user sees the failures
				activeTab = 'failed';
				toastStore.warning(
					`Failed to upload ${uploadFailedFiles.length} file${uploadFailedFiles.length === 1 ? '' : 's'} — see Failed Files tab for details`
				);
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to upload files'));
		} finally {
			uploading = false;
			// Keep the status panel visible longer if there are failures so user can see errors
			const hasFailures = fileStatuses.some((f) => f.status === 'failed');
			if (!hasFailures) {
				setTimeout(() => {
					uploadProgress = null;
					fileStatuses = [];
				}, 2000);
			}
		}
	}

	function handleFileSelect(event: Event) {
		const input = event.target as HTMLInputElement;
		uploadFiles(input.files);
	}

	function openFileDialog() {
		fileInputRef?.click();
	}

	$effect(() => {
		if (!uploading && fileInputRef) {
			fileInputRef.value = '';
		}
	});
</script>

<div class=" mx-auto">
	<div class="mb-2">
		<button onclick={onBack} class="mb-2 btn-secondary inline-flex items-center gap-2">
			<ArrowLeftOutline class="w-5 h-5" />
			Back to Collections
		</button>

		{#if pageLoading}
			<LoadingState message="Loading collection..." />
		{:else if collection}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
				{#if editMode}
					<!-- Edit Mode -->
					<div class="mb-4">
						<div class="flex items-center justify-between mb-4">
							<h2 class="text-xl font-semibold text-gray-900 dark:text-white">Edit Collection</h2>
							<span class="text-sm text-gray-500 dark:text-gray-400">
								#{collection.collection_id}
							</span>
						</div>
						<form
							onsubmit={(e) => {
								e.preventDefault();
								saveEdit();
							}}
						>
							<div class="space-y-4">
								<div>
									<label
										for="edit-title"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Title
									</label>
									<input
										id="edit-title"
										type="text"
										bind:value={editTitle}
										placeholder="Enter collection title"
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
										required
									/>
								</div>
								<div>
									<label
										for="edit-details"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Details
									</label>
									<textarea
										id="edit-details"
										bind:value={editDetails}
										placeholder="Enter collection details (optional)"
										rows="3"
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
									></textarea>
								</div>
								<div>
									<label
										for="edit-tags"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Tags <span class="text-xs text-gray-500 dark:text-gray-400"
											>(comma-separated)</span
										>
									</label>
									<input
										id="edit-tags"
										type="text"
										bind:value={editTags}
										placeholder="e.g., documents, images, reports"
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
									/>
								</div>
								{#if editError}
									<div
										class="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400"
									>
										{editError}
									</div>
								{/if}
								<div class="flex gap-3">
									<button
										type="submit"
										disabled={saving}
										class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
									>
										{saving ? 'Saving...' : 'Save'}
									</button>
									<button
										type="button"
										onclick={cancelEdit}
										disabled={saving}
										class="btn-secondary disabled:opacity-50 disabled:cursor-not-allowed"
									>
										Cancel
									</button>
								</div>
							</div>
						</form>
					</div>
				{:else}
					<!-- View Mode -->
					<div class="flex justify-between items-start mb-2">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
									{collection.title}
								</h1>
								<span class="text-sm text-gray-500 dark:text-gray-400">
									#{collection.collection_id}
								</span>
							</div>
							{#if collection.details}
								<p class="text-gray-600 dark:text-gray-400 mb-3">
									{collection.details}
								</p>
							{/if}
							<div class="flex items-center gap-2 flex-wrap">
								<!-- <span
								class="inline-flex items-center gap-1.5 px-3 py-1 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-full text-sm"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
									></path>
								</svg>
								{collection.owner}
							</span> -->
								{#if collection.tags && collection.tags.length > 0}
									{#each collection.tags as tag (tag)}
										<span
											class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
										>
											#{tag}
										</span>
									{/each}
								{/if}
							</div>
							<div class="mt-3">
								<label class="inline-flex items-center gap-2 cursor-pointer">
									<input
										type="checkbox"
										checked={collection.is_public}
										onchange={togglePublic}
										disabled={updatingPublic}
										class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">
										{#if collection.is_public}
											<span class="font-semibold text-green-600 dark:text-green-400">Public</span> - visible
											in marketplace
										{:else}
											<span class="font-semibold text-gray-600 dark:text-gray-400">Private</span> - only
											visible to you
										{/if}
									</span>
								</label>
							</div>
						</div>
						<button
							type="button"
							onclick={() => (apiIntegrationModalOpen = true)}
							class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-lg transition-colors"
						>
							<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
								/>
							</svg>
							API
						</button>
						<button
							type="button"
							onclick={startEdit}
							class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
							title="Edit collection"
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
							onclick={() => (collectionPendingDelete = true)}
							class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
							title="Delete collection"
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
				{/if}
			</div>
		{/if}
	</div>

	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
		<TabPanel {tabs} activeTabId={activeTab} onChange={(tabId: string) => (activeTab = tabId)}>
			{#snippet children(tabId)}
				{#if tabId === 'files'}
					<div id="files-panel" role="tabpanel" class="animate-fadeIn">
						<div class="p-4 border-b border-gray-200 dark:border-gray-700">
							<div class="flex justify-between items-center">
								<div class="flex items-center gap-3">
									<h2 class="text-2xl font-bold text-gray-900 dark:text-white">Files</h2>
									{#if paginatedFiles?.total_count !== null && paginatedFiles?.total_count !== undefined}
										<span
											class="px-3 py-1 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-full text-sm font-medium"
										>
											{paginatedFiles.total_count} total
										</span>
									{/if}
								</div>
								<div class="flex items-center gap-4">
									<input
										type="file"
										multiple
										accept={allowedFileTypes}
										bind:this={fileInputRef}
										onchange={handleFileSelect}
										class="hidden"
									/>
									<button
										onclick={openFileDialog}
										disabled={uploading}
										class="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors inline-flex items-center gap-2"
									>
										{#if uploading}
											<span class="animate-spin">⏳</span>
											Uploading...
										{:else}
											<UploadOutline class="w-5 h-5" />
											Upload Files
										{/if}
									</button>
									{#if allowedFileTypes}
										<span
											class="text-xs text-gray-500 dark:text-gray-400"
											title="Supported: PDF, Word, Excel, PowerPoint, HTML, XML, RTF, Markdown, CSV, JSON, EPUB, Email, Archives, and more"
										>
											<svg
												class="w-4 h-4 inline-block mr-1"
												fill="none"
												stroke="currentColor"
												viewBox="0 0 24 24"
											>
												<path
													stroke-linecap="round"
													stroke-linejoin="round"
													stroke-width="2"
													d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
												></path>
											</svg>
											File types filtered
										</span>
									{/if}
									<label class="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
										<span>Per page:</span>
										<select
											bind:value={pageSize}
											onchange={() => changePageSize(pageSize)}
											class="pl-3 pr-8 py-1 border border-gray-300 dark:border-gray-600 rounded-lg dark:bg-gray-700 dark:text-white"
										>
											<option value={10}>10</option>
											<option value={25}>25</option>
											<option value={50}>50</option>
											<option value={100}>100</option>
										</select>
									</label>
								</div>
							</div>

							{#if uploadProgress}
								<UploadProgressPanel
									{uploadProgress}
									{fileStatuses}
									isUploading={uploading}
									onDismiss={() => {
										uploadProgress = null;
										fileStatuses = [];
									}}
								/>
							{/if}
						</div>

						{#if paginatedFiles && paginatedFiles.files.length > 0}
							<div class="mb-4 mt-4 px-6">
								<div class="relative">
									<input
										type="text"
										bind:value={searchQuery}
										placeholder="Search files by name or type..."
										class="w-full px-4 py-2 pl-10 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
									/>
									<svg
										class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
									>
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
										/>
									</svg>
								</div>
							</div>
						{/if}

						{#if filesLoading}
							<LoadingState message="Loading files..." />
						{:else if error}
							<div class="p-4">
								<div
									class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
								>
									<p class="text-red-700 dark:text-red-400">{error}</p>
									<button
										onclick={fetchFiles}
										class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
									>
										Try again
									</button>
								</div>
							</div>
						{:else if paginatedFiles && paginatedFiles.files.length === 0}
							<div class="p-12 text-center">
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
										d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"
									></path>
								</svg>
								<p class="text-gray-500 dark:text-gray-400">No files in this collection</p>
							</div>
						{:else if paginatedFiles && filteredFiles.length === 0}
							<div class="p-12 text-center">
								<p class="text-gray-500 dark:text-gray-400 mb-4">No files match your search</p>
								<button
									onclick={() => (searchQuery = '')}
									class="text-blue-600 dark:text-blue-400 hover:underline"
								>
									Clear search
								</button>
							</div>
						{:else if paginatedFiles}
							<div class="overflow-x-auto">
								<table class="w-full">
									<thead class="bg-gray-50 dark:bg-gray-700">
										<tr>
											<th
												class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
											>
												File Name
											</th>
											<th
												class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
											>
												Size
											</th>
											<th
												class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
											>
												Last Modified
											</th>
											<th
												class="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
											>
												Actions
											</th>
										</tr>
									</thead>
									<tbody
										class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700"
									>
										{#each filteredFiles as file (file.key)}
											<tr class="hover:bg-gray-50 dark:hover:bg-gray-700">
												<td class="px-6 py-4 whitespace-nowrap">
													<div class="flex items-center">
														<svg
															class="w-5 h-5 mr-2 text-gray-400"
															fill="none"
															stroke="currentColor"
															viewBox="0 0 24 24"
														>
															<path
																stroke-linecap="round"
																stroke-linejoin="round"
																stroke-width="2"
																d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"
															></path>
														</svg>
														<span class="text-sm text-gray-900 dark:text-white">
															{file.key}
														</span>
													</div>
												</td>
												<td
													class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400"
												>
													{formatFileSize(file.size)}
												</td>
												<td
													class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400"
												>
													{formatDate(file.last_modified)}
												</td>
												<td class="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
													<div class="flex items-center justify-end gap-2">
														<button
															onclick={() => downloadFile(file)}
															class="px-3 py-1.5 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors inline-flex items-center gap-1.5"
															title="Download file"
														>
															<svg
																class="w-4 h-4"
																fill="none"
																stroke="currentColor"
																viewBox="0 0 24 24"
															>
																<path
																	stroke-linecap="round"
																	stroke-linejoin="round"
																	stroke-width="2"
																	d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
																></path>
															</svg>
															Download
														</button>
														<button
															onclick={() => requestDeleteFile(file)}
															disabled={deletingFile === file.key}
															class="px-3 py-1.5 bg-red-600 text-white rounded hover:bg-red-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors inline-flex items-center gap-1.5"
															title="Delete file"
														>
															{#if deletingFile === file.key}
																<span class="animate-spin">⏳</span>
																Deleting...
															{:else}
																<svg
																	class="w-4 h-4"
																	fill="none"
																	stroke="currentColor"
																	viewBox="0 0 24 24"
																>
																	<path
																		stroke-linecap="round"
																		stroke-linejoin="round"
																		stroke-width="2"
																		d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
																	></path>
																</svg>
																Delete
															{/if}
														</button>
													</div>
												</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</div>

							{#if pageSize != 0 || paginatedFiles.has_more}
								<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700">
									<div class="flex items-center justify-between">
										<div class="flex gap-2">
											<button
												onclick={goToPreviousPage}
												disabled={currentPage === 0}
												class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm text-gray-700 dark:text-gray-300"
											>
												Previous
											</button>
											<span class="px-3 py-2 text-sm text-gray-700 dark:text-gray-300">
												Page {currentPage + 1}
											</span>
											<button
												onclick={goToNextPage}
												disabled={!canGoToNextPage}
												class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm text-gray-700 dark:text-gray-300"
											>
												Next
											</button>
										</div>
									</div>
								</div>
							{/if}
						{/if}
					</div>
				{:else if tabId === 'transforms'}
					<div id="transforms-panel" role="tabpanel" class="animate-fadeIn">
						<div>
							{#if transformsLoading}
								<div class="flex items-center justify-center py-8">
									<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
								</div>
							{:else if collectionTransforms.length === 0}
								<div
									class="text-center py-8 bg-gray-50 dark:bg-gray-900/30 rounded-lg border border-dashed border-gray-300 dark:border-gray-700"
								>
									<p class="text-gray-500 dark:text-gray-400 mb-2">
										No transforms use this collection yet.
									</p>
									<p class="text-sm text-gray-400 dark:text-gray-500">
										Create a transform to process files from this collection into a dataset.
									</p>
									<div class="mt-4">
										<button
											onclick={() => (transformModalOpen = true)}
											class="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium"
											title="Process files from this collection into a dataset"
										>
											<ExpandOutline class="w-5 h-5" />
											Create Collection Transform
										</button>
									</div>
								</div>
							{:else}
								<div class="flex justify-between items-center mb-6">
									<h2 class="text-2xl font-bold text-gray-900 dark:text-white">Transforms</h2>
									<button
										onclick={() => (transformModalOpen = true)}
										class="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium"
										title="Process files from this collection into a dataset"
									>
										<ExpandOutline class="w-5 h-5" />
										Create Collection Transform
									</button>
								</div>
								<div>
									<h3
										class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-3 flex items-center gap-2"
									>
										<span
											class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 text-xs font-bold"
										>
											{collectionTransforms.length}
										</span>
										Collection Transforms
									</h3>
									<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
										Transforms that process files from this collection
									</p>
									<TransformsList
										transforms={collectionTransforms.map((t) => ({
											...t,
											last_run_stats: collectionTransformStatsMap.get(t.collection_transform_id),
										}))}
										type="collection"
										loading={transformsLoading}
									/>
								</div>
							{/if}
						</div>
					</div>
				{:else if tabId === 'failed'}
					<div id="failed-files-panel" role="tabpanel" class="animate-fadeIn">
						<div class="p-4">
							<div class="flex justify-between items-center mb-6">
								<h2 class="text-2xl font-bold text-gray-900 dark:text-white">Failed Files</h2>
								{#if failedFilesCount > 0}
									<span
										class="px-3 py-1 bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 rounded-full text-sm font-medium"
									>
										{failedFilesCount} failed
									</span>
								{/if}
							</div>

							<!-- Upload failures (client-side, from this session) -->
							{#if uploadFailures.length > 0}
								<div class="mb-6">
									<div class="flex justify-between items-center mb-3">
										<h3
											class="text-lg font-semibold text-gray-800 dark:text-gray-200 flex items-center gap-2"
										>
											<span
												class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-xs font-bold"
											>
												{uploadFailures.length}
											</span>
											Upload Failures
										</h3>
										<button
											onclick={() => {
												uploadFailures = [];
											}}
											class="text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300 underline"
										>
											Clear
										</button>
									</div>
									<p class="text-sm text-gray-500 dark:text-gray-400 mb-3">
										These files failed to upload to storage. They were not added to the collection.
									</p>
									<div class="overflow-x-auto">
										<table class="w-full text-sm text-left text-gray-600 dark:text-gray-400">
											<thead
												class="bg-red-50 dark:bg-red-900/10 border-b border-red-200 dark:border-red-800"
											>
												<tr>
													<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">File</th
													>
													<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white"
														>Error</th
													>
													<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Time</th
													>
												</tr>
											</thead>
											<tbody>
												{#each uploadFailures as file, i (file.name + '-' + i)}
													<tr
														class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
													>
														<td class="px-4 py-3 font-medium text-gray-900 dark:text-white">
															<div class="flex items-center gap-2">
																<span class="text-red-500">❌</span>
																<span class="truncate max-w-xs" title={file.name}>{file.name}</span>
															</div>
														</td>
														<td class="px-4 py-3">
															<div
																class="text-red-600 dark:text-red-400 text-xs bg-red-50 dark:bg-red-900/20 px-2 py-1 rounded max-w-md wrap-break-word whitespace-pre-wrap"
															>
																{file.error}
															</div>
														</td>
														<td
															class="px-4 py-3 text-gray-500 dark:text-gray-400 whitespace-nowrap"
														>
															{formatDate(file.timestamp)}
														</td>
													</tr>
												{/each}
											</tbody>
										</table>
									</div>
								</div>
							{/if}

							<!-- Transform processing failures (server-side) -->
							{#if failedFilesLoading}
								<div class="flex items-center justify-center py-8">
									<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
								</div>
							{:else if failedFiles.length === 0 && failedFilesTotalCount === 0 && uploadFailures.length === 0}
								<div
									class="text-center py-8 bg-gray-50 dark:bg-gray-900/30 rounded-lg border border-dashed border-gray-300 dark:border-gray-700"
								>
									<div class="text-4xl mb-3">✅</div>
									<p class="text-gray-500 dark:text-gray-400 mb-2">No failed files!</p>
									<p class="text-sm text-gray-400 dark:text-gray-500">
										All files have been uploaded and processed successfully.
									</p>
								</div>
							{:else if failedFiles.length > 0}
								<div class="mb-3">
									<h3
										class="text-lg font-semibold text-gray-800 dark:text-gray-200 flex items-center gap-2"
									>
										<span
											class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-xs font-bold"
										>
											{failedFilesTotalCount}
										</span>
										Transform Processing Failures
									</h3>
									<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
										These files were uploaded but failed during transform processing.
									</p>
								</div>
								<div class="overflow-x-auto">
									<table class="w-full text-sm text-left text-gray-600 dark:text-gray-400">
										<thead
											class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700"
										>
											<tr>
												<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">File</th>
												<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white"
													>Transform</th
												>
												<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Error</th>
												<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white"
													>Failed At</th
												>
											</tr>
										</thead>
										<tbody>
											{#each failedFiles as file (file.id)}
												<tr
													class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
												>
													<td class="px-4 py-3 font-medium text-gray-900 dark:text-white">
														<div class="flex items-center gap-2">
															<span class="text-red-500">❌</span>
															<span class="truncate max-w-xs" title={file.file_key}
																>{file.file_key}</span
															>
														</div>
													</td>
													<td class="px-4 py-3">
														<span
															class="px-2 py-1 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded text-xs font-medium"
														>
															{file.transform_title}
														</span>
													</td>
													<td class="px-4 py-3">
														{#if file.process_error}
															<div
																class="text-red-600 dark:text-red-400 text-xs bg-red-50 dark:bg-red-900/20 px-2 py-1 rounded max-w-md wrap-break-word whitespace-pre-wrap"
															>
																{file.process_error}
															</div>
														{:else}
															<span class="text-gray-400 italic">Unknown error</span>
														{/if}
													</td>
													<td class="px-4 py-3 text-gray-500 dark:text-gray-400 whitespace-nowrap">
														{formatDate(file.processed_at)}
													</td>
												</tr>
											{/each}
										</tbody>
									</table>
								</div>

								<!-- Pagination -->
								{#if failedFilesTotalCount > failedFilesPageSize}
									<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700">
										<div class="flex items-center justify-between">
											<span class="text-sm text-gray-700 dark:text-gray-300">
												Showing {failedFilesPage * failedFilesPageSize + 1}-{Math.min(
													(failedFilesPage + 1) * failedFilesPageSize,
													failedFilesTotalCount
												)} of {failedFilesTotalCount}
											</span>
											<div class="flex gap-2">
												<button
													onclick={() => {
														failedFilesPage--;
														fetchFailedFiles();
													}}
													disabled={failedFilesPage === 0}
													class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm text-gray-700 dark:text-gray-300"
												>
													Previous
												</button>
												<span class="px-3 py-2 text-sm text-gray-700 dark:text-gray-300">
													Page {failedFilesPage + 1} of {Math.ceil(
														failedFilesTotalCount / failedFilesPageSize
													)}
												</span>
												<button
													onclick={() => {
														failedFilesPage++;
														fetchFailedFiles();
													}}
													disabled={(failedFilesPage + 1) * failedFilesPageSize >=
														failedFilesTotalCount}
													class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm text-gray-700 dark:text-gray-300"
												>
													Next
												</button>
											</div>
										</div>
									</div>
								{/if}
							{/if}
						</div>
					</div>
				{/if}
			{/snippet}
		</TabPanel>
	</div>
</div>

<ConfirmDialog
	open={collectionPendingDelete}
	title="Delete Collection?"
	message="Are you sure you want to delete this collection? This action cannot be undone."
	confirmLabel="Delete"
	cancelLabel="Cancel"
	onConfirm={confirmDeleteCollection}
	onCancel={() => (collectionPendingDelete = false)}
	variant="danger"
/>

<ConfirmDialog
	open={Boolean(filePendingDelete)}
	title="Delete file?"
	message={`Are you sure you want to delete "${filePendingDelete?.key ?? ''}"? This action cannot be undone.`}
	confirmLabel="Delete file"
	cancelLabel="Cancel"
	onConfirm={confirmDeleteFile}
	onCancel={cancelDeleteFile}
/>

<CreateCollectionTransformModal
	bind:open={transformModalOpen}
	{collectionId}
	collectionTitle={collection?.title}
	onSuccess={() => {
		transformModalOpen = false;
		// Redirect to datasets page to monitor transform progress
		window.location.hash = '#/datasets';
	}}
/>

<ApiIntegrationModal
	bind:open={apiIntegrationModalOpen}
	type="collection"
	resourceId={collectionId}
/>
