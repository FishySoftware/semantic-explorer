<script lang="ts">
	import { onMount } from 'svelte';
	import './app.css';
	import Sidebar from './lib/Sidebar.svelte';
	import TopBanner from './lib/TopBanner.svelte';
	import ToastHost from './lib/components/ToastHost.svelte';
	import Chat from './lib/pages/Chat.svelte';
	import CollectionDetail from './lib/pages/CollectionDetail.svelte';
	import CollectionTransforms from './lib/pages/CollectionTransforms.svelte';
	import Collections from './lib/pages/Collections.svelte';
	import Dashboard from './lib/pages/Dashboard.svelte';
	import DatasetDetail from './lib/pages/DatasetDetail.svelte';
	import DatasetTransforms from './lib/pages/DatasetTransforms.svelte';
	import Datasets from './lib/pages/Datasets.svelte';
	import Documentation from './lib/pages/Documentation.svelte';
	import EmbeddedDatasets from './lib/pages/EmbeddedDatasets.svelte';
	import Embedders from './lib/pages/Embedders.svelte';
	import LLMs from './lib/pages/LLMs.svelte';
	import Marketplace from './lib/pages/Marketplace.svelte';
	import Search from './lib/pages/Search.svelte';
	import VisualizationTransforms from './lib/pages/VisualizationTransforms.svelte';
	import Visualizations from './lib/pages/Visualizations.svelte';

	let activeUrl = $state('/dashboard');
	let selectedCollectionId = $state<number | null>(null);
	let selectedDatasetId = $state<number | null>(null);
	let selectedVisualizationId = $state<number | null>(null);

	function parseRoute(hash: string): { path: string; params: Record<string, string> } {
		const hashWithoutQuery = hash.split('?')[0];
		const parts = hashWithoutQuery
			.slice(1)
			.split('/')
			.filter((p) => p);

		if (parts.length === 0) return { path: '/dashboard', params: {} };

		if (parts.length === 3 && parts[0] === 'collections' && parts[2] === 'details') {
			return { path: '/collections/detail', params: { id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'datasets' && parts[2] === 'details') {
			return { path: '/datasets/detail', params: { id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'visualizations' && parts[2] === 'details') {
			return { path: '/visualizations/detail', params: { id: parts[1] } };
		}

		const result = { path: '/' + parts.join('/'), params: {} };
		return result;
	}

	function handleHashChange() {
		const { path, params } = parseRoute(window.location.hash);
		activeUrl = path;

		if (path === '/collections/detail' && params.id) {
			selectedCollectionId = parseInt(params.id, 10);
		} else if (path !== '/collections/detail') {
			selectedCollectionId = null;
		}

		if (path === '/datasets/detail' && params.id) {
			selectedDatasetId = parseInt(params.id, 10);
		} else if (path !== '/datasets/detail') {
			selectedDatasetId = null;
		}

		if (path === '/visualizations/detail' && params.id) {
			selectedVisualizationId = parseInt(params.id, 10);
		} else if (path !== '/visualizations/detail') {
			selectedVisualizationId = null;
		}
	}

	onMount(() => {
		handleHashChange();
		window.addEventListener('hashchange', handleHashChange);
		return () => {
			window.removeEventListener('hashchange', handleHashChange);
		};
	});

	function viewCollection(collectionId: number) {
		selectedCollectionId = collectionId;
		activeUrl = '/collections/detail';
		window.location.hash = `/collections/${collectionId}/details`;
	}

	function backToCollections() {
		selectedCollectionId = null;
		activeUrl = '/collections';
		window.location.hash = '/collections';
	}

	function viewDataset(datasetId: number) {
		selectedDatasetId = datasetId;
		activeUrl = '/datasets/detail';
		window.location.hash = `/datasets/${datasetId}/details`;
	}

	function backToDatasets() {
		selectedDatasetId = null;
		activeUrl = '/datasets';
		window.location.hash = '/datasets';
	}

	function viewVisualization(transformId: number) {
		selectedVisualizationId = transformId;
		activeUrl = '/visualizations/detail';
		window.location.hash = `/visualizations/${transformId}/details`;
	}

	function backToVisualizations() {
		selectedVisualizationId = null;
		activeUrl = '/visualizations';
		window.location.hash = '/visualizations';
	}

	function navigate(path: string) {
		window.location.hash = path;
	}
</script>

<div class="h-screen flex flex-col bg-gray-50 dark:bg-gray-900">
	<TopBanner />
	<ToastHost />
	<div class="flex flex-1 overflow-hidden">
		<Sidebar bind:activeUrl />

		<main class="flex-1 overflow-y-auto {activeUrl === '/chat' ? 'p-0' : 'p-8'}">
			{#if activeUrl === '/dashboard'}
				<Dashboard />
			{:else if activeUrl === '/chat'}
				<Chat />
			{:else if activeUrl === '/documentation'}
				<Documentation />
			{:else if activeUrl === '/collections'}
				<Collections onViewCollection={viewCollection} onNavigate={navigate} />
			{:else if activeUrl === '/collections/detail'}
				{#if selectedCollectionId !== null}
					<CollectionDetail collectionId={selectedCollectionId} onBack={backToCollections} />
				{/if}
			{:else if activeUrl === '/datasets'}
				<Datasets onViewDataset={viewDataset} onNavigate={navigate} />
			{:else if activeUrl === '/datasets/detail'}
				{#if selectedDatasetId !== null}
					<DatasetDetail datasetId={selectedDatasetId} onBack={backToDatasets} />
				{/if}
			{:else if activeUrl === '/embedders'}
				<Embedders />
			{:else if activeUrl === '/llms'}
				<LLMs />
			{:else if activeUrl === '/collection-transforms'}
				<CollectionTransforms />
			{:else if activeUrl === '/dataset-transforms'}
				<DatasetTransforms />
			{:else if activeUrl === '/embedded-datasets'}
				<EmbeddedDatasets onViewDataset={viewDataset} onNavigate={navigate} />
			{:else if activeUrl === '/visualization-transforms'}
				<VisualizationTransforms />
			{:else if activeUrl === '/visualizations'}
				<Visualizations onViewVisualization={viewVisualization} />
			{:else if activeUrl === '/visualizations/detail'}
				{#if selectedVisualizationId !== null}
					{#await import('./lib/pages/VisualizationDetail.svelte') then { default: VisualizationDetail }}
						<VisualizationDetail
							transformId={selectedVisualizationId}
							onBack={backToVisualizations}
						/>
					{/await}
				{/if}
			{:else if activeUrl === '/search'}
				<Search />
			{:else if activeUrl === '/marketplace'}
				<Marketplace />
			{/if}
		</main>
	</div>
</div>
