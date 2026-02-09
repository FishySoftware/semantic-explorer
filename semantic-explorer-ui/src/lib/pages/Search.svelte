<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import LoadingState from '../components/LoadingState.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { SearchResultsTable } from '../components/search';
	import type {
		Dataset,
		EmbeddedDataset,
		Embedder,
		PaginatedEmbeddedDatasetList,
		PaginatedResponse,
		SearchResponse,
	} from '../types/models';
	import { apiCall } from '../utils/api';
	import { formatError, toastStore } from '../utils/notifications';

	let {
		onViewDataset: handleViewDataset,
		onViewEmbedder: handleViewEmbedder,
		onViewEmbeddedDataset: handleViewEmbeddedDataset,
	} = $props<{
		onViewDataset?: (_: number) => void;
		onViewEmbedder?: (_: number) => void;
		onViewEmbeddedDataset?: (_: number) => void;
	}>();

	const onViewDataset = (id: number) => {
		handleViewDataset?.(id);
	};

	const onViewEmbedder = (id: number) => {
		handleViewEmbedder?.(id);
	};

	const onViewEmbeddedDataset = (id: number) => {
		handleViewEmbeddedDataset?.(id);
	};

	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);
	let allEmbeddedDatasets = $state<EmbeddedDataset[]>([]);
	let datasetsCache = new SvelteMap<number, Dataset>();
	let embeddersCache = new SvelteMap<number, Embedder>();
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');
	let selectedDatasetId = $state<number | null>(null); // For filtering display
	let selectedEmbeddedDatasetIds = new SvelteSet<number>();
	let searching = $state(false);
	let searchResults = $state<SearchResponse | null>(null);
	let searchMode = $state<'documents' | 'chunks'>('documents');

	let showAdvanced = $state(false);
	let limit = $state(10);
	let scoreThreshold = $state(0.0);
	let exactSearch = $state(false);
	let hnswEf = $state<number | null>(null);
	let metadataFilters = $state('{}');

	const metadataPlaceholder = '{"category": "example", "year": 2024}';

	// Group embedded datasets by source dataset for UI display
	let embeddedDatasetsByDataset = $derived.by(() => {
		const grouped = new SvelteMap<number, EmbeddedDataset[]>();

		allEmbeddedDatasets.forEach((ed) => {
			if (!grouped.has(ed.source_dataset_id)) {
				grouped.set(ed.source_dataset_id, []);
			}
			grouped.get(ed.source_dataset_id)!.push(ed);
		});

		return grouped;
	});

	// Filtered embedded datasets based on selected dataset (for display)
	let filteredEmbeddedDatasets = $derived.by(() => {
		if (selectedDatasetId === null) {
			return allEmbeddedDatasets;
		}
		return embeddedDatasetsByDataset.get(selectedDatasetId) || [];
	});

	let canSearch = $derived(searchQuery.trim().length > 0 && selectedEmbeddedDatasetIds.size > 0);

	// Auto-select all filtered embedded datasets when the filter changes
	$effect(() => {
		const filteredIds = filteredEmbeddedDatasets.map((ed) => ed.embedded_dataset_id);
		// Clear and re-add all filtered datasets
		selectedEmbeddedDatasetIds.clear();
		filteredIds.forEach((id) => selectedEmbeddedDatasetIds.add(id));
	});

	async function fetchData() {
		try {
			loading = true;
			error = null;

			const [datasetsResponse, embeddedDatasetsResponse, embeddersResponse] = await Promise.all([
				apiCall<PaginatedResponse<Dataset>>('/api/datasets?limit=1000'),
				apiCall<PaginatedEmbeddedDatasetList>('/api/embedded-datasets?limit=1000'),
				apiCall<PaginatedResponse<Embedder>>('/api/embedders?limit=1000'),
			]);

			datasets = datasetsResponse.items;
			embedders = embeddersResponse.items;
			// Filter out standalone datasets - they don't have embedders and can't be searched
			allEmbeddedDatasets = embeddedDatasetsResponse.embedded_datasets.filter(
				(ed) =>
					!ed.is_standalone &&
					!(ed.dataset_transform_id === 0 && ed.source_dataset_id === 0 && ed.embedder_id === 0)
			);

			// Build caches for quick lookup
			datasetsCache.clear();
			datasets.forEach((d) => datasetsCache.set(d.dataset_id, d));

			embeddersCache.clear();
			embedders.forEach((e) => embeddersCache.set(e.embedder_id, e));

			// Default to first dataset for filtering display
			if (datasets.length > 0 && selectedDatasetId === null) {
				selectedDatasetId = datasets[0].dataset_id;
			}
		} catch (e) {
			const message = formatError(e, 'Failed to load data');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	function toggleEmbeddedDataset(embeddedDatasetId: number) {
		if (selectedEmbeddedDatasetIds.has(embeddedDatasetId)) {
			selectedEmbeddedDatasetIds.delete(embeddedDatasetId);
		} else {
			selectedEmbeddedDatasetIds.add(embeddedDatasetId);
		}
	}

	async function performSearch() {
		if (!canSearch) return;

		try {
			searching = true;

			let filters = null;
			if (metadataFilters.trim()) {
				try {
					filters = JSON.parse(metadataFilters);
				} catch (parseError) {
					toastStore.error('Metadata filters must be valid JSON');
					console.error('Invalid metadata filters JSON:', parseError);
					return;
				}
			}

			const searchParams =
				exactSearch || hnswEf !== null
					? {
							exact: exactSearch,
							...(hnswEf !== null && { hnsw_ef: hnswEf }),
						}
					: null;

			const response = await apiCall<SearchResponse>('/api/search', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					query: searchQuery,
					embedded_dataset_ids: Array.from(selectedEmbeddedDatasetIds),
					limit,
					score_threshold: scoreThreshold,
					search_mode: searchMode,
					...(filters && { filters }),
					...(searchParams && { search_params: searchParams }),
				}),
			});

			searchResults = response;
		} catch (e) {
			const message = formatError(e, 'Search failed');
			toastStore.error(message);
			console.error('Search failed:', e);
		} finally {
			searching = false;
		}
	}

	function handleKeyPress(event: KeyboardEvent) {
		if (event.key === 'Enter' && canSearch) {
			performSearch();
		}
	}

	onMount(async () => {
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const params = new URLSearchParams(hashParts[1]);
			const embeddedDatasetIdsParam = params.get('embedded_dataset_ids');

			if (embeddedDatasetIdsParam) {
				const datasetIds = embeddedDatasetIdsParam
					.split(',')
					.map((id) => parseInt(id.trim(), 10))
					.filter((id) => !isNaN(id));

				await fetchData();

				datasetIds.forEach((id) => {
					if (allEmbeddedDatasets.some((ed) => ed.embedded_dataset_id === id)) {
						selectedEmbeddedDatasetIds.add(id);
					}
				});

				window.history.replaceState(null, '', '#/search');
				return;
			}
		}

		fetchData();
	});
