<script lang="ts">
	import { onMount } from 'svelte';
	import './app.css';
	import Sidebar from './lib/Sidebar.svelte';
	import TopBanner from './lib/TopBanner.svelte';
	import ErrorBoundary from './lib/components/ErrorBoundary.svelte';
	import ToastHost from './lib/components/ToastHost.svelte';
	import { initializeTheme } from './lib/utils/theme';

	// Dynamic imports for code-splitting
	const Chat = () => import('./lib/pages/Chat.svelte');
	const CollectionDetail = () => import('./lib/pages/CollectionDetail.svelte');
	const CollectionTransforms = () => import('./lib/pages/CollectionTransforms.svelte');
	const CollectionTransformDetail = () => import('./lib/pages/CollectionTransformDetail.svelte');
	const Collections = () => import('./lib/pages/Collections.svelte');
	const Dashboard = () => import('./lib/pages/Dashboard.svelte');
	const DatasetDetail = () => import('./lib/pages/DatasetDetail.svelte');
	const DatasetTransforms = () => import('./lib/pages/DatasetTransforms.svelte');
	const DatasetTransformDetail = () => import('./lib/pages/DatasetTransformDetail.svelte');
	const Datasets = () => import('./lib/pages/Datasets.svelte');
	const Documentation = () => import('./lib/pages/Documentation.svelte');
	const EmbeddedDatasets = () => import('./lib/pages/EmbeddedDatasets.svelte');
	const EmbeddedDatasetDetail = () => import('./lib/pages/EmbeddedDatasetDetail.svelte');
	const EmbedderDetail = () => import('./lib/pages/EmbedderDetail.svelte');
	const Embedders = () => import('./lib/pages/Embedders.svelte');
	const GrabResource = () => import('./lib/pages/GrabResource.svelte');
	const LLMs = () => import('./lib/pages/LLMs.svelte');
	const Marketplace = () => import('./lib/pages/Marketplace.svelte');
	const Search = () => import('./lib/pages/Search.svelte');
	const VisualizationTransforms = () => import('./lib/pages/VisualizationTransforms.svelte');
	const VisualizationTransformDetail = () =>
		import('./lib/pages/VisualizationTransformDetail.svelte');
	const Visualizations = () => import('./lib/pages/Visualizations.svelte');
	const VisualizationDetail = () => import('./lib/pages/VisualizationDetail.svelte');

	let activeUrl = $state('/dashboard');
	let selectedCollectionId = $state<number | null>(null);
	let selectedDatasetId = $state<number | null>(null);
	let selectedEmbeddedDatasetId = $state<number | null>(null);
	let selectedEmbedderId = $state<number | null>(null);
	let selectedVisualizationId = $state<number | null>(null);
	let selectedCollectionTransformId = $state<number | null>(null);
	let selectedDatasetTransformId = $state<number | null>(null);
	let selectedVisualizationTransformId = $state<number | null>(null);
	let grabResourceType = $state<'collections' | 'datasets' | 'embedders' | 'llms' | null>(null);
	let grabResourceId = $state<number | null>(null);

	function parseRoute(hash: string): { path: string; params: Record<string, string> } {
		const [hashWithoutQuery, queryString] = hash.split('?');
		const parts = hashWithoutQuery
			.slice(1)
			.split('/')
			.filter((p) => p);

		const params: Record<string, string> = {};

		// Parse query string if present
		if (queryString) {
			const searchParams = new URLSearchParams(queryString);
			searchParams.forEach((value, key) => {
				params[key] = value;
			});
		}

		if (parts.length === 0) return { path: '/dashboard', params };

		if (parts.length === 3 && parts[0] === 'collections' && parts[2] === 'details') {
			return { path: '/collections/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'datasets' && parts[2] === 'details') {
			return { path: '/datasets/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'embedded-datasets' && parts[2] === 'details') {
			return { path: '/embedded-datasets/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'visualizations' && parts[2] === 'details') {
			return { path: '/visualizations/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'embedders' && parts[2] === 'details') {
			return { path: '/embedders/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'collection-transforms' && parts[2] === 'details') {
			return { path: '/collection-transforms/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'dataset-transforms' && parts[2] === 'details') {
			return { path: '/dataset-transforms/detail', params: { ...params, id: parts[1] } };
		}
		if (parts.length === 3 && parts[0] === 'visualization-transforms' && parts[2] === 'details') {
			return { path: '/visualization-transforms/detail', params: { ...params, id: parts[1] } };
		}

		if (parts.length === 4 && parts[0] === 'marketplace' && parts[3] === 'grab') {
			const resourceType = parts[1] as 'collections' | 'datasets' | 'embedders' | 'llms';
			return {
				path: `/marketplace/${resourceType}/grab`,
				params: { ...params, resourceType, id: parts[2] },
			};
		}

		const result = { path: '/' + parts.join('/'), params };
		return result;
	}

	function handleHashChange() {
		const { path, params } = parseRoute(window.location.hash);
		activeUrl = path;

		if (path === '/collections/detail' && params.id) {
			const id = parseInt(params.id, 10);
			selectedCollectionId = !isNaN(id) ? id : null;
		} else if (path !== '/collections/detail') {
			selectedCollectionId = null;
		}

		if (path === '/datasets/detail' && params.id) {
			const id = parseInt(params.id, 10);
			selectedDatasetId = !isNaN(id) ? id : null;
		} else if (path !== '/datasets/detail') {
			selectedDatasetId = null;
		}

		if (path === '/embedded-datasets/detail' && params.id) {
			selectedEmbeddedDatasetId = parseInt(params.id, 10);
		} else if (path !== '/embedded-datasets/detail') {
			selectedEmbeddedDatasetId = null;
		}

		if (path === '/visualizations/detail' && params.id) {
			selectedVisualizationId = parseInt(params.id, 10);
		} else if (path !== '/visualizations/detail') {
			selectedVisualizationId = null;
		}

		if (path === '/embedders/detail' && params.id) {
			selectedEmbedderId = parseInt(params.id, 10);
		} else if (path !== '/embedders/detail') {
			selectedEmbedderId = null;
		}

		if (path === '/collection-transforms/detail' && params.id) {
			selectedCollectionTransformId = parseInt(params.id, 10);
		} else if (path !== '/collection-transforms/detail') {
			selectedCollectionTransformId = null;
		}

		if (path === '/dataset-transforms/detail' && params.id) {
			selectedDatasetTransformId = parseInt(params.id, 10);
		} else if (path !== '/dataset-transforms/detail') {
			selectedDatasetTransformId = null;
		}

		if (path === '/visualization-transforms/detail' && params.id) {
			selectedVisualizationTransformId = parseInt(params.id, 10);
		} else if (path !== '/visualization-transforms/detail') {
			selectedVisualizationTransformId = null;
		}

		if (path.includes('/marketplace/') && path.includes('/grab')) {
			if (params.resourceType && params.id) {
				grabResourceType = params.resourceType as 'collections' | 'datasets' | 'embedders' | 'llms';
				grabResourceId = parseInt(params.id, 10);
			}
		} else {
			grabResourceType = null;
			grabResourceId = null;
		}
	}

	onMount(() => {
		initializeTheme();
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

	function backToEmbeddedDatasets() {
		selectedEmbeddedDatasetId = null;
		activeUrl = '/embedded-datasets';
		window.location.hash = '#/embedded-datasets';
	}

	function viewEmbeddedDataset(embeddedDatasetId: number) {
		selectedEmbeddedDatasetId = embeddedDatasetId;
		activeUrl = '/embedded-datasets/detail';
		window.location.hash = `/embedded-datasets/${embeddedDatasetId}/details`;
	}

	function viewVisualization(transformId: number) {
		// Handle visualization navigation
		selectedVisualizationId = transformId;
		activeUrl = '/visualizations/detail';
		window.location.hash = `/visualizations/${transformId}/details`;
	}

	function backToVisualizations() {
		selectedVisualizationId = null;
		activeUrl = '/visualizations';
		window.location.hash = '/visualizations';
	}

	function viewEmbedder(embedderId: number) {
		selectedEmbedderId = embedderId;
		activeUrl = '/embedders/detail';
		window.location.hash = `/embedders/${embedderId}/details`;
	}

	function backToEmbedders() {
		selectedEmbedderId = null;
		activeUrl = '/embedders';
		window.location.hash = '/embedders';
	}

	function viewCollectionTransform(collectionTransformId: number) {
		selectedCollectionTransformId = collectionTransformId;
		activeUrl = '/collection-transforms/detail';
		window.location.hash = `/collection-transforms/${collectionTransformId}/details`;
	}

	function backToCollectionTransforms() {
		selectedCollectionTransformId = null;
		activeUrl = '/collection-transforms';
		window.location.hash = '/collection-transforms';
	}

	function viewDatasetTransform(datasetTransformId: number) {
		selectedDatasetTransformId = datasetTransformId;
		activeUrl = '/dataset-transforms/detail';
		window.location.hash = `/dataset-transforms/${datasetTransformId}/details`;
	}

	function backToDatasetTransforms() {
		selectedDatasetTransformId = null;
		activeUrl = '/dataset-transforms';
		window.location.hash = '/dataset-transforms';
	}

	function viewVisualizationTransform(visualizationTransformId: number) {
		selectedVisualizationTransformId = visualizationTransformId;
		activeUrl = '/visualization-transforms/detail';
		window.location.hash = `/visualization-transforms/${visualizationTransformId}/details`;
	}

	function backToVisualizationTransforms() {
		selectedVisualizationTransformId = null;
		activeUrl = '/visualization-transforms';
		window.location.hash = '/visualization-transforms';
	}

	function navigate(path: string) {
		window.location.hash = path;
	}

	function navigateToResource(page: string, params?: Record<string, unknown>) {
		switch (page) {
			case 'collection-detail':
				if (params?.collectionId) viewCollection(params.collectionId as number);
				break;
			case 'dataset-detail':
				if (params?.datasetId) viewDataset(params.datasetId as number);
				break;
			case 'embedded-dataset-detail':
				if (params?.embeddedDatasetId) viewEmbeddedDataset(params.embeddedDatasetId as number);
				break;
			case 'embedder-detail':
				if (params?.embedderId) viewEmbedder(params.embedderId as number);
				break;
			case 'collection-transform-detail':
				if (params?.collectionTransformId)
					viewCollectionTransform(params.collectionTransformId as number);
				break;
			case 'dataset-transform-detail':
				if (params?.datasetTransformId) viewDatasetTransform(params.datasetTransformId as number);
				break;
			case 'visualization-transform-detail':
				if (params?.visualizationTransformId)
					viewVisualizationTransform(params.visualizationTransformId as number);
				break;
			default:
				console.warn(`Unknown navigation target: ${page}`);
		}
	}
</script>

<div class="h-screen flex flex-col bg-gray-50 dark:bg-gray-900">
	<TopBanner />
	<ToastHost />
	<div class="flex flex-1 overflow-hidden">
		<Sidebar bind:activeUrl />

		<main
			class="flex-1 overflow-y-auto {activeUrl === '/chat'
				? 'p-0'
				: 'p-3 sm:p-4 lg:p-6'} min-content-height"
		>
			<ErrorBoundary>
				{#if activeUrl === '/dashboard'}
					{#await Dashboard() then { default: DashboardComponent }}
						<DashboardComponent />
					{/await}
				{:else if activeUrl === '/chat'}
					{#await Chat() then { default: ChatComponent }}
						<ChatComponent />
					{/await}
				{:else if activeUrl === '/documentation'}
					{#await Documentation() then { default: DocumentationComponent }}
						<DocumentationComponent />
					{/await}
				{:else if activeUrl === '/collections'}
					{#await Collections() then { default: CollectionsComponent }}
						<CollectionsComponent onViewCollection={viewCollection} />
					{/await}
				{:else if activeUrl === '/collections/detail'}
					{#if selectedCollectionId !== null}
						{#await CollectionDetail() then { default: CollectionDetailComponent }}
							<CollectionDetailComponent
								collectionId={selectedCollectionId}
								onBack={backToCollections}
							/>
						{/await}
					{/if}
				{:else if activeUrl === '/datasets'}
					{#await Datasets() then { default: DatasetsComponent }}
						<DatasetsComponent onViewDataset={viewDataset} />
					{/await}
				{:else if activeUrl === '/datasets/detail'}
					{#if selectedDatasetId !== null}
						{#await DatasetDetail() then { default: DatasetDetailComponent }}
							<DatasetDetailComponent datasetId={selectedDatasetId} onBack={backToDatasets} />
						{/await}
					{/if}
				{:else if activeUrl === '/embedders'}
					{#await Embedders() then { default: EmbeddersComponent }}
						<EmbeddersComponent onViewEmbedder={viewEmbedder} />
					{/await}
				{:else if activeUrl === '/embedders/detail'}
					{#if selectedEmbedderId !== null}
						{#await EmbedderDetail() then { default: EmbedderDetailComponent }}
							<EmbedderDetailComponent embedderId={selectedEmbedderId} onBack={backToEmbedders} />
						{/await}
					{/if}
				{:else if activeUrl === '/llms'}
					{#await LLMs() then { default: LLMsComponent }}
						<LLMsComponent />
					{/await}
				{:else if activeUrl === '/collection-transforms'}
					{#await CollectionTransforms() then { default: CollectionTransformsComponent }}
						<CollectionTransformsComponent onViewTransform={viewCollectionTransform} />
					{/await}
				{:else if activeUrl === '/collection-transforms/detail'}
					{#if selectedCollectionTransformId !== null}
						{#await CollectionTransformDetail() then { default: CollectionTransformDetailComponent }}
							<CollectionTransformDetailComponent
								collectionTransformId={selectedCollectionTransformId}
								onBack={backToCollectionTransforms}
								onNavigate={navigateToResource}
							/>
						{/await}
					{/if}
				{:else if activeUrl === '/dataset-transforms'}
					{#await DatasetTransforms() then { default: DatasetTransformsComponent }}
						<DatasetTransformsComponent onViewTransform={viewDatasetTransform} />
					{/await}
				{:else if activeUrl === '/dataset-transforms/detail'}
					{#if selectedDatasetTransformId !== null}
						{#await DatasetTransformDetail() then { default: DatasetTransformDetailComponent }}
							<DatasetTransformDetailComponent
								datasetTransformId={selectedDatasetTransformId}
								onBack={backToDatasetTransforms}
								onNavigate={navigateToResource}
							/>
						{/await}
					{/if}
				{:else if activeUrl === '/embedded-datasets'}
					{#await EmbeddedDatasets() then { default: EmbeddedDatasetsComponent }}
						<EmbeddedDatasetsComponent onViewDataset={viewDataset} onNavigate={navigate} />
					{/await}
				{:else if activeUrl === '/embedded-datasets/detail'}
					{#if selectedEmbeddedDatasetId !== null}
						{#await EmbeddedDatasetDetail() then { default: EmbeddedDatasetDetailComponent }}
							<EmbeddedDatasetDetailComponent
								embeddedDatasetId={selectedEmbeddedDatasetId}
								onBack={backToEmbeddedDatasets}
							/>
						{/await}
					{/if}
				{:else if activeUrl === '/visualization-transforms'}
					{#await VisualizationTransforms() then { default: VisualizationTransformsComponent }}
						<VisualizationTransformsComponent onViewTransform={viewVisualizationTransform} />
					{/await}
				{:else if activeUrl === '/visualization-transforms/detail'}
					{#if selectedVisualizationTransformId !== null}
						{#await VisualizationTransformDetail() then { default: VisualizationTransformDetailComponent }}
							<VisualizationTransformDetailComponent
								visualizationTransformId={selectedVisualizationTransformId}
								onBack={backToVisualizationTransforms}
								onNavigate={navigateToResource}
							/>
						{/await}
					{/if}
				{:else if activeUrl === '/visualizations'}
					{#await Visualizations() then { default: VisualizationsComponent }}
						<VisualizationsComponent onViewVisualization={viewVisualization} />
					{/await}
				{:else if activeUrl === '/visualizations/detail'}
					{#if selectedVisualizationId !== null}
						{#await VisualizationDetail() then { default: VisualizationDetailComponent }}
							<VisualizationDetailComponent
								visualizationTransformId={selectedVisualizationId}
								onBack={backToVisualizations}
							/>
						{/await}
					{/if}
				{:else if activeUrl === '/search'}
					{#await Search() then { default: SearchComponent }}
						<SearchComponent onViewDataset={viewDataset} onViewEmbedder={viewEmbedder} />
					{/await}
				{:else if activeUrl === '/marketplace'}
					{#await Marketplace() then { default: MarketplaceComponent }}
						<MarketplaceComponent />
					{/await}
				{:else if activeUrl.includes('/marketplace/') && activeUrl.includes('/grab')}
					{#if grabResourceType && grabResourceId !== null}
						{#await GrabResource() then { default: GrabResourceComponent }}
							<GrabResourceComponent resourceType={grabResourceType} resourceId={grabResourceId} />
						{/await}
					{/if}
				{/if}
			</ErrorBoundary>
		</main>
	</div>
</div>
