<script lang="ts">
	import { onMount } from 'svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

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

			toastStore.success('Collection grabbed successfully!');
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

			toastStore.success('Dataset grabbed successfully!');
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

			toastStore.success('Embedder grabbed successfully!');
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

			toastStore.success('LLM grabbed successfully!');
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

<div class="marketplace-container">
	<PageHeader
		title="Marketplace"
		description="Discover and grab public resources from the community"
	/>

	<div class="tabs">
		<button
			class="tab"
			class:active={activeTab === 'collections'}
			onclick={() => (activeTab = 'collections')}
		>
			Collections ({collections.length})
		</button>
		<button
			class="tab"
			class:active={activeTab === 'datasets'}
			onclick={() => (activeTab = 'datasets')}
		>
			Datasets ({datasets.length})
		</button>
		<button
			class="tab"
			class:active={activeTab === 'embedders'}
			onclick={() => (activeTab = 'embedders')}
		>
			Embedders ({embedders.length})
		</button>
		<button class="tab" class:active={activeTab === 'llms'} onclick={() => (activeTab = 'llms')}>
			LLMs ({llms.length})
		</button>
	</div>

	{#if loading}
		<div class="loading">Loading marketplace resources...</div>
	{:else if error}
		<div class="error">{error}</div>
	{:else}
		<div class="content">
			{#if activeTab === 'collections'}
				<div class="resource-grid">
					{#each collections as collection (collection.collection_id)}
						<div class="resource-card">
							<div class="card-header">
								<h3>{collection.title}</h3>
								<span class="owner">by {collection.owner}</span>
							</div>
							{#if collection.details}
								<p class="details">{collection.details}</p>
							{/if}
							{#if collection.tags.length > 0}
								<div class="tags">
									{#each collection.tags as tag (tag)}
										<span class="tag">{tag}</span>
									{/each}
								</div>
							{/if}
							<div class="card-footer">
								<span class="date"
									>{collection.created_at
										? new Date(collection.created_at).toLocaleDateString()
										: ''}</span
								>
								<button
									class="btn-grab"
									onclick={() => grabCollection(collection.collection_id)}
									disabled={grabbingId === collection.collection_id}
								>
									{grabbingId === collection.collection_id ? 'Grabbing...' : 'Grab'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{:else if activeTab === 'datasets'}
				<div class="resource-grid">
					{#each datasets as dataset (dataset.dataset_id)}
						<div class="resource-card">
							<div class="card-header">
								<h3>{dataset.title}</h3>
								<span class="owner">by {dataset.owner}</span>
							</div>
							{#if dataset.details}
								<p class="details">{dataset.details}</p>
							{/if}
							{#if dataset.tags.length > 0}
								<div class="tags">
									{#each dataset.tags as tag (tag)}
										<span class="tag">{tag}</span>
									{/each}
								</div>
							{/if}
							<div class="card-footer">
								<span class="date"
									>{dataset.created_at
										? new Date(dataset.created_at).toLocaleDateString()
										: ''}</span
								>
								<button
									class="btn-grab"
									onclick={() => grabDataset(dataset.dataset_id)}
									disabled={grabbingId === dataset.dataset_id}
								>
									{grabbingId === dataset.dataset_id ? 'Grabbing...' : 'Grab'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{:else if activeTab === 'embedders'}
				<div class="resource-grid">
					{#each embedders as embedder (embedder.embedder_id)}
						<div class="resource-card">
							<div class="card-header">
								<h3>{embedder.name}</h3>
								<span class="owner">by {embedder.owner}</span>
							</div>
							<div class="embedder-info">
								<div class="info-row">
									<span class="label">Provider:</span>
									<span class="value">{embedder.provider}</span>
								</div>
								<div class="info-row">
									<span class="label">Dimensions:</span>
									<span class="value">{embedder.dimensions}</span>
								</div>
								<div class="info-row">
									<span class="label">Base URL:</span>
									<span class="value url">{embedder.base_url}</span>
								</div>
							</div>
							<div class="card-footer">
								<span class="date"
									>{embedder.created_at
										? new Date(embedder.created_at).toLocaleDateString()
										: ''}</span
								>
								<button
									class="btn-grab"
									onclick={() => grabEmbedder(embedder.embedder_id)}
									disabled={grabbingId === embedder.embedder_id}
								>
									{grabbingId === embedder.embedder_id ? 'Grabbing...' : 'Grab'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{:else if activeTab === 'llms'}
				<div class="resource-grid">
					{#each llms as llm (llm.llm_id)}
						<div class="resource-card">
							<div class="card-header">
								<h3>{llm.name}</h3>
								<span class="owner">by {llm.owner}</span>
							</div>
							<div class="embedder-info">
								<div class="info-row">
									<span class="label">Provider:</span>
									<span class="value">{llm.provider}</span>
								</div>
								<div class="info-row">
									<span class="label">Base URL:</span>
									<span class="value url">{llm.base_url}</span>
								</div>
							</div>
							<div class="card-footer">
								<span class="date"
									>{llm.created_at ? new Date(llm.created_at).toLocaleDateString() : ''}</span
								>
								<button
									class="btn-grab"
									onclick={() => grabLLM(llm.llm_id)}
									disabled={grabbingId === llm.llm_id}
								>
									{grabbingId === llm.llm_id ? 'Grabbing...' : 'Grab'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.marketplace-container {
		padding: 2rem;
		max-width: 1400px;
		margin: 0 auto;
	}

	.tabs {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 2rem;
		border-bottom: 2px solid #e5e7eb;
	}

	.tab {
		padding: 0.75rem 1.5rem;
		background: none;
		border: none;
		border-bottom: 3px solid transparent;
		cursor: pointer;
		font-size: 1rem;
		font-weight: 500;
		color: #6b7280;
		transition: all 0.2s;
	}

	.tab:hover {
		color: #374151;
		background: #f9fafb;
	}

	.tab.active {
		color: #3b82f6;
		border-bottom-color: #3b82f6;
	}

	.loading,
	.error {
		text-align: center;
		padding: 3rem;
		font-size: 1.1rem;
	}

	.error {
		color: #dc2626;
	}

	.resource-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
		gap: 1.5rem;
	}

	.resource-card {
		background: white;
		border: 1px solid #e5e7eb;
		border-radius: 0.5rem;
		padding: 1.5rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		transition: box-shadow 0.2s;
	}

	.resource-card:hover {
		box-shadow:
			0 4px 6px -1px rgba(0, 0, 0, 0.1),
			0 2px 4px -1px rgba(0, 0, 0, 0.06);
	}

	.card-header {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.card-header h3 {
		margin: 0;
		font-size: 1.25rem;
		color: #111827;
	}

	.owner {
		font-size: 0.875rem;
		color: #6b7280;
	}

	.details {
		margin: 0;
		color: #4b5563;
		line-height: 1.5;
		font-size: 0.9rem;
	}

	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.tag {
		padding: 0.25rem 0.75rem;
		background: #dbeafe;
		color: #1e40af;
		border-radius: 9999px;
		font-size: 0.75rem;
		font-weight: 500;
	}

	.embedder-info {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.info-row {
		display: flex;
		gap: 0.5rem;
		font-size: 0.875rem;
	}

	.info-row .label {
		font-weight: 600;
		color: #6b7280;
		min-width: 90px;
	}

	.info-row .value {
		color: #374151;
	}

	.info-row .value.url {
		font-family: monospace;
		font-size: 0.8rem;
		word-break: break-all;
	}

	.card-footer {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-top: auto;
		padding-top: 1rem;
		border-top: 1px solid #e5e7eb;
	}

	.date {
		font-size: 0.75rem;
		color: #9ca3af;
	}

	.btn-grab {
		padding: 0.5rem 1.25rem;
		background: #3b82f6;
		color: white;
		border: none;
		border-radius: 0.375rem;
		cursor: pointer;
		font-weight: 500;
		transition: background 0.2s;
	}

	.btn-grab:hover:not(:disabled) {
		background: #2563eb;
	}

	.btn-grab:disabled {
		background: #9ca3af;
		cursor: not-allowed;
	}
</style>
