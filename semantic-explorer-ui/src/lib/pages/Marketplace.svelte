<script lang="ts">
	import { onMount } from 'svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface Collection {
		collection_id: number;
		title: string;
		details: string | null;
		owner: string;
		tags: string[];
		is_public: boolean;
		created_at?: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
		details: string | null;
		owner: string;
		tags: string[];
		is_public: boolean;
		created_at?: string;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
		owner: string;
		provider: string;
		base_url: string;
		dimensions: number;
		is_public: boolean;
		created_at?: string;
	}

	interface LLM {
		llm_id: number;
		name: string;
		owner: string;
		provider: string;
		base_url: string;
		is_public: boolean;
		created_at?: string;
	}

	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);
	let llms = $state<LLM[]>([]);

	let loading = $state(true);
	let error = $state<string | null>(null);

	let activeTab = $state<'collections' | 'datasets' | 'embedders' | 'llms'>('collections');
	let grabbingId = $state<number | null>(null);
	let searchQuery = $state('');

	function matchesSearch(text: string | null | undefined): boolean {
		if (!searchQuery.trim()) return true;
		const query = searchQuery.toLowerCase();
		const content = (text || '').toLowerCase();
		return content.includes(query);
	}

	function getFilteredCollections() {
		return collections.filter(
			(c) => matchesSearch(c.title) || matchesSearch(c.details) || matchesSearch(c.owner)
		);
	}

	function getFilteredDatasets() {
		return datasets.filter(
			(d) => matchesSearch(d.title) || matchesSearch(d.details) || matchesSearch(d.owner)
		);
	}

	function getFilteredEmbedders() {
		return embedders.filter(
			(e) =>
				matchesSearch(e.name) ||
				matchesSearch(e.provider) ||
				matchesSearch(e.owner) ||
				matchesSearch(e.base_url)
		);
	}

	function getFilteredLLMs() {
		return llms.filter(
			(l) =>
				matchesSearch(l.name) ||
				matchesSearch(l.provider) ||
				matchesSearch(l.owner) ||
				matchesSearch(l.base_url)
		);
	}

	async function fetchPublicResources() {
		try {
			loading = true;
			error = null;

			const [collectionsRes, datasetsRes, embeddersRes, llmsRes] = await Promise.all([
				fetch('/api/marketplace/collections'),
				fetch('/api/marketplace/datasets'),
				fetch('/api/marketplace/embedders'),
				fetch('/api/marketplace/llms'),
			]);

			if (!collectionsRes.ok || !datasetsRes.ok || !embeddersRes.ok || !llmsRes.ok) {
				throw new Error('Failed to fetch marketplace resources');
			}

			collections = await collectionsRes.json();
			datasets = await datasetsRes.json();
			embedders = await embeddersRes.json();
			llms = await llmsRes.json();
		} catch (e) {
			const message = formatError(e, 'Failed to fetch marketplace resources');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function grabCollection(id: number) {
		try {
			grabbingId = id;
			const response = await fetch(`/api/marketplace/collections/${id}/grab`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to grab collection: ${response.statusText}`);
			}

			const newCollection = await response.json();
			toastStore.success('Collection grabbed successfully!');
			window.location.hash = `#/collections/${newCollection.collection_id}/details`;
		} catch (e) {
			const message = formatError(e, 'Failed to grab collection');
			toastStore.error(message);
		} finally {
			grabbingId = null;
		}
	}

	async function grabDataset(id: number) {
		try {
			grabbingId = id;
			const response = await fetch(`/api/marketplace/datasets/${id}/grab`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to grab dataset: ${response.statusText}`);
			}

			const newDataset = await response.json();
			toastStore.success('Dataset grabbed successfully!');
			window.location.hash = `#/datasets/${newDataset.dataset_id}/details`;
		} catch (e) {
			const message = formatError(e, 'Failed to grab dataset');
			toastStore.error(message);
		} finally {
			grabbingId = null;
		}
	}

	async function grabEmbedder(id: number) {
		try {
			grabbingId = id;
			const response = await fetch(`/api/marketplace/embedders/${id}/grab`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to grab embedder: ${response.statusText}`);
			}

			const newEmbedder = await response.json();
			toastStore.success('Embedder grabbed successfully!');
			window.location.hash = `#/embedders/${newEmbedder.embedder_id}/details`;
		} catch (e) {
			const message = formatError(e, 'Failed to grab embedder');
			toastStore.error(message);
		} finally {
			grabbingId = null;
		}
	}

	async function grabLLM(id: number) {
		try {
			grabbingId = id;
			const response = await fetch(`/api/marketplace/llms/${id}/grab`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to grab LLM: ${response.statusText}`);
			}

			const newLLM = await response.json();
			toastStore.success('LLM grabbed successfully!');
			window.location.hash = `#/llms?name=${encodeURIComponent(newLLM.name)}`;
		} catch (e) {
			const message = formatError(e, 'Failed to grab LLM');
			toastStore.error(message);
		} finally {
			grabbingId = null;
		}
	}

	onMount(() => {
		fetchPublicResources();
	});
</script>

<div class="space-y-6 max-w-7xl mx-auto">
	<PageHeader
		title="Marketplace"
		description="Discover and grab public resources from the community"
	/>

	<!-- Tabs -->
	<div class="flex gap-2 border-b border-gray-200 dark:border-gray-700">
		<button
			class="px-4 py-2 border-b-2 font-medium text-sm transition-colors"
			class:border-blue-600={activeTab === 'collections'}
			class:text-blue-600={activeTab === 'collections'}
			class:border-transparent={activeTab !== 'collections'}
			class:text-gray-600={activeTab !== 'collections'}
			class:dark:text-gray-400={activeTab !== 'collections'}
			class:dark:text-blue-400={activeTab === 'collections'}
			onclick={() => (activeTab = 'collections')}
		>
			Collections ({collections.length})
		</button>
		<button
			class="px-4 py-2 border-b-2 font-medium text-sm transition-colors"
			class:border-blue-600={activeTab === 'datasets'}
			class:text-blue-600={activeTab === 'datasets'}
			class:border-transparent={activeTab !== 'datasets'}
			class:text-gray-600={activeTab !== 'datasets'}
			class:dark:text-gray-400={activeTab !== 'datasets'}
			class:dark:text-blue-400={activeTab === 'datasets'}
			onclick={() => (activeTab = 'datasets')}
		>
			Datasets ({datasets.length})
		</button>
		<button
			class="px-4 py-2 border-b-2 font-medium text-sm transition-colors"
			class:border-blue-600={activeTab === 'embedders'}
			class:text-blue-600={activeTab === 'embedders'}
			class:border-transparent={activeTab !== 'embedders'}
			class:text-gray-600={activeTab !== 'embedders'}
			class:dark:text-gray-400={activeTab !== 'embedders'}
			class:dark:text-blue-400={activeTab === 'embedders'}
			onclick={() => (activeTab = 'embedders')}
		>
			Embedders ({embedders.length})
		</button>
		<button
			class="px-4 py-2 border-b-2 font-medium text-sm transition-colors"
			class:border-blue-600={activeTab === 'llms'}
			class:text-blue-600={activeTab === 'llms'}
			class:border-transparent={activeTab !== 'llms'}
			class:text-gray-600={activeTab !== 'llms'}
			class:dark:text-gray-400={activeTab !== 'llms'}
			class:dark:text-blue-400={activeTab === 'llms'}
			onclick={() => (activeTab = 'llms')}
		>
			LLMs ({llms.length})
		</button>
	</div>

	<!-- Search Input -->
	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
		<input
			type="text"
			placeholder="Search resources by name, description, owner, or provider..."
			bind:value={searchQuery}
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-600 dark:focus:ring-blue-400"
		/>
	</div>

	<!-- Content -->
	{#if loading}
		<div class="flex justify-center items-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
		</div>
	{:else if error}
		<div class="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-4 rounded-lg">
			<p>{error}</p>
		</div>
	{:else if activeTab === 'collections'}
		{#if getFilteredCollections().length === 0}
			<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
				<p class="text-gray-600 dark:text-gray-400">
					{searchQuery ? 'No collections match your search' : 'No public collections available'}
				</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				{#each getFilteredCollections() as collection (collection.collection_id)}
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 flex flex-col gap-3">
						<div>
							<h3 class="font-semibold text-gray-900 dark:text-white mb-1">
								{collection.title}
							</h3>
							<p class="text-sm text-gray-600 dark:text-gray-400">by {collection.owner}</p>
						</div>
						{#if collection.details}
							<p class="text-sm text-gray-700 dark:text-gray-300">
								{collection.details}
							</p>
						{/if}
						{#if collection.tags.length > 0}
							<div class="flex flex-wrap gap-2">
								{#each collection.tags as tag (tag)}
									<span
										class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
									>
										{tag}
									</span>
								{/each}
							</div>
						{/if}
						<div
							class="flex justify-between items-center mt-auto pt-3 border-t border-gray-200 dark:border-gray-700"
						>
							<span class="text-xs text-gray-500 dark:text-gray-400"
								>{collection.created_at ? formatDate(collection.created_at, false) : ''}</span
							>
							<button
								onclick={() => grabCollection(collection.collection_id)}
								disabled={grabbingId === collection.collection_id}
								class="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white text-sm font-medium rounded transition-colors"
							>
								{grabbingId === collection.collection_id ? 'Grabbing...' : 'Grab'}
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{:else if activeTab === 'datasets'}
		{#if getFilteredDatasets().length === 0}
			<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
				<p class="text-gray-600 dark:text-gray-400">
					{searchQuery ? 'No datasets match your search' : 'No public datasets available'}
				</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				{#each getFilteredDatasets() as dataset (dataset.dataset_id)}
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 flex flex-col gap-3">
						<div>
							<h3 class="font-semibold text-gray-900 dark:text-white mb-1">
								{dataset.title}
							</h3>
							<p class="text-sm text-gray-600 dark:text-gray-400">by {dataset.owner}</p>
						</div>
						{#if dataset.details}
							<p class="text-sm text-gray-700 dark:text-gray-300">
								{dataset.details}
							</p>
						{/if}
						{#if dataset.tags.length > 0}
							<div class="flex flex-wrap gap-2">
								{#each dataset.tags as tag (tag)}
									<span
										class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs font-medium"
									>
										{tag}
									</span>
								{/each}
							</div>
						{/if}
						<div
							class="flex justify-between items-center mt-auto pt-3 border-t border-gray-200 dark:border-gray-700"
						>
							<span class="text-xs text-gray-500 dark:text-gray-400"
								>{dataset.created_at ? formatDate(dataset.created_at, false) : ''}</span
							>
							<button
								onclick={() => grabDataset(dataset.dataset_id)}
								disabled={grabbingId === dataset.dataset_id}
								class="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white text-sm font-medium rounded transition-colors"
							>
								{grabbingId === dataset.dataset_id ? 'Grabbing...' : 'Grab'}
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{:else if activeTab === 'embedders'}
		{#if getFilteredEmbedders().length === 0}
			<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
				<p class="text-gray-600 dark:text-gray-400">
					{searchQuery ? 'No embedders match your search' : 'No public embedders available'}
				</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				{#each getFilteredEmbedders() as embedder (embedder.embedder_id)}
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 flex flex-col gap-3">
						<div>
							<h3 class="font-semibold text-gray-900 dark:text-white mb-1">
								{embedder.name}
							</h3>
							<p class="text-sm text-gray-600 dark:text-gray-400">by {embedder.owner}</p>
						</div>
						<div class="space-y-2 text-sm">
							<div class="flex gap-2">
								<span class="font-medium text-gray-600 dark:text-gray-400 min-w-fit">Provider:</span
								>
								>
								<span class="text-gray-700 dark:text-gray-300">{embedder.provider}</span>
							</div>
							<div class="flex gap-2">
								<span class="font-medium text-gray-600 dark:text-gray-400 min-w-fit"
									>Dimensions:</span
								>
								<span class="text-gray-700 dark:text-gray-300">{embedder.dimensions}</span>
							</div>
							<div class="flex gap-2">
								<span class="font-medium text-gray-600 dark:text-gray-400 min-w-fit">Base URL:</span
								>
								>
								<span class="text-gray-700 dark:text-gray-300 font-mono text-xs break-all">
									{embedder.base_url}
								</span>
							</div>
						</div>
						<div
							class="flex justify-between items-center mt-auto pt-3 border-t border-gray-200 dark:border-gray-700"
						>
							<span class="text-xs text-gray-500 dark:text-gray-400"
								>{embedder.created_at ? formatDate(embedder.created_at, false) : ''}</span
							>
							>
							<button
								onclick={() => grabEmbedder(embedder.embedder_id)}
								disabled={grabbingId === embedder.embedder_id}
								class="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white text-sm font-medium rounded transition-colors"
							>
								{grabbingId === embedder.embedder_id ? 'Grabbing...' : 'Grab'}
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{:else if activeTab === 'llms'}
		{#if getFilteredLLMs().length === 0}
			<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
				<p class="text-gray-600 dark:text-gray-400">
					{searchQuery ? 'No LLMs match your search' : 'No public LLMs available'}
				</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				{#each getFilteredLLMs() as llm (llm.llm_id)}
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 flex flex-col gap-3">
						<div>
							<h3 class="font-semibold text-gray-900 dark:text-white mb-1">
								{llm.name}
							</h3>
							<p class="text-sm text-gray-600 dark:text-gray-400">by {llm.owner}</p>
						</div>
						<div class="space-y-2 text-sm">
							<div class="flex gap-2">
								<span class="font-medium text-gray-600 dark:text-gray-400 min-w-fit">Provider:</span
								>
								>
								<span class="text-gray-700 dark:text-gray-300">{llm.provider}</span>
							</div>
							<div class="flex gap-2">
								<span class="font-medium text-gray-600 dark:text-gray-400 min-w-fit">Base URL:</span
								>
								>
								<span class="text-gray-700 dark:text-gray-300 font-mono text-xs break-all">
									{llm.base_url}
								</span>
							</div>
						</div>
						<div
							class="flex justify-between items-center mt-auto pt-3 border-t border-gray-200 dark:border-gray-700"
						>
							<span class="text-xs text-gray-500 dark:text-gray-400"
								>{llm.created_at ? formatDate(llm.created_at, false) : ''}</span
							>
							>
							<button
								onclick={() => grabLLM(llm.llm_id)}
								disabled={grabbingId === llm.llm_id}
								class="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white text-sm font-medium rounded transition-colors"
							>
								{grabbingId === llm.llm_id ? 'Grabbing...' : 'Grab'}
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</div>
