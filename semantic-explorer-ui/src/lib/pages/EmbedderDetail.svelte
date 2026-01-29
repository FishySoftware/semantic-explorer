<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

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

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		source_dataset_id: number;
		embedder_id: number;
		owner: string;
		collection_name: string;
		created_at: string;
		updated_at: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface PaginatedEmbedderList {
		items: Embedder[];
		total_count: number;
		limit: number;
		offset: number;
	}

	interface Props {
		embedderId: number;
		onBack: () => void;
	}

	let { embedderId, onBack }: Props = $props();

	let embedder = $state<Embedder | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let embeddingsLoading = $state(false);
	let datasetsCache = $state<Map<number, Dataset>>(new Map());

	// Edit form state
	let editMode = $state(false);
	let editFormName = $state('');
	let editFormApiKey = $state('');
	let editFormBaseUrl = $state('');
	let editFormConfig = $state('');
	let editFormBatchSize = $state(100);
	let editFormDimensions = $state(1536);
	let editFormMaxInputTokens = $state(8191);
	let editFormTruncateStrategy = $state('NONE');
	let editFormIsPublic = $state(false);
	let editError = $state<string | null>(null);
	let editLoading = $state(false);

	// Test connection state
	let testStatus = $state<'idle' | 'testing' | 'success' | 'error'>('idle');
	let testMessage = $state('');

	// Delete state
	let embedderPendingDelete = $state(false);

	// Tab state
	let activeTab = $state('overview');

	const tabs = [
		{ id: 'overview', label: 'Overview', icon: '⚙️' },
		{ id: 'embeddings', label: 'Embedded Datasets', icon: '🧬' },
	];

	async function fetchEmbedder() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/embedders?limit=10&offset=0');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedders: ${response.statusText}`);
			}
			const data: PaginatedEmbedderList = await response.json();
			const embedders: Embedder[] = data.items;
			embedder = embedders.find((e) => e.embedder_id === embedderId) || null;
			if (!embedder) {
				throw new Error('Embedder not found');
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to fetch embedder';
		} finally {
			loading = false;
		}
	}

	async function fetchDataset(datasetId: number): Promise<Dataset | null> {
		if (datasetsCache.has(datasetId)) {
			return datasetsCache.get(datasetId) || null;
		}
		try {
			const response = await fetch(`/api/datasets/${datasetId}`);
			if (response.ok) {
				const dataset = await response.json();
				datasetsCache.set(datasetId, dataset);
				datasetsCache = datasetsCache; // Trigger reactivity
				return dataset;
			}
		} catch (e) {
			console.error(`Failed to fetch dataset ${datasetId}:`, e);
		}
		return null;
	}

	async function fetchEmbeddedDatasets() {
		try {
			embeddingsLoading = true;
			const response = await fetch('/api/embedded-datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			const data = await response.json();
			const allEmbeddedDatasets: EmbeddedDataset[] = data.embedded_datasets || [];
			embeddedDatasets = allEmbeddedDatasets
				.filter((ed) => ed.embedder_id === embedderId)
				.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());

			// Fetch dataset information for each embedded dataset
			for (const ed of embeddedDatasets) {
				await fetchDataset(ed.source_dataset_id);
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to load embedded datasets'));
		} finally {
			embeddingsLoading = false;
		}
	}

	function startEdit() {
		if (!embedder) return;
		editMode = true;
		editFormName = embedder.name;
		editFormApiKey = embedder.api_key || '';
		editFormBaseUrl = embedder.base_url;
		editFormConfig = JSON.stringify(embedder.config, null, 2);
		editFormBatchSize = embedder.batch_size ?? 100;
		editFormDimensions = embedder.dimensions ?? 1536;
		editFormMaxInputTokens = (embedder as any).max_input_tokens ?? 8191;
		editFormTruncateStrategy = (embedder as any).truncate_strategy ?? 'NONE';
		editFormIsPublic = embedder.is_public || false;
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editError = null;
	}

	async function saveEdit() {
		if (!embedder) return;

		try {
			editLoading = true;
			editError = null;

			const config = JSON.parse(editFormConfig);
			const body = {
				name: editFormName,
				base_url: editFormBaseUrl,
				api_key: editFormApiKey || null,
				config,
				batch_size: editFormBatchSize,
				dimensions: editFormDimensions,
				max_input_tokens: editFormMaxInputTokens,
				truncate_strategy: editFormTruncateStrategy,
				is_public: editFormIsPublic,
			};

			const response = await fetch(`/api/embedders/${embedderId}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				throw new Error(`Failed to save embedder: ${response.statusText}`);
			}

			const updatedEmbedder = await response.json();
			embedder = updatedEmbedder;
			editMode = false;
			toastStore.success('Embedder updated successfully');
		} catch (e) {
			editError = formatError(e, 'Failed to save embedder');
		} finally {
			editLoading = false;
		}
	}

	async function testConnection() {
		testMessage = '';
		try {
			testStatus = 'testing';
			let config: Record<string, any>;
			if (editMode) {
				config = JSON.parse(editFormConfig);
			} else {
				config = embedder?.config || {};
			}
			const provider = embedder?.provider || 'openai';
			const baseUrl = editMode ? editFormBaseUrl : embedder?.base_url || '';
			const apiKey = editMode ? editFormApiKey : embedder?.api_key || '';

			const testText = ['Hello world', 'Test embedding'];
			let response: Response;

			if (provider === 'openai') {
				response = await fetch(`${baseUrl}/embeddings`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${apiKey}`,
					},
					body: JSON.stringify({
						input: testText,
						model: config.model || 'text-embedding-3-small',
						...(config.dimensions && { dimensions: config.dimensions }),
					}),
				});
			} else if (provider === 'cohere') {
				response = await fetch(baseUrl, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${apiKey}`,
					},
					body: JSON.stringify({
						texts: testText,
						model: config.model || 'embed-v4.0',
						...(config.input_type && { input_type: config.input_type }),
						...(config.embedding_types && { embedding_types: config.embedding_types }),
						...(config.truncate && { truncate: config.truncate }),
					}),
				});
			} else if (provider === 'huggingface') {
				const model = config.model || 'sentence-transformers/all-MiniLM-L6-v2';
				response = await fetch(`${baseUrl}/pipeline/feature-extraction/${model}`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						...(apiKey && { Authorization: `Bearer ${apiKey}` }),
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
			if (provider === 'cohere') {
				if (result.embeddings?.float) {
					embeddingCount = result.embeddings.float.length;
				} else if (result.embeddings?.int8) {
					embeddingCount = result.embeddings.int8.length;
				} else if (result.embeddings?.uint8) {
					embeddingCount = result.embeddings.uint8.length;
				}
			} else if (provider === 'openai') {
				embeddingCount = result.data?.length || 0;
			}

			testMessage = `Connection successful! Generated ${embeddingCount} embedding(s).`;
		} catch (e: any) {
			testStatus = 'error';
			testMessage = e.message || 'Test failed.';
		}
	}

	async function confirmDeleteEmbedder() {
		if (!embedder) return;

		try {
			const response = await fetch(`/api/embedders/${embedderId}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete embedder: ${response.statusText}`);
			}

			toastStore.success('Embedder deleted successfully');
			embedderPendingDelete = false;
			onBack();
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete embedder'));
			embedderPendingDelete = false;
		}
	}

	onMount(() => {
		fetchEmbedder();
		fetchEmbeddedDatasets();
	});
</script>

<div class="max-w-6xl mx-auto pb-8">
	<!-- Header -->
	<div class="flex items-center justify-between mb-6">
		<div class="flex items-center gap-4">
			<button
				onclick={onBack}
				class="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
				aria-label="Go back"
			>
				<svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M15 19l-7-7 7-7"
					/>
				</svg>
			</button>
			<div>
				<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
					{loading ? 'Loading...' : embedder?.name || 'Embedder'}
				</h1>
				{#if embedder}
					<p class="text-gray-600 dark:text-gray-400 mt-1">
						{embedder.provider.charAt(0).toUpperCase() + embedder.provider.slice(1)} • {embedder.owner}
					</p>
				{/if}
			</div>
		</div>
	</div>

	{#if loading}
		<LoadingState message="Loading embedder..." />
	{:else if error}
		<div class="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-4 rounded-lg">
			<p>{error}</p>
			<button
				onclick={() => {
					loading = true;
					fetchEmbedder();
				}}
				class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
			>
				Try again
			</button>
		</div>
	{:else if embedder}
		<!-- Tabs -->

		<!-- Tabs -->
		<div class="mb-6">
			<TabPanel {tabs} activeTabId={activeTab} onChange={(id) => (activeTab = id)}>
				{#if activeTab === 'overview'}
					<div class="space-y-6">
						<!-- Info Cards -->
						<div class="grid grid-cols-2 gap-4">
							<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
								<div class="text-sm font-medium text-gray-600 dark:text-gray-400">Provider</div>
								<div class="text-lg font-semibold text-gray-900 dark:text-white mt-1">
									{embedder.provider}
								</div>
							</div>
							<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
								<div class="text-sm font-medium text-gray-600 dark:text-gray-400">Dimensions</div>
								<div class="text-lg font-semibold text-gray-900 dark:text-white mt-1">
									{embedder.dimensions || 'Not specified'}
								</div>
							</div>
							<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
								<div class="text-sm font-medium text-gray-600 dark:text-gray-400">Batch Size</div>
								<div class="text-lg font-semibold text-gray-900 dark:text-white mt-1">
									{embedder.batch_size || 'Not specified'}
								</div>
							</div>
						</div>

						<!-- Timestamps -->
						<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
							<div class="grid grid-cols-2 gap-4 text-sm">
								<div>
									<div class="font-medium text-gray-600 dark:text-gray-400">Created</div>
									<div class="text-gray-900 dark:text-white mt-1">
										{formatDate(embedder.created_at)}
									</div>
								</div>
								<div>
									<div class="font-medium text-gray-600 dark:text-gray-400">Updated</div>
									<div class="text-gray-900 dark:text-white mt-1">
										{formatDate(embedder.updated_at)}
									</div>
								</div>
							</div>
						</div>

						<!-- Configuration -->
						<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
							<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
								Configuration
							</h3>
							<div class="space-y-4 text-sm">
								<div>
									<div class="font-medium text-gray-600 dark:text-gray-400 mb-1">Base URL</div>
									<div class="font-mono text-gray-700 dark:text-gray-300 break-all">
										{embedder.base_url}
									</div>
								</div>
								<div>
									<div class="font-medium text-gray-600 dark:text-gray-400 mb-1">Config</div>
									<pre
										class="bg-gray-50 dark:bg-gray-900 p-3 rounded overflow-auto text-xs text-gray-700 dark:text-gray-300">{JSON.stringify(
											embedder.config,
											null,
											2
										)}</pre>
								</div>
							</div>
						</div>

						<!-- Edit Section -->
						{#if !editMode}
							<button onclick={startEdit} class="w-full btn-primary"> Edit Embedder </button>
						{:else}
							<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
								<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
									Edit Embedder
								</h3>

								{#if editError}
									<div
										class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400"
									>
										{editError}
									</div>
								{/if}

								<div class="space-y-4">
									<div>
										<label
											for="edit-name"
											class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
										>
											Name
										</label>
										<input
											id="edit-name"
											type="text"
											bind:value={editFormName}
											class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
										/>
									</div>

									<div>
										<label
											for="edit-base-url"
											class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
										>
											Base URL
										</label>
										<input
											id="edit-base-url"
											type="text"
											bind:value={editFormBaseUrl}
											class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
										/>
									</div>

									<div>
										<label
											for="edit-api-key"
											class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
										>
											API Key
										</label>
										<input
											id="edit-api-key"
											type="password"
											autocomplete="off"
											bind:value={editFormApiKey}
											class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
											placeholder="Leave empty to keep current key"
										/>
									</div>

									<div class="grid grid-cols-2 gap-4">
										<div>
											<label
												for="edit-dimensions"
												class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
											>
												Dimensions
											</label>
											<input
												id="edit-dimensions"
												type="number"
												bind:value={editFormDimensions}
												class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
											/>
										</div>
										<div>
											<label
												for="edit-batch-size"
												class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
											>
												Batch Size
											</label>
											<input
												id="edit-batch-size"
												type="number"
												bind:value={editFormBatchSize}
												class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
											/>
										</div>
									</div>

									<div>
										<label
											for="edit-config"
											class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
										>
											Config (JSON)
										</label>
										<textarea
											id="edit-config"
											bind:value={editFormConfig}
											rows="6"
											class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white font-mono text-sm"
										></textarea>
									</div>

									<div>
										<label class="flex items-center gap-2 cursor-pointer">
											<input
												type="checkbox"
												bind:checked={editFormIsPublic}
												class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
											/>
											<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
												Make this embedder public (visible in marketplace)
											</span>
										</label>
									</div>

									<div class="flex gap-2">
										{#if embedder?.provider !== 'internal'}
											<button
												onclick={testConnection}
												disabled={testStatus === 'testing'}
												class="flex-1 btn-secondary disabled:opacity-50"
											>
												{#if testStatus === 'testing'}
													Testing...
												{:else}
													Test Connection
												{/if}
											</button>
										{/if}
										<button
											onclick={saveEdit}
											disabled={editLoading}
											class="flex-1 btn-primary disabled:opacity-50"
										>
											{editLoading ? 'Saving...' : 'Save Changes'}
										</button>
										<button
											onclick={cancelEdit}
											disabled={editLoading}
											class="flex-1 btn-secondary disabled:opacity-50"
										>
											Cancel
										</button>
									</div>

									{#if testMessage}
										<div
											class={`p-3 rounded-lg ${
												testStatus === 'success'
													? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300'
													: 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'
											}`}
										>
											{testMessage}
										</div>
									{/if}
								</div>
							</div>
						{/if}

						<!-- Delete Button -->
						<button onclick={() => (embedderPendingDelete = true)} class="w-full btn-danger">
							Delete Embedder
						</button>
					</div>
				{:else if activeTab === 'embeddings'}
					<div class="space-y-4">
						{#if embeddingsLoading}
							<div class="flex justify-center py-8">
								<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
							</div>
						{:else if embeddedDatasets.length === 0}
							<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
								<p class="text-gray-500 dark:text-gray-400">
									No embedded datasets created with this embedder yet.
								</p>
							</div>
						{:else}
							<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
								<div class="space-y-3 p-4">
									{#each embeddedDatasets as ed (ed.embedded_dataset_id)}
										<div
											class="border border-gray-200 dark:border-gray-700 rounded-lg p-4 hover:border-blue-400 dark:hover:border-blue-600 transition-colors"
										>
											<div class="flex items-start justify-between">
												<div class="flex-1 min-w-0">
													<button
														onclick={() =>
															(window.location.hash = `#/embedded-datasets/${ed.embedded_dataset_id}/details`)}
														class="font-semibold text-blue-600 dark:text-blue-400 hover:underline wrap-break-word text-left"
													>
														{ed.title}
													</button>
													<p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
														Dataset: <button
															onclick={() =>
																(window.location.hash = `#/datasets/${ed.source_dataset_id}/details`)}
															class="text-blue-600 dark:text-blue-400 hover:underline"
														>
															{datasetsCache.get(ed.source_dataset_id)?.title ||
																`Dataset #${ed.source_dataset_id}`}
														</button>
													</p>
													<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
														Collection: <button
															onclick={() =>
																(window.location.hash = `#/collections?search=${encodeURIComponent(ed.collection_name)}`)}
															class="font-mono text-[10px] text-blue-600 dark:text-blue-400 hover:underline"
														>
															{ed.collection_name}
														</button>
													</p>
												</div>
												<div class="text-right ml-4 shrink-0">
													<p class="text-xs text-gray-500 dark:text-gray-400">Created</p>
													<p class="text-sm text-gray-900 dark:text-white font-medium">
														{formatDate(ed.created_at)}
													</p>
												</div>
											</div>
										</div>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</TabPanel>
		</div>
	{/if}
</div>

<ConfirmDialog
	open={embedderPendingDelete}
	title="Delete embedder"
	message={embedder
		? `Are you sure you want to delete "${embedder.name}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDeleteEmbedder}
	onCancel={() => (embedderPendingDelete = false)}
/>
