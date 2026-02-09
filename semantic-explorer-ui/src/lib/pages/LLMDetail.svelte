<script lang="ts">
	import { ArrowLeftOutline } from 'flowbite-svelte-icons';
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import TabPanel from '../components/TabPanel.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface LLM {
		llm_id: number;
		name: string;
		owner_id: string;
		owner_display_name: string;
		provider: string;
		base_url: string;
		api_key: string | null;
		config: Record<string, any>;
		is_public: boolean;
		created_at: string;
		updated_at: string;
	}

	interface PaginatedLLMList {
		items: LLM[];
		total_count: number;
		limit: number;
		offset: number;
	}

	interface Props {
		llmId: number;
		onBack: () => void;
	}

	let { llmId, onBack }: Props = $props();

	let llm = $state<LLM | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Edit form state
	let editMode = $state(false);
	let editFormName = $state('');
	let editFormApiKey = $state('');
	let editFormBaseUrl = $state('');
	let editFormConfig = $state('');
	let editFormIsPublic = $state(false);
	let editError = $state<string | null>(null);
	let editLoading = $state(false);

	// Delete state
	let llmPendingDelete = $state(false);

	// Tab state
	let activeTab = $state('overview');

	const tabs = [{ id: 'overview', label: 'Overview', icon: '⚙️' }];

	async function fetchLLM() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/llms?limit=1000');
			if (!response.ok) {
				throw new Error(`Failed to fetch LLMs: ${response.statusText}`);
			}
			const data: PaginatedLLMList = await response.json();
			const foundLLM = data.items.find((l) => l.llm_id === llmId);
			if (!foundLLM) {
				throw new Error(`LLM with ID ${llmId} not found`);
			}
			llm = foundLLM;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to fetch LLM';
			toastStore.error(formatError(e, 'Failed to fetch LLM'));
		} finally {
			loading = false;
		}
	}

	function startEdit() {
		if (!llm) return;
		editMode = true;
		editFormName = llm.name;
		editFormApiKey = llm.api_key || '';
		editFormBaseUrl = llm.base_url;
		editFormConfig = JSON.stringify(llm.config, null, 2);
		editFormIsPublic = llm.is_public;
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editFormName = '';
		editFormApiKey = '';
		editFormBaseUrl = '';
		editFormConfig = '';
		editFormIsPublic = false;
		editError = null;
	}

	async function saveEdit() {
		if (!llm) return;

		if (!editFormName.trim()) {
			editError = 'Name is required';
			return;
		}

		let parsedConfig: Record<string, any>;
		try {
			parsedConfig = JSON.parse(editFormConfig);
		} catch {
			editError = 'Invalid JSON configuration';
			return;
		}

		try {
			editLoading = true;
			editError = null;

			const response = await fetch(`/api/llms/${llm.llm_id}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					name: editFormName.trim(),
					base_url: editFormBaseUrl.trim(),
					api_key: editFormApiKey.trim() || null,
					config: parsedConfig,
					is_public: editFormIsPublic,
				}),
			});

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to update LLM: ${errorText}`);
			}

			const updatedLLM = await response.json();
			llm = updatedLLM;
			editMode = false;
			toastStore.success('LLM updated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to update LLM');
			editError = message;
			toastStore.error(message);
		} finally {
			editLoading = false;
		}
	}

	async function confirmDeleteLLM() {
		if (!llm) return;

		llmPendingDelete = false;

		try {
			const response = await fetch(`/api/llms/${llm.llm_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to delete LLM: ${errorText}`);
			}

			toastStore.success('LLM deleted successfully');
			onBack();
		} catch (e) {
			const message = formatError(e, 'Failed to delete LLM');
			toastStore.error(message);
		}
	}

	onMount(() => {
		fetchLLM();
	});
</script>

