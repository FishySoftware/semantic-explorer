<script lang="ts">
	import ActionMenu from '$lib/components/ActionMenu.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import PageHeader from '$lib/components/PageHeader.svelte';
	import SearchInput from '$lib/components/SearchInput.svelte';
	import type { Embedder, PaginatedResponse, ProviderDefaultConfig } from '$lib/types/models';
	import { formatError, toastStore } from '$lib/utils/notifications';
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';

	let { onViewEmbedder: handleViewEmbedder } = $props<{
		onViewEmbedder?: (_: number) => void;
	}>();

	const onViewEmbedder = (id: number) => {
		handleViewEmbedder?.(id);
	};

	let embedders = $state<Embedder[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let totalCount = $state(0);
	let currentOffset = $state(0);
	const pageSize = 20;
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
	let inferenceModels = $state<string[]>([]);
	let formError = $state<string | null>(null);
	let saving = $state(false);
	let inferenceModelDimensions = $state<Record<string, number>>({});
	let localModelsForDisplay = $derived([...inferenceModels].sort((a, b) => a.localeCompare(b)));

	// Selection state for bulk operations
	let selected = new SvelteSet<number>();
	let selectAll = $state(false);
	let embeddersPendingBulkDelete = $state<Embedder[]>([]);

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
			internal: {
				url: '', // URL is configured on the backend
				models: localModelsForDisplay,
				config: { model: localDefaultModel, dimensions: localDefaultDimensions },
			},
		};
	}

	let providerDefaults = $derived(getProviderDefaults());

	async function fetchInferenceModels() {
		try {
			const response = await fetch('/api/embedding-inference/models');
			if (!response.ok) {
				console.error('Failed to fetch inference models:', response.statusText);
				return;
			}
			const embedderModels: any[] = await response.json();
			inferenceModels = [...new Set(embedderModels.map((m: any) => m.id))].sort();
			const dimMap: Record<string, number> = {};
			for (const model of embedderModels) {
				if (model.dimensions) {
					dimMap[model.id] = model.dimensions;
				}
			}
			inferenceModelDimensions = dimMap;
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
			} else if (formProvider === 'internal') {
				return; // Testing internal embedders is not needed
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
			} else if (formProvider === 'internal') {
				return; // Testing internal embedders is not needed
			}

			testMessage = `Connection successful! Generated ${embeddingCount} embedding(s).`;
		} catch (e: any) {
			testStatus = 'error';
			testMessage = e.message || 'Test failed.';
		}
	}

	function extractSearchParamFromHash() {
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new SvelteURLSearchParams(hashParts[1]);
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

	let hasMount = false;

	onMount(() => {
		fetchEmbedders();
		fetchInferenceModels();
		extractSearchParamFromHash();
		hasMount = true;
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
			const params = new SvelteURLSearchParams();
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			params.append('limit', pageSize.toString());
			params.append('offset', currentOffset.toString());
			const url = params.toString() ? `/api/embedders?${params.toString()}` : '/api/embedders';
			const response = await fetch(url);
			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to fetch embedders:', errorText);
				throw new Error(`Failed to fetch embedders: ${response.status}`);
			}
			const data: PaginatedResponse<Embedder> = await response.json();
			embedders = data.items;
			totalCount = data.total_count;
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
		formProvider = 'internal';
		formApiKey = '';
		formIsPublic = false;
		formError = null;
		updateProviderDefaults();

		testStatus = 'idle';
		testMessage = '';
		showCreateForm = true;
	}

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
			if (formProvider === 'internal') {
				// For internal provider, use the first available inference model
				localModel = defaults.models?.[0] || '';
				customModel = '';
			} else {
				// For external providers, use provider's default model for name auto-generation
				localModel = defaults.models?.[0] || '';
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
			} else if (formProvider === 'internal') {
				formBatchSize = 128;
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
		formError = null;

		// Client-side validation
		if (!formName.trim()) {
			formError = 'Name is required.';
			return;
		}

		let config: Record<string, any>;
		try {
			config = JSON.parse(formConfig);
		} catch {
			formError = 'Configuration must be valid JSON.';
			return;
		}

		if (!config.model) {
			formError = 'A model must be selected or specified in the configuration.';
			return;
		}

		if (formProvider !== 'internal' && !formBaseUrl.trim()) {
			formError = 'Base URL is required for external providers.';
			return;
		}

		saving = true;
		try {
			const body: any = {
				name: formName.trim(),
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
				let detail = '';
				try {
					const errorBody = await response.json();
					detail =
						errorBody.error || errorBody.message || errorBody.detail || JSON.stringify(errorBody);
				} catch {
					detail = await response.text();
				}
				const msg = detail
					? `Failed to save embedder (${response.status}): ${detail}`
					: `Failed to save embedder (${response.status})`;
				formError = msg;
				toastStore.error(msg);
				return;
			}

			showCreateForm = false;
			toastStore.success(`Embedder ${editingEmbedder ? 'updated' : 'created'} successfully!`);
			await fetchEmbedders();
		} catch (e: any) {
			console.error('Error saving embedder:', e);
			const msg = formatError(e, 'Failed to save embedder');
			formError = msg;
			toastStore.error(msg);
		} finally {
			saving = false;
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
				const body = await response.json().catch(() => null);
				console.error('Failed to delete embedder:', body);
				throw new Error(body?.message || `Failed to delete embedder: ${response.status}`);
			}
			toastStore.success('Embedder deleted');
			await fetchEmbedders();
		} catch (e: any) {
			console.error('Error deleting embedder:', e);
			const message = formatError(e, 'Failed to delete embedder');
			error = message;
			toastStore.error(message);
		}
	}

	function toggleSelectAll() {
		selectAll = !selectAll;
		if (selectAll) {
			selected.clear();
			for (const embedder of embedders) {
				selected.add(embedder.embedder_id);
			}
		} else {
			selected.clear();
		}
	}

	function toggleSelect(id: number) {
		if (selected.has(id)) {
			selected.delete(id);
			selectAll = false;
		} else {
			selected.add(id);
		}
	}

	function bulkDelete() {
		const toDelete: Embedder[] = [];
		for (const id of selected) {
			const embedder = embedders.find((e) => e.embedder_id === id);
			if (embedder) {
				toDelete.push(embedder);
			}
		}
		if (toDelete.length > 0) {
			embeddersPendingBulkDelete = toDelete;
		}
	}

	async function confirmBulkDelete() {
		const toDelete = embeddersPendingBulkDelete;
		embeddersPendingBulkDelete = [];

		let deleted = 0;
		for (const embedder of toDelete) {
			try {
				const response = await fetch(`/api/embedders/${embedder.embedder_id}`, {
					method: 'DELETE',
				});

				if (!response.ok) {
					const body = await response.json().catch(() => null);
					throw new Error(body?.message || `Failed to delete: ${response.statusText}`);
				}

				embedders = embedders.filter((e) => e.embedder_id !== embedder.embedder_id);
				deleted++;
			} catch (e) {
				toastStore.error(formatError(e, `Failed to delete "${embedder.name}"`));
			}
		}

		selected.clear();
		selectAll = false;
		if (deleted > 0) {
			toastStore.success(`Deleted ${deleted} embedder${deleted !== 1 ? 's' : ''}`);
		}
	}

	// Refetch when search query changes
	// Debounce search to avoid spamming API on every keystroke
	let searchDebounceTimeout: ReturnType<typeof setTimeout> | null = null;
	let previousSearchQuery = '';

	$effect(() => {
		// Only trigger fetch if we've mounted and search query actually changed
		if (hasMount && searchQuery !== previousSearchQuery) {
			previousSearchQuery = searchQuery;
			currentOffset = 0; // Reset to first page when searching
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
			searchDebounceTimeout = setTimeout(() => {
				fetchEmbedders();
			}, 300); // 300ms debounce
		}
		return () => {
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
		};
	});

	function goToPreviousPage() {
		currentOffset = Math.max(0, currentOffset - pageSize);
		fetchEmbedders();
	}

	function goToNextPage() {
		if (currentOffset + pageSize < totalCount) {
			currentOffset += pageSize;
			fetchEmbedders();
		}
	}
