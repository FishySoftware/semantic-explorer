<script lang="ts">
	import { onMount } from 'svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import type {
		Collection,
		Dataset,
		EmbeddedDataset,
		Embedder,
		LLM,
		Visualization,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatRelativeTime } from '../utils/ui-helpers';

	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let visualizations = $state<Visualization[]>([]);
	let publicCollections = $state<Collection[]>([]);
	let publicDatasets = $state<Dataset[]>([]);
	let publicEmbedders = $state<Embedder[]>([]);
	let publicLLMs = $state<LLM[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let grabbingId = $state<number | null>(null);

	onMount(async () => {
		await fetchData();
	});

	async function fetchData() {
		try {
			loading = true;
			error = null;

			const [
				collectionsRes,
				datasetsRes,
				embeddedDatasetsRes,
				visualizationsRes,
				publicCollectionsRes,
				publicDatasetsRes,
				publicEmbeddersRes,
				publicLLMsRes,
			] = await Promise.all([
				fetch('/api/collections'),
				fetch('/api/datasets'),
				fetch('/api/embedded-datasets'),
				fetch('/api/visualizations/recent?limit=5'),
				fetch('/api/marketplace/collections/recent?limit=5'),
				fetch('/api/marketplace/datasets/recent?limit=5'),
				fetch('/api/marketplace/embedders/recent?limit=5'),
				fetch('/api/marketplace/llms/recent?limit=5'),
			]);

			if (
				!collectionsRes.ok ||
				!datasetsRes.ok ||
				!embeddedDatasetsRes.ok ||
				!visualizationsRes.ok ||
				!publicCollectionsRes.ok ||
				!publicDatasetsRes.ok ||
				!publicEmbeddersRes.ok ||
				!publicLLMsRes.ok
			) {
				throw new Error('Failed to fetch data');
			}

			const collectionsData = await collectionsRes.json();
			const datasetsData = await datasetsRes.json();
			const embeddedDatasetsData = await embeddedDatasetsRes.json();
			const allVisualizations: Visualization[] = await visualizationsRes.json();
			publicCollections = await publicCollectionsRes.json();
			publicDatasets = await publicDatasetsRes.json();
			publicEmbedders = await publicEmbeddersRes.json();
			publicLLMs = await publicLLMsRes.json();

			// Extract arrays from paginated responses
			const allCollections = collectionsData.collections || collectionsData;
			const allDatasets = datasetsData.items || datasetsData;
			const allEmbeddedDatasets = embeddedDatasetsData.embedded_datasets || embeddedDatasetsData;

			collections = allCollections
				.sort(
					(a: Collection, b: Collection) =>
						new Date(b.updated_at ?? 0).getTime() - new Date(a.updated_at ?? 0).getTime()
				)
				.slice(0, 5);

			datasets = allDatasets
				.sort(
					(a: Dataset, b: Dataset) =>
						new Date(b.updated_at ?? 0).getTime() - new Date(a.updated_at ?? 0).getTime()
				)
				.slice(0, 5);

			embeddedDatasets = allEmbeddedDatasets
				.sort(
					(a: EmbeddedDataset, b: EmbeddedDataset) =>
						new Date(b.updated_at ?? 0).getTime() - new Date(a.updated_at ?? 0).getTime()
				)
				.slice(0, 5);

			visualizations = allVisualizations.slice(0, 5);
		} catch (e) {
			const message = formatError(e, 'Failed to load dashboard data');
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
</script>

<div class="max-w-full xl:max-w-7xl mx-auto">
	<div class="mb-3 lg:mb-6">
		<h1 class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mb-1 lg:mb-2">
			Dashboard
		</h1>
		<p class="text-sm lg:text-base text-gray-600 dark:text-gray-400">
			Welcome to Semantic Explorer - Your document processing and embedding platform
		</p>
	</div>

	<div class="mt-3 mb-3 lg:mt-4 lg:mb-4 grid grid-cols-2 md:grid-cols-4 gap-2 lg:gap-4">
		<a
			href="#/collections"
			class="compact-p bg-linear-to-br from-blue-500 to-blue-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
		>
			<svg
				class="w-6 h-6 lg:w-8 lg:h-8 mb-1 lg:mb-2"
				fill="none"
				stroke="currentColor"
				viewBox="0 0 24 24"
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
				></path>
			</svg>
			<h3 class="text-base lg:text-lg font-semibold">Collections</h3>
			<p class="text-xs lg:text-sm text-blue-100 mt-0.5 lg:mt-1 hidden sm:block">
				Manage your document collections
			</p>
		</a>

		<a
			href="#/datasets"
			class="compact-p bg-linear-to-br from-green-500 to-green-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
		>
			<svg
				class="w-6 h-6 lg:w-8 lg:h-8 mb-1 lg:mb-2"
				fill="none"
				stroke="currentColor"
				viewBox="0 0 24 24"
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
				></path>
			</svg>
			<h3 class="text-base lg:text-lg font-semibold">Datasets</h3>
			<p class="text-xs lg:text-sm text-green-100 mt-0.5 lg:mt-1 hidden sm:block">
				View and manage your datasets
			</p>
		</a>

		<a
			href="#/embedded-datasets"
			class="compact-p bg-linear-to-br from-purple-500 to-purple-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
		>
			<svg
				class="w-6 h-6 lg:w-8 lg:h-8 mb-1 lg:mb-2"
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
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6M9 16h6"
				></path>
			</svg>
			<h3 class="text-base lg:text-lg font-semibold">Embedded</h3>
			<p class="text-xs lg:text-sm text-purple-100 mt-0.5 lg:mt-1 hidden sm:block">
				Manage embedded datasets
			</p>
		</a>

		<a
			href="#/visualization-transforms"
			class="compact-p bg-linear-to-br from-orange-500 to-orange-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
		>
			<svg
				class="w-6 h-6 lg:w-8 lg:h-8 mb-1 lg:mb-2"
				fill="none"
				stroke="currentColor"
				viewBox="0 0 24 24"
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4a1 1 0 011-1h16a1 1 0 011 1v2.757a1 1 0 01-.293.707L12 16.414l-7.414-7.414A1 1 0 013 8.343V4z"
				></path>
			</svg>
			<h3 class="text-base lg:text-lg font-semibold">Visualizations</h3>
			<p class="text-xs lg:text-sm text-orange-100 mt-0.5 lg:mt-1 hidden sm:block">
				Create visualizations
			</p>
		</a>
	</div>

	{#if loading}
		<LoadingState message="Loading dashboard..." />
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg compact-p min-content-height"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
		</div>
	{:else}
		<div class="grid grid-cols-1 lg:grid-cols-2 gap-3 lg:gap-4">
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p min-card-height">
				<div class="flex justify-between items-center mb-2 lg:mb-3">
					<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
						Recent Collections
					</h2>
					<a href="#/collections" class="text-blue-600 dark:text-blue-400 hover:underline text-sm">
						View all
					</a>
				</div>
				{#if collections.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">No collections yet</p>
				{:else}
					<div class="space-y-2 lg:space-y-3">
						{#each collections as collection (collection.collection_id)}
							<a
								href={`#/collections/${collection.collection_id}/details`}
								class="block p-2 lg:p-3 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
							>
								<div class="flex justify-between items-start">
									<div class="flex-1">
										<h3 class="font-medium text-gray-900 dark:text-white">
											{collection.title}
										</h3>
										{#if collection.details}
											<p class="text-sm text-gray-600 dark:text-gray-400 mt-1 line-clamp-1">
												{collection.details}
											</p>
										{/if}
										<div class="flex gap-2 mt-2">
											{#each collection.tags.slice(0, 3) as tag (tag)}
												<span
													class="text-xs px-2 py-0.5 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded"
												>
													#{tag}
												</span>
											{/each}
										</div>
									</div>
									<span class="text-xs text-gray-500 dark:text-gray-400 ml-2">
										{formatRelativeTime(collection.updated_at)}
									</span>
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</div>

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p min-card-height">
				<div class="flex justify-between items-center mb-2 lg:mb-3">
					<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
						Recent Datasets
					</h2>
					<a
						href="#/datasets"
						class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
					>
						View all
					</a>
				</div>
				{#if datasets.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">No datasets yet</p>
				{:else}
					<div class="space-y-2 lg:space-y-3">
						{#each datasets as dataset (dataset.dataset_id)}
							<a
								href={`#/datasets/${dataset.dataset_id}/details`}
								class="block p-2 lg:p-3 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
							>
								<div class="flex justify-between items-start">
									<div class="flex-1">
										<h3 class="font-medium text-gray-900 dark:text-white">
											{dataset.title}
										</h3>
										{#if dataset.details}
											<p class="text-sm text-gray-600 dark:text-gray-400 mt-1 line-clamp-1">
												{dataset.details}
											</p>
										{/if}
										<div class="flex gap-2 mt-2">
											{#each dataset.tags.slice(0, 3) as tag (tag)}
												<span
													class="text-xs px-2 py-0.5 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded"
												>
													#{tag}
												</span>
											{/each}
										</div>
									</div>
									<span class="text-xs text-gray-500 dark:text-gray-400 ml-2">
										{formatRelativeTime(dataset.updated_at)}
									</span>
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<div class="grid grid-cols-1 lg:grid-cols-2 gap-3 lg:gap-4 mt-3 lg:mt-4">
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p min-card-height">
				<div class="flex justify-between items-center mb-2 lg:mb-3">
					<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
						Recent Embedded Datasets
					</h2>
					<a
						href="#/embedded-datasets"
						class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
					>
						View all
					</a>
				</div>
				{#if embeddedDatasets.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">
						No embedded datasets yet
					</p>
				{:else}
					<div class="space-y-2 lg:space-y-3">
						{#each embeddedDatasets as embeddedDataset (embeddedDataset.embedded_dataset_id)}
							<a
								href={`#/embedded-datasets/${embeddedDataset.embedded_dataset_id}/details`}
								class="block p-2 lg:p-3 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
							>
								<div class="flex justify-between items-start">
									<div class="flex-1">
										<h3 class="font-medium text-gray-900 dark:text-white">
											{embeddedDataset.title}
										</h3>
										{#if embeddedDataset.source_dataset_title}
											<p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
												Dataset: {embeddedDataset.source_dataset_title}
											</p>
										{/if}
										{#if embeddedDataset.embedder_name}
											<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
												Embedder: {embeddedDataset.embedder_name}
											</p>
										{/if}
									</div>
									<span class="text-xs text-gray-500 dark:text-gray-400 ml-2">
										{formatRelativeTime(embeddedDataset.updated_at)}
									</span>
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</div>

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p min-card-height">
				<div class="flex justify-between items-center mb-2 lg:mb-3">
					<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
						Recent Visualizations
					</h2>
					<a
						href="#/visualization-transforms"
						class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
					>
						View all
					</a>
				</div>
				{#if visualizations.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">
						No visualizations yet
					</p>
				{:else}
					<div class="space-y-2 lg:space-y-3">
						{#each visualizations as visualization (visualization.visualization_id)}
							<a
								href={`#/visualizations/${visualization.visualization_transform_id}/details`}
								class="block p-2 lg:p-3 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
							>
								<div class="flex justify-between items-start">
									<div class="flex-1">
										<h3 class="font-medium text-gray-900 dark:text-white">
											Visualization #{visualization.visualization_id}
										</h3>
										<div class="flex items-center gap-2 mt-1">
											<span
												class="text-xs px-2 py-0.5 rounded {visualization.status === 'completed'
													? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
													: 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300'}"
											>
												{visualization.status}
											</span>
										</div>
									</div>
									<span class="text-xs text-gray-500 dark:text-gray-400 ml-2">
										{formatRelativeTime(visualization.created_at)}
									</span>
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<!-- Recent Public Collections Section -->
		<div class="mt-3 lg:mt-4 bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p">
			<div class="flex justify-between items-center mb-3 lg:mb-4">
				<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
					Recent Public Collections
				</h2>
				<a
					href="#/marketplace"
					class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
				>
					View marketplace
				</a>
			</div>
			{#if publicCollections.length === 0}
				<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">
					No public collections available
				</p>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 lg:gap-4">
					{#each publicCollections as collection (collection.collection_id)}
						<div
							class="p-2 lg:p-4 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors min-card-height"
						>
							<h3 class="font-medium text-gray-900 dark:text-white mb-2">
								{collection.title}
							</h3>
							{#if collection.details}
								<p class="text-sm text-gray-600 dark:text-gray-400 mb-3 line-clamp-2">
									{collection.details}
								</p>
							{/if}
							<div class="flex gap-2 flex-wrap mb-3">
								{#each collection.tags.slice(0, 3) as tag (tag)}
									<span
										class="text-xs px-2 py-0.5 bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 rounded"
									>
										#{tag}
									</span>
								{/each}
							</div>
							<div
								class="flex justify-between items-center text-xs text-gray-500 dark:text-gray-400"
							>
								<span>{formatRelativeTime(collection.updated_at)}</span>
								<button
									disabled={grabbingId === collection.collection_id}
									onclick={() => grabCollection(collection.collection_id)}
									class="text-blue-600 dark:text-blue-400 hover:underline disabled:opacity-50 disabled:cursor-not-allowed"
								>
									{grabbingId === collection.collection_id ? 'Grabbing...' : 'Grab →'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Recent Public Datasets Section -->
		<div class="mt-3 lg:mt-4 bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p">
			<div class="flex justify-between items-center mb-3 lg:mb-4">
				<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
					Recent Public Datasets
				</h2>
				<a
					href="#/marketplace"
					class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
				>
					View marketplace
				</a>
			</div>
			{#if publicDatasets.length === 0}
				<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">
					No public datasets available
				</p>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 lg:gap-4">
					{#each publicDatasets as dataset (dataset.dataset_id)}
						<div
							class="p-2 lg:p-4 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors min-card-height"
						>
							<h3 class="font-medium text-gray-900 dark:text-white mb-2">
								{dataset.title}
							</h3>
							{#if dataset.details}
								<p class="text-sm text-gray-600 dark:text-gray-400 mb-3 line-clamp-2">
									{dataset.details}
								</p>
							{/if}
							<div class="flex gap-2 flex-wrap mb-3">
								{#each dataset.tags.slice(0, 3) as tag (tag)}
									<span
										class="text-xs px-2 py-0.5 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded"
									>
										#{tag}
									</span>
								{/each}
							</div>
							<div
								class="flex justify-between items-center text-xs text-gray-500 dark:text-gray-400"
							>
								<span>{formatRelativeTime(dataset.updated_at)}</span>
								<button
									disabled={grabbingId === dataset.dataset_id}
									onclick={() => grabDataset(dataset.dataset_id)}
									class="text-blue-600 dark:text-blue-400 hover:underline disabled:opacity-50 disabled:cursor-not-allowed"
								>
									{grabbingId === dataset.dataset_id ? 'Grabbing...' : 'Grab →'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Recent Public Embedders Section -->
		<div class="mt-3 lg:mt-4 bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p">
			<div class="flex justify-between items-center mb-3 lg:mb-4">
				<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
					Recent Public Embedders
				</h2>
				<a
					href="#/marketplace"
					class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
				>
					View marketplace
				</a>
			</div>
			{#if publicEmbedders.length === 0}
				<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">
					No public embedders available
				</p>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 lg:gap-4">
					{#each publicEmbedders as embedder (embedder.embedder_id)}
						<div
							class="p-2 lg:p-4 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors min-card-height"
						>
							<h3 class="font-medium text-gray-900 dark:text-white mb-2">
								{embedder.name}
							</h3>
							<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
								Provider: {embedder.provider}
							</p>
							<div
								class="flex justify-between items-center text-xs text-gray-500 dark:text-gray-400"
							>
								<span>{formatRelativeTime(embedder.updated_at)}</span>
								<button
									disabled={grabbingId === embedder.embedder_id}
									onclick={() => grabEmbedder(embedder.embedder_id)}
									class="text-blue-600 dark:text-blue-400 hover:underline disabled:opacity-50 disabled:cursor-not-allowed"
								>
									{grabbingId === embedder.embedder_id ? 'Grabbing...' : 'Grab →'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Recent Public LLMs Section -->
		<div class="mt-3 lg:mt-4 bg-white dark:bg-gray-800 rounded-lg shadow-md compact-p">
			<div class="flex justify-between items-center mb-3 lg:mb-4">
				<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white">
					Recent Public LLMs
				</h2>
				<a
					href="#/marketplace"
					class="text-blue-600 dark:text-blue-400 hover:underline text-xs lg:text-sm"
				>
					View marketplace
				</a>
			</div>
			{#if publicLLMs.length === 0}
				<p class="text-gray-500 dark:text-gray-400 text-xs lg:text-sm py-4">
					No public LLMs available
				</p>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 lg:gap-4">
					{#each publicLLMs as llm (llm.llm_id)}
						<div
							class="p-2 lg:p-4 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors min-card-height"
						>
							<h3 class="font-medium text-gray-900 dark:text-white mb-2">
								{llm.name}
							</h3>
							<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
								Provider: {llm.provider}
							</p>
							<div
								class="flex justify-between items-center text-xs text-gray-500 dark:text-gray-400"
							>
								<span>{formatRelativeTime(llm.updated_at)}</span>
								<button
									disabled={grabbingId === llm.llm_id}
									onclick={() => grabLLM(llm.llm_id)}
									class="text-blue-600 dark:text-blue-400 hover:underline disabled:opacity-50 disabled:cursor-not-allowed"
								>
									{grabbingId === llm.llm_id ? 'Grabbing...' : 'Grab →'}
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>
