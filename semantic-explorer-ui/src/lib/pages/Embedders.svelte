<script lang="ts">
	import ActionMenu from '$lib/components/ActionMenu.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import PageHeader from '$lib/components/PageHeader.svelte';
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { onMount } from 'svelte';

	let { onViewEmbedder: handleViewEmbedder } = $props<{
		onViewEmbedder?: (_: number) => void;
	}>();

	const onViewEmbedder = (id: number) => {
		handleViewEmbedder?.(id);
	};

	interface Embedder {
		embedder_id: number;
		name: string;
		owner: string;
		provider: string;
		base_url: string;
		api_key: string | null;
		config: Record<string, any>;
		batch_size?: number;
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
	let formBatchSize = $state(100);
	let formDimensions = $state(1536);
	let formMaxInputTokens = $state(8191);
	let formTruncateStrategy = $state('NONE');
	let formIsPublic = $state(false);

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
	let userEditedName = $state(false);
	let inferenceModels = $state<string[]>([]);
	let inferenceModelDimensions = $state<Record<string, number>>({});

	// Dimension detection state
	let dimensionDetectionStatus = $state<'idle' | 'detecting' | 'success' | 'error'>('idle');
	let detectedDimensions = $state<number | null>(null);
	let dimensionSource = $state<string>(''); // 'known', 'detected', 'configured'
	let dimensionMessage = $state<string>('');

	let localModelsForDisplay = $derived([...inferenceModels].sort((a, b) => a.localeCompare(b)));

	function getProviderDefaults(): Record<string, ProviderDefaultConfig> {
		const localDefaultModel = localModelsForDisplay[0] || '';
		const localDefaultDimensions = inferenceModelDimensions[localDefaultModel] || 384;

		return {
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
			local: {
				url: '', // URL is configured on the backend
				models: localModelsForDisplay,
				config: { model: localDefaultModel, dimensions: localDefaultDimensions },
			},
		};
	}

	let providerDefaults = $derived(getProviderDefaults());

	async function fetchInferenceModels() {
		try {
			const response = await fetch('/api/inference/models/embedders');
			if (!response.ok) {
				console.error('Failed to fetch inference models:', response.statusText);
				return;
			}
			const embedderModels = await response.json();
			if (Array.isArray(embedderModels)) {
				// Clear previous models and set new ones
				inferenceModels = [...new Set(embedderModels.map((m: any) => m.id))].sort();
				// Build dimensions map
				const dimMap: Record<string, number> = {};
				for (const model of embedderModels) {
					if (model.dimensions) {
						dimMap[model.id] = model.dimensions;
					}
				}
				inferenceModelDimensions = dimMap;
			}
		} catch (e) {
			console.error('Error fetching inference models:', e);
		}
	}

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
			} else if (formProvider === 'local') {
				// Test local inference through the backend API
				const model = config.model || 'BAAI/bge-small-en-v1.5';
				response = await fetch('/api/inference/test', {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						model,
						texts: testText,
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
			} else if (formProvider === 'local') {
				embeddingCount = result.embeddings?.length || 0;
			}

			testMessage = `Connection successful! Generated ${embeddingCount} embedding(s).`;
		} catch (e: any) {
			testStatus = 'error';
			testMessage = e.message || 'Test failed.';
		}
	}

	async function autoDetectDimensions() {
		if (!formProvider || (formProvider !== 'local' && !formApiKey)) {
			dimensionDetectionStatus = 'error';
			dimensionMessage = 'API key required for dimension detection';
			return;
		}

		try {
			dimensionDetectionStatus = 'detecting';
			dimensionMessage = '';

			const config = JSON.parse(formConfig);

			const response = await fetch('/api/embedders/detect-dimensions', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					provider: formProvider,
					base_url: formBaseUrl,
					api_key: formApiKey || null,
					config: config,
				}),
			});

			if (!response.ok) {
				throw new Error(`HTTP ${response.status}`);
			}

			const result = await response.json();

			if (result.dimensions) {
				detectedDimensions = result.dimensions;
				formDimensions = result.dimensions;
				dimensionSource = result.source;
				dimensionDetectionStatus = 'success';
				dimensionMessage = result.message;

				// Update config with detected dimensions
				const updatedConfig = { ...config, dimensions: result.dimensions };
				formConfig = JSON.stringify(updatedConfig, null, 2);
			} else {
				dimensionDetectionStatus = 'error';
				dimensionMessage = result.message || 'Failed to detect dimensions';
			}
		} catch (e: any) {
			dimensionDetectionStatus = 'error';
			dimensionMessage = e.message || 'Failed to detect dimensions';
		}
	}

	// Check for dimension mismatches
	let dimensionMismatch = $derived.by(() => {
		if (!detectedDimensions || !formDimensions) return null;
		if (detectedDimensions !== formDimensions) {
			return {
				detected: detectedDimensions,
				configured: formDimensions,
			};
		}
		return null;
	});

	function extractSearchParamFromHash() {
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
	}

	onMount(() => {
		fetchEmbedders();
		fetchInferenceModels();
		extractSearchParamFromHash();
	});

	$effect(() => {
		// Re-check for search param when hash changes (e.g., after redirect from create)
		window.location.hash;
		extractSearchParamFromHash();
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
		formProvider = 'local';
		formApiKey = '';
		formIsPublic = false;
		updateProviderDefaults();

		testStatus = 'idle';
		testMessage = '';
		userEditedName = false; // Reset the flag
		showCreateForm = true;
	}

	$effect(() => {
		// Only auto-generate name on initial load when creating (not editing)
		if (showCreateForm && !editingEmbedder && !userEditedName && !formName) {
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
		formBatchSize = embedder.batch_size ?? 100;
		formDimensions = embedder.dimensions ?? 1536;
		formMaxInputTokens = (embedder as any).max_input_tokens ?? 8191;
		formTruncateStrategy = (embedder as any).truncate_strategy ?? 'NONE';
		formIsPublic = embedder.is_public ?? false;
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
		} catch (e) {
			console.error('Failed to parse embedder config:', e);
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

			// Reset model selection when switching providers to avoid accumulation
			if (formProvider === 'local') {
				// For local provider, use the first available inference model
				localModel = defaults.models?.[0] || '';
				customModel = '';
			} else {
				// For external providers, reset local model and use provider's default model
				localModel = '';
				customModel = '';
				// Update the config with the provider's default model
				let config: Record<string, any> = {};
				try {
					config = JSON.parse(formConfig);
				} catch {
					// Ignore parsing errors, use defaults
					config = { ...defaults.config };
				}
				if (defaults.models?.[0]) {
					config['model'] = defaults.models[0];
					formConfig = JSON.stringify(config, null, 2);
				}
			}

			localInputType = defaults.inputTypes?.[0] || '';
			localDimensions =
				defaults.config.dimensions || (localModel && inferenceModelDimensions[localModel]) || null;

			// Set batch size based on provider
			if (formProvider === 'openai') {
				formBatchSize = 2048;
			} else if (formProvider === 'cohere') {
				formBatchSize = 96;
			} else if (formProvider === 'local') {
				formBatchSize = 256;
			} else {
				formBatchSize = 100;
			}

			formDimensions = localDimensions ?? 1536;
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
				batch_size: formBatchSize,
				dimensions: formDimensions,
				max_input_tokens: formMaxInputTokens,
				truncate_strategy: formTruncateStrategy,
				is_public: formIsPublic,
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

			const newEmbedder = await response.json();
			showCreateForm = false;

			if (!editingEmbedder) {
				// Fetch updated list and then redirect to show the new embedder
				await fetchEmbedders();
				window.location.hash = `#/embedders/${newEmbedder.embedder_id}/details`;
			} else {
				await fetchEmbedders();
			}
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

	<div class="flex justify-between items-center mb-4">
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
			class="btn-primary"
		>
			{showCreateForm ? 'Cancel' : 'Create Embedder'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
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
							oninput={() => {
								userEditedName = true;
							}}
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
								<option value="local">Inference API</option>
								<option value="openai">OpenAI</option>
								<option value="cohere">Cohere</option>
							</select>
						</div>

						{#if formProvider === 'local'}
							<div
								class="pt-3 pb-3 mt-2 mb-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg"
							>
								<p class=" p-2 text-sm text-blue-700 dark:text-blue-300">
									<strong>Local Inference:</strong>
									The inference API URL is configured on the server. No API key is required.
								</p>
							</div>
						{:else}
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
						{/if}

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
										const dimensions = inferenceModelDimensions[value];
										if (dimensions) {
											config['dimensions'] = dimensions;
											localDimensions = dimensions;
											formDimensions = dimensions;
											dimensionSource = 'known';
											dimensionMessage = `Known dimensions for ${value}`;
										} else {
											// Reset dimension info for unknown models
											dimensionSource = '';
											dimensionMessage = '';
											detectedDimensions = null;
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
								for="embedder-batch-size"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Batch Size
							</label>
							<input
								id="embedder-batch-size"
								type="number"
								bind:value={formBatchSize}
								min="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
								placeholder="e.g., 100"
							/>
							<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								Number of texts to embed per API call
							</div>
						</div>
						<div>
							<label
								for="embedder-dimensions"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Dimensions
							</label>
							<div class="flex gap-2">
								<input
									id="embedder-dimensions"
									type="number"
									bind:value={formDimensions}
									min="1"
									class="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									placeholder="e.g., 384, 768, 1536"
								/>
								<button
									type="button"
									onclick={autoDetectDimensions}
									disabled={dimensionDetectionStatus === 'detecting' ||
										(formProvider !== 'local' && !formApiKey)}
									class="px-3 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
								>
									{#if dimensionDetectionStatus === 'detecting'}
										Detecting...
									{:else}
										Auto-Detect
									{/if}
								</button>
							</div>
							<div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								{#if dimensionSource === 'known'}
									<span class="text-green-600 dark:text-green-400">{dimensionMessage}</span>
								{:else if dimensionSource === 'detected'}
									<span class="text-blue-600 dark:text-blue-400">{dimensionMessage}</span>
								{:else if localModel && localModel !== '__custom__' && inferenceModelDimensions[localModel]}
									Default for {localModel}: {inferenceModelDimensions[localModel]}
								{:else}
									Enter embedding vector dimensions for this model
								{/if}
							</div>
							{#if dimensionDetectionStatus === 'error'}
								<div class="mt-1 text-xs text-red-600 dark:text-red-400">
									{dimensionMessage}
								</div>
							{/if}
							{#if dimensionMismatch}
								<div
									class="mt-1 text-xs text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded px-2 py-1"
								>
									⚠️ Warning: Configured dimensions ({dimensionMismatch.configured}) don't match
									detected dimensions ({dimensionMismatch.detected})
								</div>
							{/if}
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
								API Key {formProvider === 'local' ? '(not required for local)' : '(optional)'}
							</label>
							<input
								id="embedder-api-key"
								type="password"
								autocomplete="off"
								bind:value={formApiKey}
								disabled={formProvider === 'local'}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
								placeholder={formProvider === 'local' ? 'Not required' : 'Optional'}
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

						<div>
							<label class="flex items-center gap-2 cursor-pointer">
								<input
									type="checkbox"
									bind:checked={formIsPublic}
									class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
								/>
								<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
									Make this embedder public (visible in marketplace)
								</span>
							</label>
						</div>
					</div>

					<div class="mt-4 flex flex-col gap-2">
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
							<button type="submit" class="btn-primary">
								{editingEmbedder ? 'Update' : 'Create'}
							</button>
							<button
								type="button"
								onclick={testEmbedderConnection}
								class="px-4 py-2 bg-yellow-500 hover:bg-yellow-600 text-white font-semibold rounded-lg shadow-md transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-yellow-400 focus:ring-offset-2 dark:focus:ring-offset-gray-900 disabled:opacity-50 disabled:cursor-not-allowed"
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
								class="btn-secondary"
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
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Name</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Provider</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Model</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Dimensions</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Owner</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each filteredEmbedders as embedder (embedder.embedder_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3">
								<button
									onclick={() => onViewEmbedder(embedder.embedder_id)}
									class="font-semibold text-blue-600 dark:text-blue-400 hover:underline"
								>
									{embedder.name}
								</button>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span
									class="inline-block px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-sm font-medium"
								>
									{embedder.provider}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span class="text-gray-700 dark:text-gray-300 text-sm">
									{embedder.config?.model ?? 'N/A'}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								<span class="text-gray-700 dark:text-gray-300 text-sm font-medium">
									{embedder.dimensions ?? embedder.config?.dimensions ?? 1536}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span class="text-gray-700 dark:text-gray-300">{embedder.owner}</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								<ActionMenu
									actions={[
										{
											label: 'View',
											handler: () => onViewEmbedder(embedder.embedder_id),
										},
										{
											label: 'Edit',
											handler: () => openEditForm(embedder),
										},
										{
											label: 'Delete',
											handler: () => requestDeleteEmbedder(embedder),
											isDangerous: true,
										},
									]}
								/>
							</TableBodyCell>
						</tr>
					{/each}
				</TableBody>
			</Table>
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
