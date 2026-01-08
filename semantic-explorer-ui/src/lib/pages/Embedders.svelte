<script lang="ts">
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import PageHeader from '$lib/components/PageHeader.svelte';
	import { onMount } from 'svelte';

	interface Embedder {
		embedder_id: number;
		name: string;
		owner: string;
		provider: string;
		base_url: string;
		api_key: string | null;
		config: Record<string, any>;
		max_batch_size?: number;
		dimensions?: number;
		collection_name: string;
		is_public: boolean;
		created_at: string;
		updated_at: string;
	}

	type ProviderDefaultConfig = {
		url: string;
		models: string[];
		inputTypes?: string[];
		embeddingTypes?: string[];
		truncate?: string[];
		config: Record<string, any>;
	};

	let embedders = $state<Embedder[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let showCreateForm = $state(false);
	let editingEmbedder = $state<Embedder | null>(null);

	let searchQuery = $state('');

	let formName = $state('');
	let formProvider = $state('openai');
	let formBaseUrl = $state('https://api.openai.com/v1');
	let formApiKey = $state('');
	let formConfig = $state('{}');
	let formMaxBatchSize = $state(96);
	let formDimensions = $state(1536);
	let formMaxInputTokens = $state(8191);
	let formTruncateStrategy = $state('NONE');

	let testStatus = $state<'idle' | 'testing' | 'success' | 'error'>('idle');
	let testMessage = $state('');

	let localModel = $state('');
	let localInputType = $state('');
	let localDimensions = $state<number | null>(null);
	let customModel = $state('');

	let embedderPendingDelete = $state<Embedder | null>(null);
	let customInputType = $state('');
	let customEmbeddingTypes = $state('');
	let customTruncate = $state('');

	const modelDimensions: Record<string, number> = {
		'text-embedding-3-small': 1536,
		'text-embedding-3-large': 3072,
		'text-embedding-ada-002': 1536,
		'embed-v4.0': 1024,
		'embed-english-v3.0': 1024,
		'embed-multilingual-v3.0': 1024,
		'embed-english-light-v3.0': 384,
		'embed-multilingual-light-v3.0': 384,
		'embed-english-v2.0': 4096,
		'embed-english-light-v2.0': 1024,
		'embed-multilingual-v2.0': 768,
		'sentence-transformers/all-MiniLM-L6-v2': 384,
		'sentence-transformers/all-mpnet-base-v2': 768,
		'sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2': 384,
		'sentence-transformers/distiluse-base-multilingual-cased-v2': 512,
		'BAAI/bge-small-en-v1.5': 384,
		'BAAI/bge-base-en-v1.5': 768,
		'BAAI/bge-large-en-v1.5': 1024,
		'thenlper/gte-small': 384,
		'thenlper/gte-base': 768,
		'thenlper/gte-large': 1024,
	};

	const providerDefaults: Record<string, ProviderDefaultConfig> = {
		openai: {
			url: 'https://api.openai.com/v1',
			models: ['text-embedding-3-small', 'text-embedding-3-large', 'text-embedding-ada-002'],
			config: { model: 'text-embedding-3-small', dimensions: 1536 },
		},
		cohere: {
			url: 'https://api.cohere.com/v2/embed',
			models: [
				'embed-v4.0',
				'embed-english-v3.0',
				'embed-multilingual-v3.0',
				'embed-english-light-v3.0',
				'embed-multilingual-light-v3.0',
				'embed-english-v2.0',
				'embed-english-light-v2.0',
				'embed-multilingual-v2.0',
			],
			inputTypes: ['clustering', 'search_document', 'search_query', 'classification', 'image'],
			embeddingTypes: ['float', 'int8', 'uint8', 'binary', 'ubinary'],
			truncate: ['NONE', 'START', 'END'],
			config: {
				model: 'embed-v4.0',
				input_type: 'clustering',
				embedding_types: ['float'],
				truncate: 'NONE',
				dimensions: 1024,
			},
		},
	};

	async function testEmbedderConnection() {
		testMessage = '';
		try {
			const config = JSON.parse(formConfig);
			const testText = ['Hello world', 'Test embedding'];

			let response: Response;

			if (formProvider === 'openai') {
				response = await fetch(`${formBaseUrl}/embeddings`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${formApiKey}`,
					},
					body: JSON.stringify({
						input: testText,
						model: config.model || 'text-embedding-3-small',
						...(config.dimensions && { dimensions: config.dimensions }),
					}),
				});
			} else if (formProvider === 'cohere') {
				response = await fetch(formBaseUrl, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${formApiKey}`,
					},
					body: JSON.stringify({
						texts: testText,
						model: config.model || 'embed-v4.0',
						...(config.input_type && { input_type: config.input_type }),
						...(config.embedding_types && { embedding_types: config.embedding_types }),
						...(config.truncate && { truncate: config.truncate }),
					}),
				});
			} else if (formProvider === 'huggingface') {
				const model = config.model || 'sentence-transformers/all-MiniLM-L6-v2';
				response = await fetch(`${formBaseUrl}/pipeline/feature-extraction/${model}`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						...(formApiKey && { Authorization: `Bearer ${formApiKey}` }),
					},
					body: JSON.stringify({
						inputs: testText,
					}),
				});
			} else {
				testStatus = 'error';
				testMessage = 'Testing custom providers is not supported. Please save and test manually.';
				return;
			}

			if (!response.ok) {
				const errorText = await response.text();
				testStatus = 'error';
				testMessage = `Test failed (${response.status}): ${errorText}`;
				return;
			}

			const result = await response.json();
			testStatus = 'success';

			let embeddingCount = 0;
			if (formProvider === 'cohere') {
				if (result.embeddings?.float) {
					embeddingCount = result.embeddings.float.length;
				} else if (result.embeddings?.int8) {
					embeddingCount = result.embeddings.int8.length;
				} else if (result.embeddings?.uint8) {
					embeddingCount = result.embeddings.uint8.length;
				} else if (result.embeddings?.binary) {
					embeddingCount = result.embeddings.binary.length;
				} else if (result.embeddings?.ubinary) {
					embeddingCount = result.embeddings.ubinary.length;
				}
			} else if (formProvider === 'openai') {
				embeddingCount = result.data?.length || 0;
			}

			testMessage = `Connection successful! Generated ${embeddingCount} embedding(s).`;
		} catch (e: any) {
			testStatus = 'error';
			testMessage = e.message || 'Test failed.';
		}
	}

	onMount(() => {
		fetchEmbedders();

		// Check for name parameter in URL to pre-filter search
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);
			const nameParam = urlParams.get('name');

			if (nameParam) {
				searchQuery = nameParam;
				// Clean up URL after setting search
				const basePath = hashParts[0];
				window.history.replaceState(
					null,
					'',
					window.location.pathname + window.location.search + basePath
				);
			}
		}
	});

	async function fetchEmbedders() {
		loading = true;
		error = null;
		try {
			const response = await fetch('/api/embedders');
			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to fetch embedders:', errorText);
				throw new Error(`Failed to fetch embedders: ${response.status}`);
			}
			embedders = await response.json();
		} catch (e: any) {
			console.error('Error fetching embedders:', e);
			error = e.message || 'Failed to load embedders';
		} finally {
			loading = false;
		}
	}

	function openCreateForm() {
		editingEmbedder = null;
		formName = '';
		formProvider = 'cohere';
		formApiKey = '';
		updateProviderDefaults();

		testStatus = 'idle';
		testMessage = '';
		showCreateForm = true;
	}

	$effect(() => {
		if (showCreateForm && !editingEmbedder && !formName) {
			const model = localModel === '__custom__' ? customModel : localModel;
			if (model) {
				const cleanModel = model.split('/').pop()?.toLowerCase() || model.toLowerCase();
				formName = `embedders-${formProvider}-${cleanModel}`;
			}
		}
	});

	$effect(() => {
		if (showCreateForm && !editingEmbedder && formName.startsWith('embedders-')) {
			const model = localModel === '__custom__' ? customModel : localModel;
			if (model) {
				const cleanModel = model.split('/').pop()?.toLowerCase() || model.toLowerCase();
				formName = `embedders-${formProvider}-${cleanModel}`;
			}
		}
	});

	function openEditForm(embedder: Embedder) {
		editingEmbedder = embedder;
		formName = embedder.name;
		formProvider = embedder.provider;
		formBaseUrl = embedder.base_url;
		formApiKey = embedder.api_key || '';
		formConfig = JSON.stringify(embedder.config, null, 2);
		formMaxBatchSize = embedder.max_batch_size ?? 96;
		formDimensions = embedder.dimensions ?? 1536;
		formMaxInputTokens = (embedder as any).max_input_tokens ?? 8191;
		formTruncateStrategy = (embedder as any).truncate_strategy ?? 'NONE';
		try {
			const cfg = embedder.config || {};
			const defaults = providerDefaults[formProvider] || {};

			if (cfg.model && defaults.models?.includes(cfg.model)) {
				localModel = cfg.model;
				customModel = '';
			} else if (cfg.model) {
				localModel = '__custom__';
				customModel = cfg.model;
			} else {
				localModel = defaults.models?.[0] || '';
				customModel = '';
			}

			localDimensions = cfg.dimensions || null;

			if (cfg.input_type && defaults.inputTypes?.includes(cfg.input_type)) {
				localInputType = cfg.input_type;
				customInputType = '';
			} else if (cfg.input_type) {
				localInputType = '__custom__';
				customInputType = cfg.input_type;
			} else {
				localInputType = defaults.inputTypes?.[0] || '';
				customInputType = '';
			}

			customEmbeddingTypes = cfg.embedding_types ? cfg.embedding_types.join(', ') : '';
			if (cfg.truncate && defaults.truncate?.includes(cfg.truncate)) {
				customTruncate = cfg.truncate;
			} else if (cfg.truncate) {
				customTruncate = '__custom__';
			} else {
				customTruncate = '';
			}
		} catch {
			localModel = '';
			localInputType = '';
			localDimensions = null;
			customModel = '';
			customInputType = '';
			customEmbeddingTypes = '';
			customTruncate = '';
		}
		showCreateForm = true;
	}

	function updateProviderDefaults() {
		const defaults = providerDefaults[formProvider];
		if (defaults) {
			formBaseUrl = defaults.url;
			formConfig = JSON.stringify(defaults.config, null, 2);
			localModel = defaults.models?.[0] || '';
			localInputType = defaults.inputTypes?.[0] || '';
			localDimensions =
				defaults.config.dimensions || (localModel && modelDimensions[localModel]) || null;
			formMaxBatchSize = 96;
			formDimensions = localDimensions ?? 1536;
			customModel = '';
			customInputType = '';
			customEmbeddingTypes = defaults.config.embedding_types
				? defaults.config.embedding_types.join(', ')
				: '';
			customTruncate = defaults.config.truncate || '';
		}
	}

	async function saveEmbedder() {
		error = null;
		try {
			const config = JSON.parse(formConfig);
			const body: any = {
				name: formName,
				provider: formProvider,
				base_url: formBaseUrl,
				api_key: formApiKey || null,
				config,
				batch_size: editingEmbedder ? undefined : 50,
				max_batch_size: formMaxBatchSize,
				dimensions: formDimensions,
				max_input_tokens: formMaxInputTokens,
				truncate_strategy: formTruncateStrategy,
			};

			const url = editingEmbedder
				? `/api/embedders/${editingEmbedder.embedder_id}`
				: '/api/embedders';
			const method = editingEmbedder ? 'PATCH' : 'POST';

			const response = await fetch(url, {
				method,
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to save embedder:', errorText);
				throw new Error(`Failed to save embedder: ${response.status}`);
			}

			showCreateForm = false;
			await fetchEmbedders();
		} catch (e: any) {
			console.error('Error saving embedder:', e);
			error = e.message || 'Failed to save embedder';
		}
	}

	function requestDeleteEmbedder(embedder: Embedder) {
		embedderPendingDelete = embedder;
	}

	async function confirmDeleteEmbedder() {
		if (!embedderPendingDelete) return;

		const id = embedderPendingDelete.embedder_id;
		embedderPendingDelete = null;
		error = null;

		try {
			const response = await fetch(`/api/embedders/${id}`, { method: 'DELETE' });
			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to delete embedder:', errorText);
				throw new Error(`Failed to delete embedder: ${response.status}`);
			}
			await fetchEmbedders();
		} catch (e: any) {
			console.error('Error deleting embedder:', e);
			error = e.message || 'Failed to delete embedder';
		}
	}

	let filteredEmbedders = $derived(
		embedders.filter((e) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return (
				e.name.toLowerCase().includes(query) ||
				e.provider.toLowerCase().includes(query) ||
				e.owner.toLowerCase().includes(query) ||
				e.base_url.toLowerCase().includes(query)
			);
		})
	);
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Embedders"
		description="Provides embedding provider instances that are user-managed. Define OpenAI or Cohere compatible embedders that can be used on dataset transforms to produce vector embeddings for semantic search."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Embedders</h1>
		<button
			onclick={() => {
				if (showCreateForm) {
					showCreateForm = false;
					editingEmbedder = null;
					testStatus = 'idle';
					testMessage = '';
				} else {
					openCreateForm();
				}
			}}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Embedder'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingEmbedder ? 'Edit Embedder' : 'Create New Embedder'}
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					saveEmbedder();
				}}
			>
				<div class="space-y-4">
					<div>
						<label
							for="embedder-name"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Name
						</label>
						<input
							id="embedder-name"
							type="text"
							bind:value={formName}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							placeholder="My Embedder"
						/>
					</div>
					<div>
						<div>
							<label
								for="embedder-provider"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Provider
							</label>
							<select
								id="embedder-provider"
								bind:value={formProvider}
								onchange={updateProviderDefaults}
								disabled={!!editingEmbedder}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50"
							>
								<option value="openai">OpenAI</option>
								<option value="cohere">Cohere</option>

								<option value="custom">Custom</option>
							</select>
						</div>

						<div>
							<label
								for="embedder-base-url"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Base URL
							</label>
							<input
								id="embedder-base-url"
								type="text"
								bind:value={formBaseUrl}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								placeholder={providerDefaults[formProvider]?.url || ''}
							/>
						</div>

						<div>
							<label
								for="embedder-model"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Model
							</label>
							<select
								id="embedder-model"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								bind:value={localModel}
								onchange={(e) => {
									const value = (e.target as HTMLSelectElement).value;
									localModel = value;
									let config: Record<string, any> = {};
									try {
										config = JSON.parse(formConfig);
									} catch {
										// Ignore parsing errors, use empty config
									}
									if (value !== '__custom__') {
										config['model'] = value;
										if (modelDimensions[value]) {
											config['dimensions'] = modelDimensions[value];
											localDimensions = modelDimensions[value];
										}
										formConfig = JSON.stringify(config, null, 2);
									}
								}}
							>
								{#if providerDefaults[formProvider]?.models}
									{#each providerDefaults[formProvider].models as model (model)}
										<option value={model}>{model}</option>
									{/each}
									<option value="__custom__">Custom...</option>
								{:else}
									<option value="__custom__">Custom...</option>
								{/if}
							</select>
							{#if localModel === '__custom__'}
								<input
									type="text"
									class="w-full px-3 py-2 mt-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									bind:value={customModel}
									oninput={(e) => {
										const value = (e.target as HTMLInputElement).value;
										customModel = value;
										let config: Record<string, any> = {};
										try {
											config = JSON.parse(formConfig);
										} catch {
											// Ignore parsing errors, use empty config
										}
										config['model'] = value;
										formConfig = JSON.stringify(config, null, 2);
									}}
									placeholder="Enter custom model name"
								/>
							{/if}
						</div>

						<div>
							<label
								for="embedder-max-batch-size"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Max Batch Size
							</label>
							<input
								id="embedder-max-batch-size"
								type="number"
								bind:value={formMaxBatchSize}
								min="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								placeholder="e.g., 96"
							/>
							<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								Default: 96 (OpenAI/Cohere)
							</div>
						</div>
						<div>
							<label
								for="embedder-dimensions"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Dimensions
							</label>
							<input
								id="embedder-dimensions"
								type="number"
								bind:value={formDimensions}
								min="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								placeholder="e.g., 384, 768, 1536"
							/>
							<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								{#if localModel && localModel !== '__custom__' && modelDimensions[localModel]}
									Default for {localModel}: {modelDimensions[localModel]}
								{:else}
									Enter embedding vector dimensions for this model
								{/if}
							</div>
						</div>

						<div>
							<label
								for="embedder-max-input-tokens"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Max Input Tokens
							</label>
							<input
								id="embedder-max-input-tokens"
								type="number"
								bind:value={formMaxInputTokens}
								min="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								placeholder="e.g., 8191"
							/>
							<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								Maximum tokens accepted by this embedder model
							</div>
						</div>

						<div>
							<label
								for="embedder-truncate-strategy"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Truncate Strategy
							</label>
							<select
								id="embedder-truncate-strategy"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								bind:value={formTruncateStrategy}
							>
								<option value="NONE">NONE - Return error if text exceeds max_input_tokens</option>
								<option value="START">START - Truncate from beginning, keep end</option>
								<option value="END">END - Truncate from end, keep beginning</option>
							</select>
							<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								How to handle text longer than max_input_tokens
							</div>
						</div>

						{#if providerDefaults[formProvider]?.inputTypes}
							<div>
								<label
									for="embedder-input-type"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Input Type
								</label>
								<select
									id="embedder-input-type"
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									bind:value={localInputType}
									onchange={(e) => {
										const value = (e.target as HTMLSelectElement).value;
										localInputType = value;
										let config: Record<string, any> = {};
										try {
											config = JSON.parse(formConfig);
										} catch {
											// Ignore parsing errors, use empty config
										}
										if (value !== '__custom__') {
											config['input_type'] = value;
											formConfig = JSON.stringify(config, null, 2);
										}
									}}
								>
									{#each providerDefaults[formProvider].inputTypes as inputType (inputType)}
										<option value={inputType}>{inputType}</option>
									{/each}
									<option value="__custom__">Custom...</option>
								</select>
								{#if localInputType === '__custom__'}
									<input
										type="text"
										class="w-full px-3 py-2 mt-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
										bind:value={customInputType}
										oninput={(e) => {
											const value = (e.target as HTMLInputElement).value;
											customInputType = value;
											let config: Record<string, any> = {};
											try {
												config = JSON.parse(formConfig);
											} catch {
												// Ignore parsing errors, use empty config
											}
											config['input_type'] = value;
											formConfig = JSON.stringify(config, null, 2);
										}}
										placeholder="Enter custom input type"
									/>
								{/if}
							</div>
						{/if}

						{#if providerDefaults[formProvider]?.embeddingTypes}
							<div>
								<label
									for="embedder-embedding-type"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Embedding Types <span class="text-xs text-gray-500">(comma-separated)</span>
								</label>
								<input
									id="embedder-embedding-type"
									type="text"
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									bind:value={customEmbeddingTypes}
									oninput={(e) => {
										const value = (e.target as HTMLInputElement).value;
										customEmbeddingTypes = value;
										let config: Record<string, any> = {};
										try {
											config = JSON.parse(formConfig);
										} catch {
											// Ignore parsing errors, use empty config
										}
										config['embedding_types'] = value
											.split(',')
											.map((v) => v.trim())
											.filter((v) => v);
										formConfig = JSON.stringify(config, null, 2);
									}}
									placeholder="e.g. float, int8, uint8"
								/>
								<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
									Available: {providerDefaults[formProvider]?.embeddingTypes?.join(', ') || ''}
								</div>
							</div>
						{/if}

						{#if providerDefaults[formProvider]?.truncate}
							<div>
								<label
									for="embedder-truncate"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Truncate
								</label>
								<select
									id="embedder-truncate"
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									bind:value={customTruncate}
									onchange={(e) => {
										const value = (e.target as HTMLSelectElement).value;
										customTruncate = value;
										let config: Record<string, any> = {};
										try {
											config = JSON.parse(formConfig);
										} catch {
											// Ignore parsing errors, use empty config
										}
										if (value && value !== '__custom__') {
											config['truncate'] = value;
											formConfig = JSON.stringify(config, null, 2);
										}
									}}
								>
									<option value="">-- Select --</option>
									{#each providerDefaults[formProvider].truncate as truncOpt (truncOpt)}
										<option value={truncOpt}>{truncOpt}</option>
									{/each}
									<option value="__custom__">Custom...</option>
								</select>
								{#if customTruncate === '__custom__'}
									<input
										type="text"
										class="w-full px-3 py-2 mt-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
										oninput={(e) => {
											const value = (e.target as HTMLInputElement).value;
											let config: Record<string, any> = {};
											try {
												config = JSON.parse(formConfig);
											} catch {
												// Ignore parsing errors, use empty config
											}
											config['truncate'] = value;
											formConfig = JSON.stringify(config, null, 2);
										}}
										placeholder="Enter custom truncate value"
									/>
								{/if}
							</div>
						{/if}

						<div>
							<label
								for="embedder-api-key"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								API Key (optional)
							</label>
							<input
								id="embedder-api-key"
								type="password"
								bind:value={formApiKey}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								placeholder="Optional"
							/>
						</div>

						<div>
							<label
								for="embedder-config"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Configuration (JSON)
							</label>
							<textarea
								id="embedder-config"
								bind:value={formConfig}
								rows="6"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white font-mono text-sm"
								placeholder={JSON.stringify({
									model: providerDefaults[formProvider]?.models?.[0] || '',
								})}
							></textarea>
						</div>
					</div>

					<div class="mt-6 flex flex-col gap-2">
						{#if testStatus === 'success'}
							<div
								class="p-2 bg-green-100 dark:bg-green-900/20 text-green-800 dark:text-green-300 rounded text-sm"
							>
								{testMessage}
							</div>
						{:else if testStatus === 'error'}
							<div
								class="p-2 bg-red-100 dark:bg-red-900/20 text-red-800 dark:text-red-300 rounded text-sm"
							>
								{testMessage}
							</div>
						{:else if testStatus === 'testing'}
							<div
								class="p-2 bg-blue-100 dark:bg-blue-900/20 text-blue-800 dark:text-blue-300 rounded text-sm"
							>
								Testing connection...
							</div>
						{/if}
						<div class="flex gap-3">
							<button
								type="submit"
								class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
							>
								{editingEmbedder ? 'Update' : 'Create'}
							</button>
							<button
								type="button"
								onclick={testEmbedderConnection}
								class="px-4 py-2 bg-yellow-500 text-white rounded-lg hover:bg-yellow-600 transition-colors"
								disabled={testStatus === 'testing'}
							>
								Test Connection
							</button>
							<button
								type="button"
								onclick={() => {
									showCreateForm = false;
									editingEmbedder = null;
									testStatus = 'idle';
									testMessage = '';
								}}
								class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
							>
								Cancel
							</button>
						</div>
					</div>
				</div>
			</form>
		</div>
	{/if}

	{#if !showCreateForm && embedders.length > 0}
		<div class="mb-4">
			<div class="relative">
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search embedders by name, provider, owner, or URL..."
					class="w-full px-4 py-2 pl-10 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
				/>
				<svg
					class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400"
					fill="none"
					stroke="currentColor"
					viewBox="0 0 24 24"
				>
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
					/>
				</svg>
			</div>
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
			<button
				onclick={fetchEmbedders}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if embedders.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No embedders yet</p>
			<button
				onclick={() => openCreateForm()}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Create your first embedder
			</button>
		</div>
	{:else if filteredEmbedders.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No embedders match your search</p>
			<button
				onclick={() => (searchQuery = '')}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Clear search
			</button>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredEmbedders as embedder (embedder.embedder_id)}
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 hover:shadow-lg transition-shadow"
				>
					<div class="flex justify-between items-start">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h3 class="text-xl font-semibold text-gray-900 dark:text-white">
									{embedder.name}
								</h3>
								<span class="text-sm text-gray-500 dark:text-gray-400">
									#{embedder.embedder_id}
								</span>
							</div>
							<div class="flex items-center gap-2 flex-wrap mb-3">
								<span
									class="inline-flex items-center gap-1.5 px-3 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded-full text-sm font-medium"
								>
									{embedder.provider}
								</span>
								<span
									class="inline-flex items-center gap-1.5 px-3 py-1 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-full text-sm"
								>
									<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
										></path>
									</svg>
									{embedder.owner}
								</span>
							</div>
							<div class="space-y-2 text-sm">
								<div>
									<span class="text-gray-500 dark:text-gray-400">URL:</span>
									<span class="ml-2 text-gray-900 dark:text-gray-100 break-all"
										>{embedder.base_url}</span
									>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">API Key:</span>
									<span class="ml-2 text-gray-900 dark:text-gray-100">
										{embedder.api_key ? '••••••••' : 'Not set'}
									</span>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Max Batch Size:</span>
									<span class="ml-2 text-gray-900 dark:text-gray-100"
										>{embedder.max_batch_size ?? 96}</span
									>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Dimensions:</span>
									<span class="ml-2 text-gray-900 dark:text-gray-100"
										>{embedder.dimensions ?? 1536}</span
									>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Config:</span>
									<pre
										class="mt-1 p-2 bg-gray-100 dark:bg-gray-900 rounded text-xs overflow-auto">{JSON.stringify(
											embedder.config,
											null,
											2
										)}</pre>
								</div>
							</div>
						</div>
						<div class="ml-4 flex flex-col gap-2">
							<button
								onclick={() => openEditForm(embedder)}
								class="px-3 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700 transition-colors flex items-center gap-2 dark:bg-gray-500 dark:hover:bg-gray-600"
								title="Edit embedder"
							>
								<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
									/>
								</svg>
								Edit
							</button>
							<button
								onclick={() => requestDeleteEmbedder(embedder)}
								class="px-3 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors flex items-center gap-2"
								title="Delete embedder"
							>
								<svg
									class="w-4 h-4"
									fill="none"
									stroke="currentColor"
									viewBox="0 0 24 24"
									aria-hidden="true"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
									/>
								</svg>
								Delete
							</button>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={embedderPendingDelete !== null}
	title="Delete embedder"
	message={embedderPendingDelete
		? `Are you sure you want to delete "${embedderPendingDelete.name}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteEmbedder}
	on:cancel={() => (embedderPendingDelete = null)}
/>
