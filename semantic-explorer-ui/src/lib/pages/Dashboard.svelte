<script lang="ts">
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Collection {
		collection_id: number;
		title: string;
		details: string | null;
		updated_at: string;
		tags: string[];
	}

	interface Dataset {
		dataset_id: number;
		title: string;
		details: string | null;
		updated_at: string;
		tags: string[];
	}

	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(async () => {
		await fetchData();
	});

	async function fetchData() {
		try {
			loading = true;
			error = null;

			const [collectionsRes, datasetsRes] = await Promise.all([
				fetch('/api/collections'),
				fetch('/api/datasets'),
			]);

			if (!collectionsRes.ok || !datasetsRes.ok) {
				throw new Error('Failed to fetch data');
			}

			const allCollections = await collectionsRes.json();
			const allDatasets = await datasetsRes.json();

			collections = allCollections
				.sort(
					(a: Collection, b: Collection) =>
						new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
				)
				.slice(0, 5);

			datasets = allDatasets
				.sort(
					(a: Dataset, b: Dataset) =>
						new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
				)
				.slice(0, 5);
		} catch (e) {
			const message = formatError(e, 'Failed to load dashboard data');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	function formatDate(dateStr: string): string {
		try {
			const date = new Date(dateStr);
			if (isNaN(date.getTime())) {
				return 'Invalid date: ' + dateStr;
			}
			const now = new Date();
			const diffMs = now.getTime() - date.getTime();
			const diffMins = Math.floor(diffMs / 60000);
			const diffHours = Math.floor(diffMs / 3600000);
			const diffDays = Math.floor(diffMs / 86400000);

			if (diffMins < 1) return 'Just now';
			if (diffMins < 60) return `${diffMins} minute${diffMins === 1 ? '' : 's'} ago`;
			if (diffHours < 24) return `${diffHours} hour${diffHours === 1 ? '' : 's'} ago`;
			if (diffDays < 7) return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
			return date.toLocaleDateString();
		} catch {
			return 'Invalid date';
		}
	}

</script>

<div class="max-w-7xl mx-auto">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">Dashboard</h1>
		<p class="text-gray-600 dark:text-gray-400">
			Welcome to Semantic Explorer - Your document processing and embedding platform
		</p>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
		</div>
	{:else}
		<div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
				<div class="flex justify-between items-center mb-4">
					<h2 class="text-xl font-semibold text-gray-900 dark:text-white">Recent Collections</h2>
					<a href="#/collections" class="text-blue-600 dark:text-blue-400 hover:underline text-sm">
						View all
					</a>
				</div>
				{#if collections.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-sm py-4">No collections yet</p>
				{:else}
					<div class="space-y-3">
						{#each collections as collection (collection.collection_id)}
							<a
								href={`#/collections/${collection.collection_id}/details`}
								class="block p-3 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-750 transition-colors"
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
										{formatDate(collection.updated_at)}
									</span>
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</div>

			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
				<div class="flex justify-between items-center mb-4">
					<h2 class="text-xl font-semibold text-gray-900 dark:text-white">Recent Datasets</h2>
					<a href="#/datasets" class="text-blue-600 dark:text-blue-400 hover:underline text-sm">
						View all
					</a>
				</div>
				{#if datasets.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-sm py-4">No datasets yet</p>
				{:else}
					<div class="space-y-3">
						{#each datasets as dataset (dataset.dataset_id)}
							<a
								href={`#/datasets/${dataset.dataset_id}/details`}
								class="block p-3 bg-gray-50 dark:bg-gray-900 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-750 transition-colors"
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
										{formatDate(dataset.updated_at)}
									</span>
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<div class="mt-6 grid grid-cols-1 md:grid-cols-4 gap-4">
			<a
				href="#/collections"
				class="p-6 bg-linear-to-br from-blue-500 to-blue-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
			>
				<svg class="w-8 h-8 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
					></path>
				</svg>
				<h3 class="text-lg font-semibold">Collections</h3>
				<p class="text-sm text-blue-100 mt-1">Manage your document collections</p>
			</a>

			<a
				href="#/datasets"
				class="p-6 bg-linear-to-br from-green-500 to-green-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
			>
				<svg class="w-8 h-8 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
					></path>
				</svg>
				<h3 class="text-lg font-semibold">Datasets</h3>
				<p class="text-sm text-green-100 mt-1">View and manage your datasets</p>
			</a>

			<a
				href="#/collection-transforms"
				class="p-6 bg-linear-to-br from-purple-500 to-purple-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
			>
				<svg class="w-8 h-8 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
					></path>
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
					></path>
				</svg>
				<h3 class="text-lg font-semibold">Collection Transforms</h3>
				<p class="text-sm text-purple-100 mt-1">Extract data from collections</p>
			</a>

			<a
				href="#/visualization-transforms"
				class="p-6 bg-linear-to-br from-orange-500 to-orange-600 text-white rounded-lg shadow-md hover:shadow-lg transition-shadow"
			>
				<svg class="w-8 h-8 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4a1 1 0 011-1h16a1 1 0 011 1v2.757a1 1 0 01-.293.707L12 16.414l-7.414-7.414A1 1 0 013 8.343V4z"
					></path>
				</svg>
				<h3 class="text-lg font-semibold">Visualizations</h3>
				<p class="text-sm text-orange-100 mt-1">Create 3D visualizations</p>
			</a>
		</div>
	{/if}
</div>
