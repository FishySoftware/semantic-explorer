<script lang="ts">
	import ActionMenu from '$lib/components/ActionMenu.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import PageHeader from '$lib/components/PageHeader.svelte';
	import type {
		LLM,
		ModelInfo,
		ModelsResponse,
		PaginatedLLMList,
		ProviderDefaultConfig,
	} from '$lib/types/models';
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';

	let llms = $state<LLM[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let totalCount = $state(0);
	let currentOffset = $state(0);
	const pageSize = 20;
	let showCreateForm = $state(false);
	let editingLLM = $state<LLM | null>(null);

	let searchQuery = $state('');

	let formName = $state('');
	let formProvider = $state('openai');
	let formBaseUrl = $state('https://api.openai.com/v1');
	let formApiKey = $state('');
	let formConfig = $state('{}');
	let formIsPublic = $state(false);

	let testStatus = $state<'idle' | 'testing' | 'success' | 'error'>('idle');
	let testMessage = $state('');

	let localModel = $state('');
	let customModel = $state('');

	let llmPendingDelete = $state<LLM | null>(null);

	let inferenceModels = $state<ModelInfo[]>([]);
	let localModelsForDisplay = $derived(
		[...inferenceModels].sort((a, b) => a.name.localeCompare(b.name)),
	);

	function getProviderDefaults(): Record<string, ProviderDefaultConfig> {
		const localDefaultModel = localModelsForDisplay[0]?.id || '';

		return {
			local: {
				url: '', // URL is configured on the backend
				models: localModelsForDisplay.map((m) => m.id),
				config: { model: localDefaultModel },
			},
			openai: {
				url: 'https://api.openai.com/v1',
				models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-4', 'gpt-3.5-turbo'],
				config: { model: 'gpt-4o' },
			},
			cohere: {
				url: 'https://api.cohere.com/v2',
				models: [
					'command-a-03-2025',
					'command-r-plus-08-2024',
					'command-r-08-2024',
					'command-r7b-12-2024',
				],
				config: { model: 'command-a-03-2025' },
			},
		};
	}

	let providerDefaults = $derived(getProviderDefaults());

	async function fetchInferenceModels() {
		try {
			const response = await fetch('/api/llm-inference/models');
			if (!response.ok) {
				console.error('Failed to fetch inference models:', response.statusText);
				return;
			}
			const data = (await response.json()) as ModelsResponse;
			inferenceModels = data.models;
		} catch (e) {
			console.error('Error fetching inference models:', e);
			inferenceModels = [];
		}
	}

	async function testLLMConnection() {
		testStatus = 'testing';
		testMessage = '';
		try {
			const config = JSON.parse(formConfig);
			const testPrompt = 'Say "Hello" in one word.';

			let response: Response;

			if (formProvider === 'openai') {
				response = await fetch(`${formBaseUrl}/chat/completions`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${formApiKey}`,
					},
					body: JSON.stringify({
						model: config.model || 'gpt-4o',
						messages: [{ role: 'user', content: testPrompt }],
						max_tokens: 10,
					}),
				});
			} else if (formProvider === 'local') {
				return; // Testing local LLMs is not needed
			} else if (formProvider === 'cohere') {
				response = await fetch(`${formBaseUrl}/chat`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${formApiKey}`,
					},
					body: JSON.stringify({
						model: config.model || 'command-a-03-2025',
						messages: [{ role: 'user', content: testPrompt }],
						max_tokens: 10,
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

			let responseText = '';
			if (formProvider === 'openai') {
				responseText = result.choices?.[0]?.message?.content || '';
			} else if (formProvider === 'cohere') {
				responseText =
					result.message?.content?.[0]?.text || result.text || result.generations?.[0]?.text || '';
			}

			testMessage = `Connection successful! Response: "${responseText.trim()}"`;
		} catch (e: any) {
			testStatus = 'error';
			testMessage = e.message || 'Test failed.';
		}
	}

	function extractSearchParamFromHash() {
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);
			const nameParam = urlParams.get('name');

			if (nameParam) {
				searchQuery = nameParam;
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
		fetchInferenceModels();
		fetchLLMs();
		extractSearchParamFromHash();
	});

	$effect(() => {
		// Re-check for search param when hash changes (e.g., after redirect from create)
		window.location.hash;
		extractSearchParamFromHash();
	});

	async function fetchLLMs() {
		loading = true;
		error = null;
		try {
			const params = new SvelteURLSearchParams();
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			params.append('limit', pageSize.toString());
			params.append('offset', currentOffset.toString());
			const url = params.toString() ? `/api/llms?${params.toString()}` : '/api/llms';
			const response = await fetch(url);
			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to fetch LLMs:', errorText);
				throw new Error(`Failed to fetch LLMs: ${response.status}`);
			}
			const data: PaginatedLLMList = await response.json();
			llms = data.items;
			totalCount = data.total_count;
		} catch (e: any) {
			console.error('Error fetching LLMs:', e);
			error = e.message || 'Failed to load LLMs';
		} finally {
			loading = false;
		}
	}

	function openCreateForm() {
		editingLLM = null;
		formName = '';
		formProvider = 'local';
		formApiKey = '';
		formIsPublic = false;
		updateProviderDefaults();

		testStatus = 'idle';
		testMessage = '';
		showCreateForm = true;
	}

	$effect(() => {
		if (showCreateForm && !editingLLM && !formName) {
			const model = localModel === '__custom__' ? customModel : localModel;
			if (model && typeof model === 'string') {
				const cleanModel = model.split('/').pop()?.toLowerCase() || model.toLowerCase();
				formName = `llm-${formProvider}-${cleanModel}`;
			}
		}
	});

	$effect(() => {
		if (showCreateForm && !editingLLM && formName.startsWith('llm-')) {
			const model = localModel === '__custom__' ? customModel : localModel;
			if (model && typeof model === 'string') {
				const cleanModel = model.split('/').pop()?.toLowerCase() || model.toLowerCase();
				formName = `llm-${formProvider}-${cleanModel}`;
			}
		}
	});

	function openEditForm(llm: LLM) {
		editingLLM = llm;
		formName = llm.name;
		formProvider = llm.provider;
		formBaseUrl = llm.base_url;
		formApiKey = llm.api_key || '';
		formConfig = JSON.stringify(llm.config, null, 2);
		formIsPublic = llm.is_public;
		try {
			const cfg = llm.config || {};
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
		} catch (e) {
			console.error('Failed to parse LLM config:', e);
			localModel = '';
			customModel = '';
		}
		showCreateForm = true;
	}

	function updateProviderDefaults() {
		const defaults = providerDefaults[formProvider];
		if (defaults) {
			formBaseUrl = defaults.url;
			formConfig = JSON.stringify(defaults.config, null, 2);
			localModel = defaults.models?.[0] || '';
			customModel = '';
		} else {
			formBaseUrl = '';
			formConfig = '{}';
			localModel = '';
			customModel = '';
		}
	}

	async function saveLLM() {
		error = null;
		try {
			const config = JSON.parse(formConfig);

			// Extract model from config - it should be there
			const model = config.model;
			if (!model) {
				throw new Error('Model is required in configuration');
			}

			const body: any = {
				name: formName,
				provider: formProvider,
				model,
				base_url: formBaseUrl,
				api_key: formApiKey || null,
				config,
				is_public: formIsPublic,
			};

			const url = editingLLM ? `/api/llms/${editingLLM.llm_id}` : '/api/llms';
			const method = editingLLM ? 'PATCH' : 'POST';

			const response = await fetch(url, {
				method,
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to save LLM:', errorText);
				throw new Error(`Failed to save LLM: ${response.status}`);
			}

			const newLLM = await response.json();
			showCreateForm = false;

			if (!editingLLM) {
				// Fetch updated list and then redirect to show the new LLM
				await fetchLLMs();
				window.location.hash = `#/llms?name=${encodeURIComponent(newLLM.name)}`;
			} else {
				await fetchLLMs();
			}
		} catch (e: any) {
			console.error('Error saving LLM:', e);
			error = e.message || 'Failed to save LLM';
		}
	}

	function requestDeleteLLM(llm: LLM) {
		llmPendingDelete = llm;
	}

	async function confirmDeleteLLM() {
		if (!llmPendingDelete) return;

		const id = llmPendingDelete.llm_id;
		llmPendingDelete = null;
		error = null;

		try {
			const response = await fetch(`/api/llms/${id}`, { method: 'DELETE' });
			if (!response.ok) {
				const errorText = await response.text();
				console.error('Failed to delete LLM:', errorText);
				throw new Error(`Failed to delete LLM: ${response.status}`);
			}
			await fetchLLMs();
		} catch (e: any) {
			console.error('Error deleting LLM:', e);
			error = e.message || 'Failed to delete LLM';
		}
	}

	// Refetch when search query changes
	// Debounce search to avoid spamming API on every keystroke
	let searchDebounceTimeout: ReturnType<typeof setTimeout> | null = null;
	$effect(() => {
		if (searchQuery !== undefined) {
			currentOffset = 0; // Reset to first page when searching
			if (searchDebounceTimeout) {
				clearTimeout(searchDebounceTimeout);
			}
			searchDebounceTimeout = setTimeout(() => {
				fetchLLMs();
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
		fetchLLMs();
	}

	function goToNextPage() {
		if (currentOffset + pageSize < totalCount) {
			currentOffset += pageSize;
			fetchLLMs();
		}
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="LLMs"
		description="Manage Large Language Model configurations. Define OpenAI, Cohere, or custom LLM providers that can be used for chat, completions, and AI-powered features."
	/>

	<div class="flex justify-between items-center mb-4">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">LLMs</h1>
		<button
			onclick={() => {
				if (showCreateForm) {
					showCreateForm = false;
					editingLLM = null;
					testStatus = 'idle';
					testMessage = '';
				} else {
					openCreateForm();
				}
			}}
			class="btn-primary"
		>
			{showCreateForm ? 'Cancel' : 'Create LLM'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 mb-4">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingLLM ? 'Edit LLM' : 'Create New LLM'}
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					saveLLM();
				}}
			>
				<div class="space-y-4">
					<div>
						<label
							for="llm-name"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Name
						</label>
						<input
							id="llm-name"
							type="text"
							bind:value={formName}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							placeholder="My LLM"
						/>
					</div>
					<div>
						<div>
							<label
								for="llm-provider"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Provider
							</label>
							<select
								id="llm-provider"
								bind:value={formProvider}
								onchange={updateProviderDefaults}
								disabled={!!editingLLM}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50"
							>
								<option value="local">LLM Inference API</option>
								<option value="openai">OpenAI</option>
								<option value="cohere">Cohere</option>
								<option value="custom">Custom</option>
							</select>
						</div>

						{#if formProvider === 'local'}
							<div
								class="pt-3 pb-3 mt-2 mb-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg"
							>
								<p class="p-2 text-sm text-blue-700 dark:text-blue-300">
									<strong>LLM Inference API:</strong>
									The LLM Inference API URL is configured on the server. No API key is required.
								</p>
							</div>
						{:else}
							<div class="mt-4">
								<label
									for="llm-base-url"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Base URL
								</label>
								<input
									id="llm-base-url"
									type="text"
									bind:value={formBaseUrl}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									placeholder={providerDefaults[formProvider]?.url || ''}
								/>
							</div>
						{/if}
						<div class="mt-4">
							<label
								for="llm-model"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Model
							</label>
							<select
								id="llm-model"
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
										formConfig = JSON.stringify(config, null, 2);
									}
								}}
							>
								{#if formProvider === 'local' && localModelsForDisplay.length > 0}
									{#each localModelsForDisplay as model (model.id)}
										<option value={model.id}>{model.name}</option>
									{/each}
									<option value="__custom__">Custom...</option>
								{:else if providerDefaults[formProvider]?.models}
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

						{#if formProvider !== 'local'}
							<div class="mt-4">
								<label
									for="llm-api-key"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									API Key
								</label>
								<input
									id="llm-api-key"
									type="password"
									autocomplete="off"
									bind:value={formApiKey}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									placeholder="Enter your API key"
								/>
							</div>
						{/if}

						<div class="mt-4">
							<label
								for="llm-config"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Configuration (JSON)
							</label>
							<textarea
								id="llm-config"
								bind:value={formConfig}
								rows="4"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white font-mono text-sm"
								placeholder={JSON.stringify({
									model: providerDefaults[formProvider]?.models?.[0] || '',
								})}
							></textarea>
						</div>

						<div class="mt-4">
							<label class="flex items-center gap-2 cursor-pointer">
								<input
									type="checkbox"
									bind:checked={formIsPublic}
									class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
								/>
								<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
									Make this LLM public (visible in marketplace)
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
								{editingLLM ? 'Update' : 'Create'}
							</button>
							<button
								type="button"
								onclick={testLLMConnection}
								class="px-4 py-2 bg-yellow-500 text-white rounded-lg hover:bg-yellow-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
								disabled={testStatus === 'testing'}
							>
								Test Connection
							</button>
							<button
								type="button"
								onclick={() => {
									showCreateForm = false;
									editingLLM = null;
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
		<div class="mb-4">
			<div class="relative">
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search LLMs by name, provider, owner, or URL..."
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
				onclick={fetchLLMs}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if llms.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-12 text-center">
			<p class="text-gray-500 dark:text-gray-400 mb-4">No LLMs yet</p>
			<button
				onclick={() => openCreateForm()}
				class="text-blue-600 dark:text-blue-400 hover:underline"
			>
				Create your first LLM
			</button>
		</div>
	{:else}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
			<Table hoverable striped>
				<TableHead>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Name</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Provider</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Model</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Public</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold">Owner</TableHeadCell>
					<TableHeadCell class="px-4 py-3 text-sm font-semibold text-center">Actions</TableHeadCell>
				</TableHead>
				<TableBody>
					{#each llms as llm (llm.llm_id)}
						<tr class="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
							<TableBodyCell class="px-4 py-3">
								<button
									onclick={() => openEditForm(llm)}
									class="font-semibold text-blue-600 dark:text-blue-400 hover:underline"
								>
									{llm.name}
								</button>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span
									class="inline-block px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-sm font-medium"
								>
									{llm.provider}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								<span class="text-gray-700 dark:text-gray-300 text-sm">
								{llm.config.model ?? 'N/A'}
								</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
								{#if llm.is_public}
									<span
										class="inline-block px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 rounded text-sm font-medium"
									>
										Yes
									</span>
								{:else}
									<span class="text-gray-500 dark:text-gray-400 text-sm">No</span>
								{/if}
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3">
							<span class="text-gray-700 dark:text-gray-300">{llm.owner_display_name}</span>
							</TableBodyCell>
							<TableBodyCell class="px-4 py-3 text-center">
								<ActionMenu
									actions={[
										{
											label: 'Edit',
											handler: () => openEditForm(llm),
										},
										{
											label: 'Delete',
											handler: () => requestDeleteLLM(llm),
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
					LLMs
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
	open={llmPendingDelete !== null}
	title="Delete LLM"
	message={llmPendingDelete
		? `Are you sure you want to delete "${llmPendingDelete.name}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteLLM}
	on:cancel={() => (llmPendingDelete = null)}
/>