<div class=" mx-auto">
	<div class="mb-4">
		<button onclick={onBack} class="mb-4 btn-secondary inline-flex items-center gap-2">
			<ArrowLeftOutline class="w-5 h-5" />
			Back to LLMs
		</button>

		{#if loading}
			<LoadingState message="Loading LLM..." />
		{:else if error}
			<div
				class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
			>
				<p class="text-red-700 dark:text-red-400">{error}</p>
				<button
					onclick={fetchLLM}
					class="mt-2 text-sm text-red-600 dark:text-red-400 hover:underline"
				>
					Try again
				</button>
			</div>
		{:else if llm}
			<!-- Main Card -->
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-4">
				{#if editMode}
					<!-- Edit Mode -->
					<div class="flex justify-between items-center mb-4">
						<h2 class="text-xl font-semibold text-gray-900 dark:text-white">Edit LLM</h2>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={cancelEdit}
								disabled={editLoading}
								class="px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors disabled:opacity-50"
							>
								Cancel
							</button>
							<button
								type="button"
								onclick={saveEdit}
								disabled={editLoading}
								class="px-3 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors disabled:opacity-50"
							>
								{editLoading ? 'Saving...' : 'Save Changes'}
							</button>
						</div>
					</div>

					{#if editError}
						<div
							class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3 mb-4"
						>
							<p class="text-sm text-red-700 dark:text-red-400">{editError}</p>
						</div>
					{/if}

					<div class="space-y-4">
						<div>
							<label
								for="edit-name"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Name <span class="text-red-500">*</span>
							</label>
							<input
								id="edit-name"
								type="text"
								bind:value={editFormName}
								disabled={editLoading}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
								placeholder="Enter LLM name"
							/>
						</div>

						{#if llm.provider !== 'internal'}
							<div>
								<label
									for="edit-base-url"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Base URL <span class="text-red-500">*</span>
								</label>
								<input
									id="edit-base-url"
									type="text"
									bind:value={editFormBaseUrl}
									disabled={editLoading}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
									placeholder="https://api.example.com/v1"
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
									bind:value={editFormApiKey}
									disabled={editLoading}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
									placeholder="Enter API key (optional)"
								/>
							</div>
						{/if}

						<div>
							<label
								for="edit-config"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Configuration (JSON) <span class="text-red-500">*</span>
							</label>
							<textarea
								id="edit-config"
								bind:value={editFormConfig}
								disabled={editLoading}
								rows="8"
								class="w-full px-3 py-2 font-mono text-sm border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white disabled:opacity-50"
								placeholder={`{ "model": "gpt-4o" }`}
							></textarea>
						</div>

						<div>
							<label class="inline-flex items-center gap-2 cursor-pointer">
								<input
									type="checkbox"
									bind:checked={editFormIsPublic}
									disabled={editLoading}
									class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
								/>
								<span class="text-sm text-gray-700 dark:text-gray-300">
									Public - visible in marketplace
								</span>
							</label>
						</div>
					</div>
				{:else}
					<!-- View Mode -->
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h1 class="text-3xl font-bold text-gray-900 dark:text-white">
									{llm.name}
								</h1>
								<span class="text-sm text-gray-500 dark:text-gray-400">
									#{llm.llm_id}
								</span>
							</div>
							<div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
								<div>
									<p class="text-sm text-gray-600 dark:text-gray-400">Provider</p>
									<p class="text-lg font-medium text-gray-900 dark:text-white">
										{llm.provider}
									</p>
								</div>
								<div>
									<p class="text-sm text-gray-600 dark:text-gray-400">Model</p>
									<p class="text-lg font-medium text-gray-900 dark:text-white">
										{llm.config?.model || 'N/A'}
									</p>
								</div>
								<div>
									<p class="text-sm text-gray-600 dark:text-gray-400">Owner</p>
									<p class="text-lg font-medium text-gray-900 dark:text-white">
										{llm.owner_display_name}
									</p>
								</div>
								<div>
									<p class="text-sm text-gray-600 dark:text-gray-400">Public</p>
									<p class="text-lg font-medium text-gray-900 dark:text-white">
										{llm.is_public ? 'Yes' : 'No'}
									</p>
								</div>
							</div>

							<!-- Configuration Details -->
							<div class="mt-6">
								<details class="group">
									<summary
										class="flex items-center gap-2 cursor-pointer text-sm font-semibold text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white"
									>
										<svg
											class="w-4 h-4 transition-transform group-open:rotate-90"
											fill="none"
											stroke="currentColor"
											viewBox="0 0 24 24"
										>
											<path
												stroke-linecap="round"
												stroke-linejoin="round"
												stroke-width="2"
												d="M9 5l7 7-7 7"
											/>
										</svg>
										Configuration Details
									</summary>
									<div class="mt-3 space-y-3">
										{#if llm.provider !== 'internal'}
											<div>
												<p class="text-sm font-medium text-gray-600 dark:text-gray-400 mb-1">
													Base URL
												</p>
												<p class="text-sm font-mono text-gray-700 dark:text-gray-300 break-all">
													{llm.base_url}
												</p>
											</div>
										{/if}
										<div>
											<p class="text-sm font-medium text-gray-600 dark:text-gray-400 mb-1">
												Configuration
											</p>
											<pre
												class="bg-gray-50 dark:bg-gray-900 p-3 rounded overflow-auto text-xs text-gray-700 dark:text-gray-300">{JSON.stringify(
													llm.config,
													null,
													2
												)}</pre>
										</div>
										<div class="grid grid-cols-2 gap-3 text-sm">
											<div>
												<p class="text-gray-600 dark:text-gray-400">Created</p>
												<p class="text-gray-900 dark:text-white font-medium">
													{formatDate(llm.created_at)}
												</p>
											</div>
											<div>
												<p class="text-gray-600 dark:text-gray-400">Updated</p>
												<p class="text-gray-900 dark:text-white font-medium">
													{formatDate(llm.updated_at)}
												</p>
											</div>
										</div>
									</div>
								</details>
							</div>
						</div>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={startEdit}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
								title="Edit LLM"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
								type="button"
								onclick={() => (llmPendingDelete = true)}
								class="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-red-700 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
				{/if}
			</div>

			<!-- Tabs Section -->
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4">
				<TabPanel {tabs} activeTabId={activeTab} onChange={(id) => (activeTab = id)}>
					{#snippet children(tabId)}
						{#if tabId === 'overview' && llm}
							<div class="p-4">
								<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">
									LLM Information
								</h3>
								<div class="space-y-3">
									<div>
										<p class="text-sm text-gray-600 dark:text-gray-400">Name</p>
										<p class="text-gray-900 dark:text-white font-medium">{llm.name}</p>
									</div>
									<div>
										<p class="text-sm text-gray-600 dark:text-gray-400">Provider</p>
										<p class="text-gray-900 dark:text-white font-medium">{llm.provider}</p>
									</div>
									<div>
										<p class="text-sm text-gray-600 dark:text-gray-400">Model</p>
										<p class="text-gray-900 dark:text-white font-medium">
											{llm.config?.model || 'N/A'}
										</p>
									</div>
									<div>
										<p class="text-sm text-gray-600 dark:text-gray-400">Created</p>
										<p class="text-gray-900 dark:text-white font-medium">
											{formatDate(llm.created_at)}
										</p>
									</div>
									<div>
										<p class="text-sm text-gray-600 dark:text-gray-400">Last Updated</p>
										<p class="text-gray-900 dark:text-white font-medium">
											{formatDate(llm.updated_at)}
										</p>
									</div>
								</div>
							</div>
						{/if}
					{/snippet}
				</TabPanel>
			</div>
		{/if}
	</div>
</div>

<!-- Delete Confirmation Dialog -->
<ConfirmDialog
	open={llmPendingDelete}
	title="Delete LLM?"
	message="Are you sure you want to delete this LLM? This action cannot be undone."
	confirmLabel="Delete"
	cancelLabel="Cancel"
	onConfirm={confirmDeleteLLM}
	onCancel={() => (llmPendingDelete = false)}
	variant="danger"
/>
