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

	interface Embedder {
		embedder_id: number;
		name: string;
		owner: string;
		provider: string;
		base_url: string;
		api_key: string | null;
		config: Record<string, any>;
		collection_name: string;
		created_at: string;
		updated_at: string;
	}

	interface DatasetEmbedders {
		dataset_id: number;
		embedders: Embedder[];
	}

	interface SearchMatch {
		id: string;
		score: number;
		text: string;
		metadata: Record<string, any>;
	}

	interface EmbedderSearchResults {
		embedder_id: number;
		embedder_name: string;
		collection_name: string;
		matches: SearchMatch[];
		error?: string;
	}

	interface SearchResponse {
		results: EmbedderSearchResults[];
		query: string;
	}

	let datasets = $state<Dataset[]>([]);
	let datasetEmbedders = $state<DatasetEmbedders[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');
	let selectedDatasetId = $state<number | null>(null);
	let selectedEmbedderIds = new SvelteSet<number>();
	let searching = $state(false);
	let searchResults = $state<SearchResponse | null>(null);

	$effect(() => {
		if (selectedDatasetId !== null) {
			const datasetEmbeddersList = embeddersByDataset.get(selectedDatasetId) || [];
			selectedEmbedderIds.clear();
			datasetEmbeddersList.forEach((e) => selectedEmbedderIds.add(e.embedder_id));
		}
	});

	let showAdvanced = $state(false);
	let limit = $state(10);
	let scoreThreshold = $state(0.0);
	let exactSearch = $state(false);
	let hnswEf = $state<number | null>(null);
	let metadataFilters = $state('{}');

	const metadataPlaceholder = '{"category": "example", "year": 2024}';

	let embeddersByDataset = $derived.by(() => {
		const grouped = new SvelteMap<number, Embedder[]>();

		datasetEmbedders.forEach((de) => {
			grouped.set(de.dataset_id, de.embedders);
		});

		return grouped;
	});

	let allEmbedders = $derived.by(() => {
		const result: Embedder[] = [];
		datasetEmbedders.forEach((de) => {
			result.push(...de.embedders);
		});
		return result;
	});

	let canSearch = $derived(
		searchQuery.trim().length > 0 && selectedDatasetId !== null && selectedEmbedderIds.size > 0
	);

	async function fetchData() {
		try {
			loading = true;
			error = null;

			const [datasetsData, datasetEmbeddersData] = await Promise.all([
				apiCall<Dataset[]>('/api/datasets'),
				apiCall<DatasetEmbedders[]>('/api/datasets/embedders'),
			]);

			datasets = datasetsData;
			datasetEmbedders = datasetEmbeddersData;

			if (datasets.length > 0 && selectedDatasetId === null) {
				selectedDatasetId = datasets[0].dataset_id;
			}
		} catch (e) {
			const message = formatError(e, 'Failed to load datasets');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	function toggleEmbedder(embedderId: number) {
		if (selectedEmbedderIds.has(embedderId)) {
			selectedEmbedderIds.delete(embedderId);
		} else {
			selectedEmbedderIds.add(embedderId);
		}
	}

	async function embedQuery(embedder: Embedder, query: string): Promise<number[]> {
		const { provider, base_url, api_key, config } = embedder;

		const headers: Record<string, string> = {
			'Content-Type': 'application/json',
		};

		if (api_key) {
			headers['Authorization'] = `Bearer ${api_key}`;
		}

		let body: any;
		let endpoint = base_url;

		if (provider === 'openai') {
			endpoint = `${base_url}/embeddings`;
			body = {
				input: query,
				model: config.model || 'text-embedding-3-small',
			};
		} else if (provider === 'cohere') {
			body = {
				texts: [query],
				model: config.model || 'embed-v4.0',
				input_type: 'search_query',
			};
		} else {
			throw new Error(`Unsupported provider: ${provider}`);
		}

		const response = await fetch(endpoint, {
			method: 'POST',
			headers,
			body: JSON.stringify(body),
		});

		if (!response.ok) {
			const errorText = await response.text();
			throw new Error(`Embedding failed (${response.status}): ${errorText}`);
		}

		const data = await response.json();

		let embedding: number[] | undefined;

		if (provider === 'openai') {
			embedding = data?.data?.[0]?.embedding;
		} else if (provider === 'cohere') {
			if (
				data?.embeddings &&
				typeof data.embeddings === 'object' &&
				!Array.isArray(data.embeddings)
			) {
				const embeddingsArray = data.embeddings.float || data.embeddings.int;
				embedding = embeddingsArray?.[0];
			} else {
				embedding = data?.embeddings?.[0];
			}
		}

		if (!embedding || !Array.isArray(embedding)) {
			console.error(`Invalid embedding response from ${provider}:`, data);
			throw new Error(
				`Invalid embedding format from ${provider}. Expected array, got: ${typeof embedding}. Raw response: ${JSON.stringify(data)}`
			);
		}

		return embedding;
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

			const embeddings: Record<number, number[]> = {};
			const embeddingErrors: string[] = [];

			for (const embedderId of selectedEmbedderIds) {
				const embedder = allEmbedders.find((e) => e.embedder_id === embedderId);
				if (!embedder) {
					console.error(`Embedder ${embedderId} not found in allEmbedders`);
					embeddingErrors.push(`Embedder ${embedderId} not found`);
					continue;
				}

				try {
					const embedding = await embedQuery(embedder, searchQuery);
					embeddings[embedderId] = embedding;
				} catch (error) {
					console.error(`Failed to embed query for ${embedder.name}:`, error);
					embeddingErrors.push(
						`${embedder.name}: ${error instanceof Error ? error.message : String(error)}`
					);
				}
			}

			if (Object.keys(embeddings).length === 0) {
				const detail = embeddingErrors.join('; ');
				toastStore.error(
					'Could not embed the query for any selected embedders',
					'Embedding failed'
				);
				if (detail) {
					console.error('Embedding failures:', detail);
				}
				return;
			}

			if (embeddingErrors.length > 0) {
				toastStore.warning(
					`Some embeds failed: ${embeddingErrors.join('; ')}`,
					'Partial embedding issues',
					8000
				);
				console.warn('Some embeddings failed:', embeddingErrors);
			}

			const response = await apiCall<SearchResponse>('/api/search', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					query: searchQuery,
					dataset_id: selectedDatasetId,
					embeddings,
					limit,
					score_threshold: scoreThreshold,
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
		description="Provides the ability to execute semantic searches against multiple embedded datasets for comparison purposes. Test and evaluate different embedding models side-by-side to determine which performs best for your use case."
	/>

	<h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-6">Search Datasets</h1>

	{#if loading}
		<div class="flex justify-center items-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
		</div>
	{:else if error}
		<div class="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-4 rounded-lg">
			{error}
		</div>
	{:else}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
				<div>
					<label
						for="dataset-filter"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						1. Select Dataset
					</label>
					<select
						id="dataset-filter"
						bind:value={selectedDatasetId}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
					>
						{#each datasets as dataset (dataset.dataset_id)}
							<option value={dataset.dataset_id}>{dataset.title}</option>
						{/each}
					</select>
				</div>

				<div class="md:col-span-2">
					<span class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
						2. Select Embedders
					</span>
					{#if selectedDatasetId !== null}
						{@const datasetEmbeddersList = embeddersByDataset.get(selectedDatasetId) || []}
						<div class="flex flex-wrap gap-3">
							{#each datasetEmbeddersList as embedder (embedder.embedder_id)}
								<label
									class="flex items-center gap-2 cursor-pointer bg-gray-50 dark:bg-gray-700 px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-600 hover:border-blue-400 dark:hover:border-blue-500 transition-colors"
								>
									<input
										type="checkbox"
										checked={selectedEmbedderIds.has(embedder.embedder_id)}
										onchange={() => toggleEmbedder(embedder.embedder_id)}
										class="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
									/>
									<div class="flex flex-col">
										<span class="text-sm font-medium text-gray-900 dark:text-white">
											{embedder.name}
										</span>
										<span class="text-xs text-gray-500 dark:text-gray-400">
											{embedder.provider}
										</span>
									</div>
								</label>
							{/each}
							{#if datasetEmbeddersList.length === 0}
								<div class="text-sm text-gray-500 dark:text-gray-400 py-2">
									No embedders configured for this dataset.
								</div>
							{/if}
						</div>
					{:else}
						<div class="text-sm text-gray-500 dark:text-gray-400 py-2">
							Please select a dataset first.
						</div>
					{/if}
				</div>
			</div>
		</div>

		{#if selectedDatasetId !== null && selectedEmbedderIds.size > 0}
			<div class="space-y-6">
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Search Query</h2>

					<div class="mb-4">
						<textarea
							bind:value={searchQuery}
							onkeypress={handleKeyPress}
							placeholder="Enter your search query..."
							class="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none focus:ring-2 focus:ring-blue-500"
							rows="3"
						></textarea>
					</div>

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
										Limit (per embedder)
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
						class="w-full px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors font-medium"
					>
						{searching ? 'Searching...' : 'Search'}
					</button>
				</div>

				{#if searchResults}
					<div class="space-y-6">
						<div class="flex items-center justify-between">
							<h2 class="text-xl font-semibold text-gray-900 dark:text-white">
								Results for: "{searchResults.query}"
							</h2>
							<span class="text-sm text-gray-600 dark:text-gray-400">
								{searchResults.results.length} embedder{searchResults.results.length !== 1
									? 's'
									: ''}
							</span>
						</div>

						<div class="overflow-x-auto pb-4">
							<div class="flex justify-center gap-6 min-w-max">
								{#each searchResults.results as result (result.embedder_id)}
									<div class="w-96 shrink-0 bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
										<div class="flex items-start justify-between mb-4">
											<div>
												<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
													{result.embedder_name}
												</h3>
												<p class="text-sm text-gray-600 dark:text-gray-400">
													Embedded Dataset: {result.collection_name}
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
																	Result #{idx + 1}
																</div>
																{#if match.metadata.item_id || match.metadata.name || match.metadata.file || match.metadata.title}
																	<div
																		class="text-sm font-semibold text-gray-900 dark:text-white mt-1"
																	>
																		{match.metadata.title ||
																			match.metadata.name ||
																			match.metadata.file ||
																			`Item ${match.metadata.item_id}`}
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
															class="bg-gray-50 dark:bg-gray-900 rounded p-3 mb-3 border border-gray-200 dark:border-gray-700"
														>
															<p
																class="text-sm text-gray-900 dark:text-gray-100 leading-relaxed whitespace-pre-wrap line-clamp-6"
															>
																{match.text}
															</p>
														</div>

														{#if Object.keys(match.metadata).length > 0}
															<details class="mt-3">
																<summary
																	class="text-xs font-medium text-gray-600 dark:text-gray-400 cursor-pointer hover:text-gray-900 dark:hover:text-gray-200 flex items-center gap-2"
																>
																	<span>📋 Metadata & Details</span>
																	<span class="text-gray-400">▾</span>
																</summary>
																<div class="mt-2 space-y-2 text-xs">
																	{#each Object.entries(match.metadata) as [key, value] (key)}
																		<div class="flex flex-col">
																			<span class="font-medium text-gray-700 dark:text-gray-300">
																				{key}:
																			</span>
																			<span class="text-gray-600 dark:text-gray-400 ml-2">
																				{typeof value === 'string' ? value : JSON.stringify(value)}
																			</span>
																		</div>
																	{/each}
																</div>
															</details>
														{/if}

														<div class="text-xs text-gray-500 dark:text-gray-500 mt-3">
															ID: {match.id}
														</div>
													</div>
												{/each}
											</div>
										{/if}
									</div>
								{/each}
							</div>
						</div>
					</div>
				{/if}
			</div>
		{:else}
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
					Select a dataset and at least one embedder above to start searching and comparing results.
				</p>
			</div>
		{/if}
	{/if}
</div>
