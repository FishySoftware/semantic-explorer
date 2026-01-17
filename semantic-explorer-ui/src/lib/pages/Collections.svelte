<script lang="ts">
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import ActionMenu from '../components/ActionMenu.svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import CreateCollectionTransformModal from '../components/CreateCollectionTransformModal.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Collection {
		collection_id: number;
		title: string;
		details: string | null;
		owner: string;
		bucket: string;
		tags: string[];
		created_at?: string;
		updated_at?: string;
		file_count: number;
	}

	interface PaginatedCollectionList {
		collections: Collection[];
		total_count: number;
		limit: number;
		offset: number;
	}

	let { onViewCollection: handleViewCollection } = $props<{
		onViewCollection: (_: number) => void;
	}>();

	const onViewCollection = (id: number) => {
		handleViewCollection(id);
	};

	const onCreateTransform = (collectionId: number) => {
		selectedCollectionForTransform = collectionId;
		transformModalOpen = true;
	};

	let collections = $state<Collection[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let totalCount = $state(0);
	let currentOffset = $state(0);
	const pageSize = 20;

	let searchQuery = $state('');

	let filteredCollections = $derived(collections);

	let showCreateForm = $state(false);
	let newTitle = $state('');
	let newDetails = $state('');
	let newTags = $state('');
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let collectionPendingDelete = $state<Collection | null>(null);

	let transformModalOpen = $state(false);
	let selectedCollectionForTransform = $state<number | null>(null);

	$effect(() => {
		if (showCreateForm && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `collection-${date}-${time}`;
		}
	});

	async function fetchCollections() {
		try {
			loading = true;
			error = null;
			const params = new SvelteURLSearchParams();
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			params.append('limit', pageSize.toString());
			params.append('offset', currentOffset.toString());
			const url = `/api/collections?${params.toString()}`;
			const response = await fetch(url);
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			const data: PaginatedCollectionList = await response.json();
			collections = data.collections;
			totalCount = data.total_count;
		} catch (e) {
			const message = formatError(e, 'Failed to fetch collections');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function createCollection() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		const tags = newTags
			.split(',')
			.map((tag) => tag.trim())
			.filter((tag) => tag.length > 0);

		try {
			creating = true;
			createError = null;
			const response = await fetch('/api/collections', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					title: newTitle,
					details: newDetails.trim() || null,
					tags,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to create collection: ${response.statusText}`);
			}

			const newCollection = await response.json();
			collections = [...collections, newCollection];

			newTitle = '';
			newDetails = '';
			newTags = '';
			showCreateForm = false;
			toastStore.success('Collection created successfully');
			handleViewCollection(newCollection.collection_id);
		} catch (e) {
			const message = formatError(e, 'Failed to create collection');
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	function requestDeleteCollection(collection: Collection) {
		collectionPendingDelete = collection;
	}

	async function confirmDeleteCollection() {
		if (!collectionPendingDelete) {
			return;
		}

		const target = collectionPendingDelete;
		collectionPendingDelete = null;

		try {
			const response = await fetch(`/api/collections/${target.collection_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete collection: ${response.statusText}`);
			}

			collections = collections.filter((c) => c.collection_id !== target.collection_id);
			toastStore.success('Collection deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete collection'));
		}
	}

	// Refetch when search query changes
	// Debounce search to avoid spamming API on every keystroke
	let searchDebounceTimeout: ReturnType<typeof setTimeout> | null = null;
	$effect(() => {
		if (searchQuery !== undefined) {
			currentOffset = 0; // Reset to first page when searching
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
			searchDebounceTimeout = setTimeout(() => {
				fetchCollections();
			}, 300); // 300ms debounce
		}
		return () => {
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
		};
	});

	onMount(() => {
		fetchCollections();
	});

	function goToPreviousPage() {
		currentOffset = Math.max(0, currentOffset - pageSize);
		fetchCollections();
	}

	function goToNextPage() {
		if (currentOffset + pageSize < totalCount) {
			currentOffset += pageSize;
			fetchCollections();
		}
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Collections"
		description="Organize collections of documents of interest. You can add as many files as you want, up to 1GB per file. Most common content types are supported including Office documents (Word, Excel, PowerPoint), HTML, XML, and raw text files."
	/>

	<div class="flex justify-between items-center mb-4">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Collections</h1>
		<button onclick={() => (showCreateForm = !showCreateForm)} class="btn-primary">
			{showCreateForm ? 'Cancel' : 'Create Collection'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				Create New Collection
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					createCollection();
				}}
			>
				<div class="mb-4">
					<label
						for="collection-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Title
					</label>
					<input
						id="collection-title"
						type="text"
						bind:value={newTitle}
						placeholder="Enter collection title"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
						required
					/>
				</div>
				<div class="mb-4">
					<label
						for="collection-details"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Details
					</label>
					<textarea
						id="collection-details"
						bind:value={newDetails}
						placeholder="Enter collection details (optional)"
						rows="3"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
					></textarea>
				</div>
				<div class="mb-4">
					<label
						for="collection-tags"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Tags <span class="text-xs text-gray-500 dark:text-gray-400">(comma-separated)</span>
					</label>
					<input
						id="collection-tags"
						type="text"
						bind:value={newTags}
						placeholder="e.g., documents, images, reports"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
					/>
				</div>
				{#if createError}
					<div
						class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400"
					>
						{createError}
					</div>
				{/if}
				<div class="flex gap-3">
					<button
						type="submit"
						disabled={creating}
						class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{creating ? 'Creating...' : 'Create'}
					</button>
					<button
						type="button"
						onclick={() => {
							showCreateForm = false;
							newTitle = '';
							newDetails = '';
							newTags = '';
							createError = null;
						}}
						class="btn-secondary"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	{#if !showCreateForm}
		<div class="mb-4">
			<div class="relative">
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search collections by title, details, tags, or owner..."
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

	{#if loading}
		<div class="flex items-center justify-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
			<button
				onclick={fetchCollections}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if collections.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No collections yet</p>
			<button
				onclick={() => (showCreateForm = true)}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Create your first collection
			</button>
		</div>
	{:else if filteredCollections.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No collections match your search</p>
			<button
				onclick={() => (searchQuery = '')}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Clear search
			</button>
		</div>
	{:else}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Title</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Files</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Owner</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Tags</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredCollections as collection (collection.collection_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3">
								<button
									onclick={() => onViewCollection(collection.collection_id)}
									class="font-semibold text-blue-600 dark:text-blue-400 hover:underline"
								>
									{collection.title}
								</button>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								{#if collection.file_count !== undefined && collection.file_count !== null}
									<span
										class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-sm font-medium"
									>
										{collection.file_count}
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400">—</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span class="text-gray-700 dark:text-gray-300">{collection.owner}</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<div class="flex flex-wrap gap-1">
									{#if collection.tags && collection.tags.length > 0}
										{#each collection.tags.slice(0, 2) as tag (tag)}
											<span
												class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
											>
												#{tag}
											</span>
										{/each}
										{#if collection.tags.length > 2}
											<span class="text-xs text-gray-500 dark:text-gray-400 px-2 py-1">
												+{collection.tags.length - 2} more
											</span>
										{/if}
									{/if}
								</div>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								<ActionMenu
									actions={[
										{
											label: 'View Files',
											handler: () => onViewCollection(collection.collection_id),
										},
										...(collection.file_count && collection.file_count > 0
											? [
													{
														label: 'Create Transform',
														handler: () => onCreateTransform(collection.collection_id),
													},
												]
											: []),
										{
											label: 'Delete',
											handler: () => requestDeleteCollection(collection),
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
					collections
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

<ConfirmDialog
	open={collectionPendingDelete !== null}
	title="Delete collection"
	message={collectionPendingDelete
		? `Are you sure you want to delete "${collectionPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteCollection}
	on:cancel={() => (collectionPendingDelete = null)}
/>

<CreateCollectionTransformModal
	open={transformModalOpen}
	collectionId={selectedCollectionForTransform}
	onSuccess={() => {
		transformModalOpen = false;
		selectedCollectionForTransform = null;
		// Redirect to datasets page to monitor transform progress
		window.location.hash = '#/datasets';
	}}
/>