</script>

<div class="mx-auto">
	<PageHeader
		title="Search"
		description="Execute semantic searches against multiple embedded datasets to test and evaluate different embedding models."
	/>

	{#if loading}
		<LoadingState message="Loading search configuration..." />
	{:else if error}
		<div class="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-4 rounded-lg">
			{error}
		</div>
	{:else}
		<!-- Search Query Section at Top -->
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Search Query</h2>

			<!-- Query Input -->
			<div class="mb-4">
				<textarea
					bind:value={searchQuery}
					onkeypress={handleKeyPress}
					placeholder="Enter your search query..."
					class="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none focus:ring-2 focus:ring-blue-500"
					rows="3"
				></textarea>
			</div>

			<!-- Search Mode Toggle -->
			<div class="mb-4 flex items-center gap-4">
				<span class="text-sm font-medium text-gray-700 dark:text-gray-300">Search Mode:</span>
				<div class="flex gap-2">
					<button
						onclick={() => (searchMode = 'documents')}
						class={searchMode === 'documents'
							? 'btn-primary'
							: 'btn-secondary hover:bg-gray-300 dark:hover:bg-gray-600'}
					>
						📄 Documents
					</button>
					<button
						onclick={() => (searchMode = 'chunks')}
						class={searchMode === 'chunks'
							? 'btn-primary'
							: 'btn-secondary hover:bg-gray-300 dark:hover:bg-gray-600'}
					>
						📁 Chunks
					</button>
				</div>
				<span class="text-xs text-gray-500 dark:text-gray-400">
					{searchMode === 'chunks' ? 'View individual chunks' : 'View results grouped by documents'}
				</span>
			</div>

			<!-- Dataset Filter and Embedded Dataset Selection -->
			<div class="mb-4">
				<label
					for="dataset-filter"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
				>
					Filter by Dataset (optional)
				</label>
				<select
					id="dataset-filter"
					bind:value={selectedDatasetId}
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
				>
					<option value={null}>All Datasets</option>
					{#each datasets as dataset (dataset.dataset_id)}
						<option value={dataset.dataset_id}>{dataset.title}</option>
					{/each}
				</select>
			</div>

			<div class="mb-4">
				<span class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
					Select Embedded Datasets to Search
					{#if selectedEmbeddedDatasetIds.size > 0}
						<span class="text-blue-600 dark:text-blue-400">
							({selectedEmbeddedDatasetIds.size} selected)
						</span>
					{/if}
				</span>

				<div
					class="max-h-48 overflow-y-auto border border-gray-200 dark:border-gray-600 rounded-lg p-2 bg-gray-50 dark:bg-gray-900"
				>
					{#if filteredEmbeddedDatasets.length === 0}
						<div class="text-sm text-gray-500 dark:text-gray-400 py-2 text-center">
							No embedded datasets available.
						</div>
					{:else}
						<div class="space-y-1">
							{#each filteredEmbeddedDatasets as embeddedDataset (embeddedDataset.embedded_dataset_id)}
								{@const dataset = datasetsCache.get(embeddedDataset.source_dataset_id)}
								{@const embedder = embeddersCache.get(embeddedDataset.embedder_id)}
								<label
									class="flex items-center gap-2 cursor-pointer bg-white dark:bg-gray-800 px-2 py-1.5 rounded border border-gray-200 dark:border-gray-600 hover:border-blue-400 dark:hover:border-blue-500 transition-colors"
								>
									<input
										type="checkbox"
										checked={selectedEmbeddedDatasetIds.has(embeddedDataset.embedded_dataset_id)}
										onchange={() => toggleEmbeddedDataset(embeddedDataset.embedded_dataset_id)}
										class="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
									/>
									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-2 flex-wrap">
											<button
												type="button"
												onclick={(e) => {
													e.stopPropagation();
													onViewEmbeddedDataset(embeddedDataset.embedded_dataset_id);
												}}
												class="text-xs font-semibold text-blue-600 dark:text-blue-400 hover:underline truncate"
											>
												{embeddedDataset.title}
											</button>
											<span class="text-gray-300 dark:text-gray-600">|</span>
											<span class="text-[10px] text-gray-500 dark:text-gray-400">
												{#if dataset}
													<button
														type="button"
														onclick={(e) => {
															e.stopPropagation();
															onViewDataset(dataset.dataset_id);
														}}
														class="hover:text-blue-600 dark:hover:text-blue-400 hover:underline"
													>
														📊 {dataset.title}
													</button>
												{:else}
													📊 ...
												{/if}
											</span>
											<span class="text-gray-300 dark:text-gray-600">|</span>
											<span class="text-[10px] text-gray-500 dark:text-gray-400">
												{#if embedder}
													<button
														type="button"
														onclick={(e) => {
															e.stopPropagation();
															onViewEmbedder(embedder.embedder_id);
														}}
														class="hover:text-blue-600 dark:hover:text-blue-400 hover:underline"
													>
														🧠 {embedder.name}
													</button>
													{#if embedder.dimensions}
														<span class="font-mono text-indigo-600 dark:text-indigo-400 ml-1"
															>({embedder.dimensions}d)</span
														>
													{/if}
												{:else}
													🧠 ...
												{/if}
											</span>
										</div>
									</div>
								</label>
							{/each}
						</div>
					{/if}
				</div>
			</div>

			<!-- Advanced Options -->
			<button
				onclick={() => (showAdvanced = !showAdvanced)}
				class="text-sm text-blue-600 dark:text-blue-400 hover:underline mb-3"
			>
				{showAdvanced ? '▼' : '▶'} Advanced Options
			</button>

			{#if showAdvanced}
				<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4 mb-4 space-y-4">
					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="search-limit"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Limit (per embedded dataset)
							</label>
							<input
								id="search-limit"
								type="number"
								bind:value={limit}
								min="1"
								max="100"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
						</div>

						<div>
							<label
								for="score-threshold"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Score Threshold (0-1)
							</label>
							<input
								id="score-threshold"
								type="number"
								bind:value={scoreThreshold}
								min="0"
								max="1"
								step="0.1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
						</div>
					</div>

					<div class="flex items-center gap-4">
						<label class="flex items-center gap-2 cursor-pointer">
							<input
								type="checkbox"
								bind:checked={exactSearch}
								class="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
							/>
							<span class="text-sm text-gray-700 dark:text-gray-300"
								>Exact search (slower, more accurate)</span
							>
						</label>
					</div>

					<div>
						<label
							for="hnsw-ef"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							HNSW ef parameter (optional, higher = more accurate but slower)
						</label>
						<input
							id="hnsw-ef"
							type="number"
							bind:value={hnswEf}
							min="0"
							placeholder="Leave empty for default"
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						/>
					</div>

					<div>
						<label
							for="metadata-filters"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Metadata Filters (JSON)
						</label>
						<textarea
							id="metadata-filters"
							bind:value={metadataFilters}
							placeholder={metadataPlaceholder}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white font-mono text-sm"
							rows="3"
						></textarea>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Filter results by metadata fields
						</p>
					</div>
				</div>
			{/if}

			<button
				onclick={performSearch}
				disabled={!canSearch || searching}
				class="w-full btn-primary disabled:opacity-50 disabled:cursor-not-allowed font-medium"
			>
				{searching ? 'Searching...' : 'Search'}
			</button>
		</div>

		<!-- Search Results -->
		{#if searchResults}
			<div class="space-y-6">
				<div class="flex items-center justify-between">
					<h2 class="text-xl font-semibold text-gray-900 dark:text-white">
						Results for: "{searchResults.query}"
					</h2>
					<span class="text-sm text-gray-600 dark:text-gray-400">
						{searchResults.search_mode === 'documents'
							? `${searchResults.results.reduce((sum, r) => sum + (r.documents?.length || 0), 0)} document${searchResults.results.reduce((sum, r) => sum + (r.documents?.length || 0), 0) !== 1 ? 's' : ''}`
							: `${searchResults.results.reduce((sum, r) => sum + (r.matches?.length || 0), 0)} chunk${searchResults.results.reduce((sum, r) => sum + (r.matches?.length || 0), 0) !== 1 ? 's' : ''}`}
						across {searchResults.results.length} embedded dataset{searchResults.results.length !==
						1
							? 's'
							: ''}
					</span>
				</div>

				<!-- Table-based comparison view -->
				<SearchResultsTable
					results={searchResults.results}
					searchMode={searchResults.search_mode}
					{onViewDataset}
					{onViewEmbedder}
					{onViewEmbeddedDataset}
				/>
			</div>
		{:else if !loading}
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
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
						d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
					></path>
				</svg>
				<h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">Ready to Search</h3>
				<p class="text-gray-500 dark:text-gray-400">
					Select a dataset and at least one embedder, enter your query, and click Search.
				</p>
			</div>
		{/if}
	{/if}
</div>
