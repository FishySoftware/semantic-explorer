<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
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
		file_count?: number;
	}

	let { onViewCollection: handleViewCollection, onNavigate } = $props<{
		onViewCollection: (_: number) => void;
		onNavigate: (_: string) => void;
	}>();

	const onViewCollection = (id: number) => {
		handleViewCollection(id);
	};

	const onCreateTransform = (collectionId: number) => {
		onNavigate(`/collection-transforms?action=create&collection_id=${collectionId}`);
	};

	let collections = $state<Collection[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let newTitle = $state('');
	let newDetails = $state('');
	let newTags = $state('');
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let collectionPendingDelete = $state<Collection | null>(null);

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
			const response = await fetch('/api/collections');
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			collections = await response.json();
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
			deleting = target.collection_id;
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
		} finally {
			deleting = null;
		}
	}

	onMount(() => {
		fetchCollections();
	});

	let filteredCollections = $derived(
		collections.filter((c) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				c.title.toLowerCase().includes(query) ||
				c.details?.toLowerCase().includes(query) ||
				c.tags.some((tag) => tag.toLowerCase().includes(query)) ||
				c.owner.toLowerCase().includes(query)
			);
		})
	);
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Collections"
		description="Organize collections of documents of interest. You can add as many files as you want, up to 1GB per file. Most common content types are supported including Office documents (Word, Excel, PowerPoint), HTML, XML, and raw text files."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Collections</h1>
		<button
			onclick={() => (showCreateForm = !showCreateForm)}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Collection'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
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
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
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
						class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	{#if !showCreateForm && collections.length > 0}
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
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No collections yet</p>
			<button
				onclick={() => (showCreateForm = true)}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Create your first collection
			</button>
		</div>
	{:else if filteredCollections.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No collections match your search</p>
			<button
				onclick={() => (searchQuery = '')}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Clear search
			</button>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredCollections as collection (collection.collection_id)}
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 hover:shadow-lg transition-shadow"
				>
					<div class="flex justify-between items-start">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h3 class="text-xl font-semibold text-gray-900 dark:text-white">
									{collection.title}
								</h3>
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
								{#if collection.file_count !== undefined && collection.file_count !== null}
									<span
										class="inline-flex items-center gap-1.5 px-3 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded-full text-sm font-medium"
									>
										<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"
											></path>
										</svg>
										{collection.file_count}
										{collection.file_count === 1 ? 'file' : 'files'}
									</span>
								{/if}
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
						<div class="ml-4 flex flex-col gap-2">
							<button
								onclick={() => onViewCollection(collection.collection_id)}
								class="px-3 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
								title="Manage files"
							>
								<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
									></path>
								</svg>
								Manage Files
							</button>
							{#if collection.file_count && collection.file_count > 0}
								<button
									onclick={() => onCreateTransform(collection.collection_id)}
									class="px-3 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors flex items-center gap-2"
									title="Process files from this collection into a dataset"
								>
									<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"
										></path>
									</svg>
									Create Transform
								</button>
							{/if}
							<button
								onclick={() => requestDeleteCollection(collection)}
								disabled={deleting === collection.collection_id}
								class="px-3 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors flex items-center gap-2"
								title="Delete collection"
							>
								{#if deleting === collection.collection_id}
									<span class="animate-spin" role="status" aria-label="Deleting collection">⏳</span
									>
									Deleting...
								{:else}
									<svg
										class="w-4 h-4"
										fill="none"
										stroke="currentColor"
										viewBox="0 0 24 24"
										aria-hidden="true"
									>
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
										/>
									</svg>
									Delete
								{/if}
							</button>
						</div>
					</div>
				</div>
			{/each}
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
