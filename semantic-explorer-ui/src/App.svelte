<script lang="ts">
	import { onMount } from 'svelte';
	import './app.css';
	import Sidebar from './lib/Sidebar.svelte';
	import TopBanner from './lib/TopBanner.svelte';
	import ToastHost from './lib/components/ToastHost.svelte';
	import CollectionDetail from './lib/pages/CollectionDetail.svelte';
	import Collections from './lib/pages/Collections.svelte';
	import Dashboard from './lib/pages/Dashboard.svelte';
	import DatasetDetail from './lib/pages/DatasetDetail.svelte';
	import Datasets from './lib/pages/Datasets.svelte';
	import Documentation from './lib/pages/Documentation.svelte';
	import Embedders from './lib/pages/Embedders.svelte';
	import Search from './lib/pages/Search.svelte';
	import Transforms from './lib/pages/Transforms.svelte';
	import Visualizations from './lib/pages/Visualizations.svelte';

	let activeUrl = $state('/dashboard');
	let selectedCollectionId = $state<number | null>(null);
	let selectedDatasetId = $state<number | null>(null);
	let selectedTransformId = $state<number | null>(null);

	function parseRoute(hash: string): { path: string; params: Record<string, string> } {
		const parts = hash
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
		if (parts.length === 3 && parts[0] === 'transforms' && parts[2] === 'details') {
			return { path: '/transforms/detail', params: { id: parts[1] } };
		}

		return { path: '/' + parts.join('/'), params: {} };
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

		if (path === '/transforms/detail' && params.id) {
			selectedTransformId = parseInt(params.id, 10);
		} else if (path !== '/transforms/detail') {
			selectedTransformId = null;
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

	function viewTransform(transformId: number) {
		selectedTransformId = transformId;
		activeUrl = '/transforms/detail';
		window.location.hash = `/transforms/${transformId}/details`;
	}

	function backToTransforms() {
		selectedTransformId = null;
		activeUrl = '/transforms';
		window.location.hash = '/transforms';
	}
</script>

<div class="h-screen flex flex-col bg-gray-50 dark:bg-gray-900">
	<TopBanner />
	<ToastHost />
	<div class="flex flex-1 overflow-hidden">
		<Sidebar bind:activeUrl />

		<main class="flex-1 overflow-y-auto p-8">
			{#if activeUrl === '/dashboard'}
				<Dashboard />
			{:else if activeUrl === '/documentation'}
				<Documentation />
			{:else if activeUrl === '/collections'}
				<Collections onViewCollection={viewCollection} />
			{:else if activeUrl === '/collections/detail'}
				{#if selectedCollectionId !== null}
					<CollectionDetail collectionId={selectedCollectionId} onBack={backToCollections} />
				{/if}
			{:else if activeUrl === '/datasets'}
				<Datasets onViewDataset={viewDataset} />
			{:else if activeUrl === '/datasets/detail'}
				{#if selectedDatasetId !== null}
					<DatasetDetail datasetId={selectedDatasetId} onBack={backToDatasets} />
				{/if}
			{:else if activeUrl === '/embedders'}
				<Embedders />
			{:else if activeUrl === '/transforms'}
				<Transforms onViewTransform={viewTransform} />
			{:else if activeUrl === '/transforms/detail'}
				{#if selectedTransformId !== null}
					{#await import('./lib/pages/TransformDetail.svelte') then mod}
						<mod.default transformId={selectedTransformId} onBack={backToTransforms} />
					{/await}
				{/if}
			{:else if activeUrl === '/visualizations'}
				<Visualizations />
			{:else if activeUrl === '/search'}
				<Search />
			{/if}
		</main>
	</div>
</div>
