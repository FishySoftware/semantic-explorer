<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import ApiExamples from '../ApiExamples.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import { formatError, toastStore } from '../utils/notifications';

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

	interface Collection {
		collection_id: number;
		title: string;
		details: string | null;
		owner: string;
		bucket: string;
		tags: string[];
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

	// Track pagination history for cursor-based pagination
	let paginationHistory = $state<(string | null)[]>([null]); // Start with null token for first page
	let currentPageIndex = $state(0);
	let totalCount = $state<number | null>(null); // Preserve total count across pagination

	let searchQuery = $state('');

	let uploading = $state(false);
	let uploadProgress = $state<{
		completed: number;
		total: number;
		currentBatch: number;
		totalBatches: number;
	} | null>(null);
	let fileInputRef: HTMLInputElement;

	onMount(async () => {
		await Promise.all([fetchCollection(), fetchFiles()]);
	});

	async function fetchCollection() {
		try {
			pageLoading = true;
			const response = await fetch('/api/collections');
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			const collections: Collection[] = await response.json();
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

			// Preserve total count from first page load
			if (
				paginatedFiles &&
				paginatedFiles.total_count !== null &&
				paginatedFiles.total_count !== undefined
			) {
				totalCount = paginatedFiles.total_count;
			}
			// Restore total count on subsequent pages
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

			// Decrement total count if we have it
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

	function formatFileSize(bytes: number): string {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
	}

	function formatDate(dateString: string | null): string {
		if (!dateString) return 'N/A';
		try {
			const date = new Date(dateString);
			return date.toLocaleString();
		} catch {
			return dateString;
		}
	}

	function goToNextPage() {
		if (!paginatedFiles?.has_more) return;

		// If we have a continuation token, add it to history if not already there
		if (paginatedFiles.continuation_token) {
			// Move to next page
			currentPageIndex++;
			// Add the token to history if we're moving to a new page
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
		paginatedFiles?.files.filter((file) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				file.key.toLowerCase().includes(query) || file.content_type?.toLowerCase().includes(query)
			);
		}) || []
	);

	function changePageSize(newSize: number) {
		pageSize = newSize;
		currentPage = 0;
		currentPageIndex = 0;
		paginationHistory = [null]; // Reset pagination history
		totalCount = null; // Reset total count to refetch
		fetchFiles();
	}

	async function uploadFiles(files: FileList | null) {
		if (!files || files.length === 0) {
			return;
		}

		uploading = true;
		const allCompleted: string[] = [];
		const allFailed: string[] = [];
		const fileArray = Array.from(files);
		const totalFiles = fileArray.length;
		const batchSize = 20;
		const totalBatches = Math.ceil(totalFiles / batchSize);

		try {
			for (let i = 0; i < totalFiles; i += batchSize) {
				const batch = fileArray.slice(i, i + batchSize);
				const currentBatch = Math.floor(i / batchSize) + 1;

				uploadProgress = {
					completed: allCompleted.length + allFailed.length,
					total: totalFiles,
					currentBatch,
					totalBatches,
				};

				const formData = new FormData();
				batch.forEach((file) => {
					formData.append('files', file);
				});

				const response = await fetch(`/api/collections/${collectionId}/files`, {
					method: 'POST',
					body: formData,
				});

				if (!response.ok) {
					batch.forEach((file) => allFailed.push(file.name));
					continue;
				}

				const result = await response.json();
				allCompleted.push(...result.completed);
				allFailed.push(...result.failed);
			}

			if (allCompleted.length > 0) {
				// Reset to first page to see newly uploaded files
				currentPage = 0;
				currentPageIndex = 0;
				paginationHistory = [null];
				totalCount = null; // Reset total count to refetch
				await fetchFiles();
				toastStore.success(
					`Successfully uploaded ${allCompleted.length} file${allCompleted.length === 1 ? '' : 's'}`
				);
			}

			if (allFailed.length > 0) {
				toastStore.warning(
					`Failed to upload ${allFailed.length} file${allFailed.length === 1 ? '' : 's'}`
				);
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to upload files'));
		} finally {
			uploading = false;
			uploadProgress = null;
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

<div class="max-w-7xl mx-auto">
	<div class="mb-2">
		<button
			onclick={onBack}
			class="mb-2 px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors inline-flex items-center gap-2"
		>
			<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M10 19l-7-7m0 0l7-7m-7 7h18"
				></path>
			</svg>
			Back to Collections
		</button>

		{#if pageLoading}
			<div class="flex items-center justify-center py-12">
				<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
			</div>
		{:else if collection}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
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
					<span
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
					</span>
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
			</div>
		{/if}
	</div>

	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md">
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
							<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
								></path>
							</svg>
							Upload Files
						{/if}
					</button>
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

			{#if uploading && uploadProgress}
				<div
					class="mt-4 p-4 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg"
				>
					<div class="flex items-center justify-between mb-2">
						<span class="text-sm font-medium text-blue-700 dark:text-blue-400">
							Uploading batch {uploadProgress.currentBatch} of {uploadProgress.totalBatches}
						</span>
						<span class="text-sm text-blue-600 dark:text-blue-400">
							{uploadProgress.completed} / {uploadProgress.total} files processed
						</span>
					</div>
					<div class="w-full bg-blue-200 dark:bg-blue-900 rounded-full h-2.5">
						<div
							class="bg-blue-600 h-2.5 rounded-full transition-all duration-300"
							style="width: {(uploadProgress.completed / uploadProgress.total) * 100}%"
						></div>
					</div>
				</div>
			{/if}
		</div>

		{#if paginatedFiles && paginatedFiles.files.length > 0}
			<div class="mb-4 px-6">
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
			<div class="flex items-center justify-center py-12">
				<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
			</div>
		{:else if error}
			<div class="p-6">
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
					<tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
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
								<td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
									{formatFileSize(file.size)}
								</td>
								<td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
									{formatDate(file.last_modified)}
								</td>
								<td class="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
									<div class="flex items-center justify-end gap-2">
										<button
											onclick={() => downloadFile(file)}
											class="px-3 py-1.5 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors inline-flex items-center gap-1.5"
											title="Download file"
										>
											<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
												<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
								disabled={!paginatedFiles.has_more}
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

	{#if collection}
		<div class="mt-6 bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
			<h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-4">API Integration</h2>
			<p class="text-sm text-gray-600 dark:text-gray-400 mb-6">
				Use these examples to interact with this collection programmatically.
			</p>

			<div class="mb-6">
				<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
					List files in collection
				</h3>
				<ApiExamples
					endpoint="/api/collections/{collectionId}/files?page=0&page_size=10"
					method="GET"
				/>
			</div>

			<div class="mb-6">
				<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
					Upload files (multipart/form-data)
				</h3>
				<p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
					Note: This endpoint requires multipart/form-data. Use FormData and append files with the
					key "files".
				</p>
				<ApiExamples endpoint="/api/collections/{collectionId}/files" method="POST" />
			</div>

			<div class="mb-6">
				<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
					Download a specific file
				</h3>
				<ApiExamples
					endpoint="/api/collections/{collectionId}/files/example-file.txt"
					method="GET"
				/>
			</div>

			<div>
				<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">Delete a file</h3>
				<ApiExamples
					endpoint="/api/collections/{collectionId}/files/example-file.txt"
					method="DELETE"
				/>
			</div>
		</div>
	{/if}
</div>

<ConfirmDialog
	open={Boolean(filePendingDelete)}
	title="Delete file?"
	message={`Are you sure you want to delete "${filePendingDelete?.key ?? ''}"? This action cannot be undone.`}
	confirmLabel="Delete file"
	cancelLabel="Cancel"
	on:confirm={confirmDeleteFile}
	on:cancel={cancelDeleteFile}
/>
