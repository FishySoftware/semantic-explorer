<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import PageHeader from '../components/PageHeader.svelte';
	import { apiCall } from '../utils/api';
	import { formatError, toastStore } from '../utils/notifications';

	interface Dataset {
		dataset_id: number;
		title: string;
		details: string | null;
		owner: string;
		tags: string[];
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		dataset_transform_id: number;
		source_dataset_id: number;
		source_dataset_title: string;
		embedder_id: number;
		embedder_name: string;
		owner: string;
		collection_name: string;
		created_at: string;
		updated_at: string;
	}

	interface SearchMatch {
		id: string;
		score: number;
		text: string;
		metadata: Record<string, any>;
	}

	interface DocumentResult {
		item_id: number;
		item_title: string;
		best_score: number;
		chunk_count: number;
		best_chunk: SearchMatch;
	}

	interface EmbeddedDatasetSearchResults {
		embedded_dataset_id: number;
		embedded_dataset_title: string;
		source_dataset_id: number;
		source_dataset_title: string;
		embedder_id: number;
		embedder_name: string;
		collection_name: string;
		matches: SearchMatch[];
		documents?: DocumentResult[];
		error?: string;
	}

	interface SearchResponse {
		results: EmbeddedDatasetSearchResults[];
		query: string;
		search_mode: 'documents' | 'chunks';
	}

	let datasets = $state<Dataset[]>([]);
	let allEmbeddedDatasets = $state<EmbeddedDataset[]>([]);
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

	async function fetchData() {
		try {
			loading = true;
			error = null;

			const [datasetsData, embeddedDatasetsData] = await Promise.all([
				apiCall<Dataset[]>('/api/datasets'),
				apiCall<EmbeddedDataset[]>('/api/embedded-datasets'),
			]);

			datasets = datasetsData;
			allEmbeddedDatasets = embeddedDatasetsData;

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

	onMount(() => {
		fetchData();
	});
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Search"
		description="Execute semantic searches against multiple embedded datasets to test and evaluate different embedding models."
	/>

	{#if loading}
		<div class="flex justify-center items-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
		</div>
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
					class="max-h-64 overflow-y-auto border border-gray-200 dark:border-gray-600 rounded-lg p-3 bg-gray-50 dark:bg-gray-900"
				>
					{#if filteredEmbeddedDatasets.length === 0}
						<div class="text-sm text-gray-500 dark:text-gray-400 py-2 text-center">
							No embedded datasets available.
						</div>
					{:else}
						<div class="space-y-2">
							{#each filteredEmbeddedDatasets as embeddedDataset (embeddedDataset.embedded_dataset_id)}
								<label
									class="flex items-start gap-3 cursor-pointer bg-white dark:bg-gray-800 px-3 py-2 rounded border border-gray-200 dark:border-gray-600 hover:border-blue-400 dark:hover:border-blue-500 transition-colors"
								>
									<input
										type="checkbox"
										checked={selectedEmbeddedDatasetIds.has(embeddedDataset.embedded_dataset_id)}
										onchange={() => toggleEmbeddedDataset(embeddedDataset.embedded_dataset_id)}
										class="mt-1 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
									/>
									<div class="flex-1 flex flex-col">
										<span class="text-sm font-medium text-gray-900 dark:text-white">
											{embeddedDataset.title}
										</span>
										<div class="text-xs text-gray-500 dark:text-gray-400 mt-1 space-y-0.5">
											<div>Dataset: {embeddedDataset.source_dataset_title}</div>
											<div>Embedder: {embeddedDataset.embedder_name}</div>
											<div class="font-mono text-[10px]">
												{embeddedDataset.collection_name}
											</div>
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
							: `${searchResults.results.length} embedded dataset${searchResults.results.length !== 1 ? 's' : ''}`}
					</span>
				</div>

				<!-- Results display: side-by-side columns per embedded dataset -->
				{#if searchResults.search_mode === 'documents'}
					<div class="overflow-x-auto pb-4">
						<div class="flex justify-center gap-4 min-w-max">
							{#each searchResults.results as result (result.embedded_dataset_id)}
								<div class="w-96 shrink-0 bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
									<div class="flex items-start justify-between mb-4">
										<div>
											<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
												{result.embedded_dataset_title}
											</h3>
											<p class="text-sm text-gray-600 dark:text-gray-400">
												Dataset: {result.source_dataset_title}
											</p>
											<p class="text-xs text-gray-500 dark:text-gray-500">
												Embedder: {result.embedder_name}
											</p>
										</div>
									</div>

									{#if result.error}
										<div
											class="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-3 rounded text-sm"
										>
											{result.error}
										</div>
									{:else if !result.documents || result.documents.length === 0}
										<div class="text-gray-500 dark:text-gray-400 text-center py-4">
											No results found
										</div>
									{:else}
										<div class="space-y-3">
											{#each result.documents as document, idx (document.item_id)}
												<div
													class="border border-gray-200 dark:border-gray-700 rounded-lg p-4 hover:border-blue-400 dark:hover:border-blue-600 transition-colors"
												>
													<div class="flex items-start justify-between mb-3">
														<div class="flex-1">
															<div class="text-xs font-medium text-gray-500 dark:text-gray-400">
																Document #{idx + 1}
															</div>
															<div class="text-sm font-semibold text-gray-900 dark:text-white mt-1">
																📄 {document.item_title}
															</div>
															<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">
																{document.chunk_count} chunk{document.chunk_count !== 1 ? 's' : ''}
															</div>
														</div>
														<div class="text-right">
															<div class="text-xs text-gray-600 dark:text-gray-400">Best Score</div>
															<div class="text-lg font-bold text-blue-600 dark:text-blue-400">
																{document.best_score.toFixed(4)}
															</div>
														</div>
													</div>

													<div
														class="bg-gray-50 dark:bg-gray-900 rounded p-3 border border-gray-200 dark:border-gray-700"
													>
														<div class="text-xs text-gray-600 dark:text-gray-400 mb-2">
															Best matching chunk:
														</div>
														<p
															class="text-sm text-gray-900 dark:text-gray-100 leading-relaxed whitespace-pre-wrap line-clamp-4"
														>
															{document.best_chunk.text}
														</p>
													</div>
												</div>
											{/each}
										</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{:else}
					<!-- Chunks Mode: Show side-by-side embedded dataset results -->
					<div class="overflow-x-auto pb-4">
						<div class="flex justify-center gap-4 min-w-max">
							{#each searchResults.results as result (result.embedded_dataset_id)}
								<div class="w-96 shrink-0 bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
									<div class="flex items-start justify-between mb-4">
										<div>
											<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
												{result.embedded_dataset_title}
											</h3>
											<p class="text-sm text-gray-600 dark:text-gray-400">
												Dataset: {result.source_dataset_title}
											</p>
											<p class="text-xs text-gray-500 dark:text-gray-500">
												Embedder: {result.embedder_name}
											</p>
										</div>
									</div>

									{#if result.error}
										<div
											class="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-3 rounded text-sm"
										>
											{result.error}
										</div>
									{:else if result.matches.length === 0}
										<div class="text-gray-500 dark:text-gray-400 text-center py-4">
											No results found
										</div>
									{:else}
										<div class="space-y-3">
											{#each result.matches as match, idx (match.id)}
												<div
													class="border border-gray-200 dark:border-gray-700 rounded-lg p-4 hover:border-blue-400 dark:hover:border-blue-600 transition-colors"
												>
													<div class="flex items-start justify-between mb-3">
														<div class="flex-1">
															<div class="text-xs font-medium text-gray-500 dark:text-gray-400">
																Chunk #{match.metadata.chunk_index || idx + 1}
															</div>
															{#if match.metadata.item_title}
																<div
																	class="text-sm font-semibold text-gray-900 dark:text-white mt-1"
																>
																	{match.metadata.item_title}
																</div>
															{/if}
														</div>
														<div class="text-right">
															<div class="text-xs text-gray-600 dark:text-gray-400">Score</div>
															<div class="text-lg font-bold text-blue-600 dark:text-blue-400">
																{match.score.toFixed(4)}
															</div>
														</div>
													</div>

													<div
														class="bg-gray-50 dark:bg-gray-900 rounded p-3 border border-gray-200 dark:border-gray-700"
													>
														<p
															class="text-sm text-gray-900 dark:text-gray-100 leading-relaxed whitespace-pre-wrap line-clamp-6"
														>
															{match.text}
														</p>
													</div>
												</div>
											{/each}
										</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{/if}
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