</script>

<div class="mx-auto">
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
							required
							class="w-full px-3 py-2 border rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white {!formName.trim() &&
							formError
								? 'border-red-500 dark:border-red-500'
								: 'border-gray-300 dark:border-gray-600'}"
							placeholder="Enter a name for this embedder"
						/>
						{#if !formName.trim() && formError}
							<p class="mt-1 text-sm text-red-600 dark:text-red-400">Name is required</p>
						{/if}
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
								<option value="internal">Embedding Inference API</option>
								<option value="openai">OpenAI</option>
								<option value="cohere">Cohere</option>
							</select>
						</div>

						{#if formProvider === 'internal'}
							<div
								class="pt-3 pb-3 mt-2 mb-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg"
							>
								<p class=" p-2 text-sm text-blue-700 dark:text-blue-300">
									<strong>Internal Embedding Inference API:</strong>
									The embedding inference API URL is configured on the server. No API key is required.
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
										}
										formConfig = JSON.stringify(config, null, 2);
									}
								}}
							>
								{#if providerDefaults[formProvider]?.models}
									{#each providerDefaults[formProvider].models as model (model)}
										<option value={model}>
											{model}
										</option>
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
								API Key {formProvider === 'internal'
									? '(not required for internal embedding inference API)'
									: '(optional)'}
							</label>
							<input
								id="embedder-api-key"
								type="password"
								autocomplete="off"
								bind:value={formApiKey}
								disabled={formProvider === 'internal'}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
								placeholder={formProvider === 'internal' ? 'Not required' : 'Optional'}
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
						{#if formError}
							<div
								class="p-3 bg-red-100 dark:bg-red-900/20 border border-red-300 dark:border-red-700 text-red-800 dark:text-red-300 rounded-lg text-sm"
							>
								{formError}
							</div>
						{/if}
						<div class="flex gap-3">
							<button type="submit" class="btn-primary" disabled={saving}>
								{#if saving}
									Saving...
								{:else}
									{editingEmbedder ? 'Update' : 'Create'}
								{/if}
							</button>
							{#if formProvider !== 'internal'}
								<button
									type="button"
									onclick={testEmbedderConnection}
									class="px-4 py-2 bg-yellow-500 hover:bg-yellow-600 text-white font-semibold rounded-lg shadow-md transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-yellow-400 focus:ring-offset-2 dark:focus:ring-offset-gray-900 disabled:opacity-50 disabled:cursor-not-allowed"
									disabled={testStatus === 'testing'}
								>
									Test Connection
								</button>
							{/if}
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

	{#if !showCreateForm}
		<SearchInput
			bind:value={searchQuery}
			placeholder="Search embedders by name, provider, owner, or URL..."
		/>
	{/if}

	{#if loading}
		<LoadingState message="Loading embedders..." />
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
			<p class="text-gray-500 dark:text-gray-400 mb-4">
				{#if searchQuery.trim()}
					No embedders match your search
				{:else}
					No embedders yet
				{/if}
			</p>
			{#if searchQuery.trim()}
				<button
					onclick={() => (searchQuery = '')}
					class="text-blue-600 dark:text-blue-400 hover:underline"
				>
					Clear search
				</button>
			{:else}
				<button
					onclick={() => openCreateForm()}
					class="text-blue-600 dark:text-blue-400 hover:underline"
				>
					Create your first embedder
				</button>
			{/if}
		</div>
	{:else}
		{#if selected.size > 0}
			<div
				class="mb-4 flex items-center gap-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4"
			>
				<span class="text-sm text-blue-700 dark:text-blue-300 flex-1">
					{selected.size} embedder{selected.size !== 1 ? 's' : ''} selected
				</span>
				<button
					onclick={() => bulkDelete()}
					class="text-sm px-3 py-1 rounded bg-red-600 hover:bg-red-700 text-white transition-colors"
				>
					Delete
				</button>
				<button
					onclick={() => {
						selected.clear();
						selectAll = false;
					}}
					class="text-sm px-3 py-1 rounded bg-gray-300 hover:bg-gray-400 dark:bg-gray-600 dark:hover:bg-gray-500 text-gray-900 dark:text-white transition-colors"
				>
					Clear
				</button>
			</div>
		{/if}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 w-12">
						<input
							type="checkbox"
							checked={selectAll}
							onchange={() => toggleSelectAll()}
							class="cursor-pointer"
						/>
					</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Name</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Provider</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Model</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center"
						>Dimensions</TableHeadCell
					>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Public</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each embedders as embedder (embedder.embedder_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(embedder.embedder_id)}
									onchange={() => toggleSelect(embedder.embedder_id)}
									class="cursor-pointer"
								/>
							</TableBodyCell>
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
								{#if embedder.is_public}
									<span
										class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-sm font-medium"
									>
										Yes
									</span>
								{:else}
									<span
										class="inline-block px-2 py-1 bg-gray-100 dark:bg-gray-700/30 text-gray-500 dark:text-gray-400 rounded text-sm font-medium"
									>
										No
									</span>
								{/if}
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

			<!-- Pagination Controls -->
			<div class="mt-6 px-4 pb-4 flex items-center justify-between">
				<div class="text-sm text-gray-600 dark:text-gray-400">
					Showing {currentOffset + 1}-{Math.min(currentOffset + pageSize, totalCount)} of {totalCount}
					embedders
				</div>
				<div class="flex gap-2">
					<button
						onclick={goToPreviousPage}
						disabled={currentOffset === 0}
						class="px-4 py-2 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-100 dark:hover:bg-gray-700"
					>
						Previous
					</button>
					<button
						onclick={goToNextPage}
						disabled={currentOffset + pageSize >= totalCount}
						class="px-4 py-2 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-100 dark:hover:bg-gray-700"
					>
						Next
					</button>
				</div>
			</div>
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
	onConfirm={confirmDeleteEmbedder}
	onCancel={() => (embedderPendingDelete = null)}
/>

<ConfirmDialog
	open={embeddersPendingBulkDelete.length > 0}
	title="Delete Embedders"
	message={`Are you sure you want to delete ${embeddersPendingBulkDelete.length} embedder${embeddersPendingBulkDelete.length !== 1 ? 's' : ''}? This action cannot be undone.`}
	confirmLabel="Delete All"
	variant="danger"
	onConfirm={confirmBulkDelete}
	onCancel={() => (embeddersPendingBulkDelete = [])}
/>
