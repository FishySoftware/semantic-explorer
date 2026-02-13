<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import ChatInput from '../components/ChatInput.svelte';
	import ChatMessage from '../components/ChatMessage.svelte';
	import ChatSettings from '../components/ChatSettings.svelte';
	import ChatSidebar from '../components/ChatSidebar.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { useChatStream } from '../composables/useChatStream.svelte';
	import type {
		ChatMessage as ChatMessageType,
		ChatSession,
		EmbeddedDataset,
		LLM,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';

	// Session management
	let sessions = $state<ChatSession[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let llms = $state<LLM[]>([]);
	let currentSession = $state<ChatSession | null>(null);
	let currentMessages = $state<ChatMessageType[]>([]);
	let messagesLoading = $state(false);

	// Create form state
	let newSessionTitle = $state('');
	let newSessionEmbeddedDatasetId = $state<number | null>(null);
	let newSessionLLMId = $state<number | null>(null);
	let showCreateForm = $state(false);

	// UI state
	let settingsExpanded = $state(false);
	let inputValue = $state('');

	// RAG Configuration
	let maxChunks = $state(5);
	let minSimilarityScore = $state(0.2);

	// LLM Configuration
	let temperature = $state(0.7);
	let maxTokens = $state(2000);
	let systemPrompt = $state('');

	// Chat stream state
	let chatStream = $state<ReturnType<typeof useChatStream> | null>(null);

	// Auto-scroll to messages container
	let messagesContainer = $state<HTMLDivElement | null>(null);

	// Auto-scroll to bottom when messages change
	$effect(() => {
		if (messagesContainer && currentMessages.length > 0) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	});

	function generateDefaultTitle(): string {
		const now = new Date();
		const year = now.getFullYear();
		const month = String(now.getMonth() + 1).padStart(2, '0');
		const day = String(now.getDate()).padStart(2, '0');
		const hours = String(now.getHours()).padStart(2, '0');
		const minutes = String(now.getMinutes()).padStart(2, '0');
		const seconds = String(now.getSeconds()).padStart(2, '0');
		return `chat-session-${year}${month}${day}-${hours}${minutes}${seconds}`;
	}

	function resetCreateForm() {
		newSessionTitle = '';
		newSessionEmbeddedDatasetId = null;
		newSessionLLMId = null;
		showCreateForm = false;
	}

	async function fetchSessions() {
		try {
			const response = await fetch('/api/chat/sessions?limit=200');
			if (!response.ok) {
				throw new Error(`Failed to fetch chat sessions: ${response.statusText}`);
			}
			const data = await response.json();
			sessions = data.sessions;
		} catch (e) {
			const message = formatError(e, 'Failed to fetch chat sessions');
			toastStore.error(message);
		}
	}

	async function fetchEmbeddedDatasets() {
		try {
			const response = await fetch('/api/embedded-datasets?limit=1000');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			const data = (await response.json()) as { embedded_datasets: EmbeddedDataset[] };
			// Filter out standalone datasets - they don't have embedders and can't be used for RAG
			embeddedDatasets = data.embedded_datasets.filter(
				(ed) =>
					!ed.is_standalone &&
					!(ed.dataset_transform_id === 0 && ed.source_dataset_id === 0 && ed.embedder_id === 0)
			);
		} catch (e) {
			console.error('Failed to fetch embedded datasets:', e);
		}
	}

	async function fetchLLMs() {
		try {
			const response = await fetch('/api/llms?limit=100');
			if (!response.ok) {
				throw new Error(`Failed to fetch LLMs: ${response.statusText}`);
			}
			const data = (await response.json()) as { items: LLM[] };
			llms = data.items;
		} catch (e) {
			console.error('Failed to fetch LLMs:', e);
		}
	}

	async function createSession() {
		if (!newSessionEmbeddedDatasetId || !newSessionLLMId) {
			toastStore.error('Please select both an embedded dataset and an LLM');
			return;
		}

		try {
			const title = newSessionTitle.trim();

			const response = await fetch('/api/chat/sessions', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					embedded_dataset_id: newSessionEmbeddedDatasetId,
					llm_id: newSessionLLMId,
					title: title,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to create chat session: ${response.statusText}`);
			}

			const newSession = await response.json();
			sessions = [...sessions, newSession];

			// Select the new session
			currentSession = newSession;
			currentMessages = [];
			inputValue = '';

			toastStore.success('Chat session created successfully');

			resetCreateForm();
		} catch (e) {
			const message = formatError(e, 'Failed to create chat session');
			toastStore.error(message);
		}
	}

	async function selectSession(session: ChatSession) {
		try {
			messagesLoading = true;
			currentSession = session;
			inputValue = '';

			const response = await fetch(`/api/chat/sessions/${session.session_id}/messages`);
			if (!response.ok) {
				throw new Error(`Failed to fetch messages: ${response.statusText}`);
			}

			const data = await response.json();
			currentMessages = data.messages;
		} catch (e) {
			const message = formatError(e, 'Failed to load chat messages');
			toastStore.error(message);
		} finally {
			messagesLoading = false;
		}
	}

	async function deleteSession(session: ChatSession | null) {
		if (!session) {
			console.error('No session selected for deletion');
			return;
		}

		if (!confirm(`Delete chat session "${session.title || 'Untitled'}"?`)) {
			return;
		}

		try {
			const response = await fetch(`/api/chat/sessions/${session.session_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete session: ${response.statusText}`);
			}

			sessions = sessions.filter((s) => s.session_id !== session.session_id);

			if (currentSession?.session_id === session.session_id) {
				currentSession = null;
				currentMessages = [];
			}

			toastStore.success('Chat session deleted successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to delete chat session');
			toastStore.error(message);
		}
	}

	function getEmbeddedDatasetTitle(id: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === id);
		return dataset ? `${dataset.title} (${dataset.embedder_name})` : `Dataset ${id}`;
	}

	function getLLMTitle(id: number): string {
		const llm = llms.find((l) => l.llm_id === id);
		return llm ? `${llm.name} (${llm.provider})` : `LLM ${id}`;
	}

	async function handleSendMessage() {
		if (!currentSession || !inputValue.trim()) return;

		const content = inputValue.trim();
		inputValue = '';

		// Add optimistic user message
		const tempUserMessage = {
			message_id: Date.now(),
			role: 'user' as const,
			content,
			created_at: new Date().toISOString(),
			tokens_used: null,
			metadata: null,
			documents_retrieved: null,
			status: 'complete' as const,
		};
		currentMessages = [...currentMessages, tempUserMessage];

		// Add placeholder assistant message
		const tempAssistantMessage = {
			message_id: Date.now() + 1,
			role: 'assistant' as const,
			content: '',
			created_at: new Date().toISOString(),
			tokens_used: null,
			metadata: null,
			documents_retrieved: 0,
			status: 'incomplete' as const,
		};
		currentMessages = [...currentMessages, tempAssistantMessage];

		// Initialize chat stream if needed
		if (!chatStream) {
			chatStream = useChatStream({
				sessionId: currentSession.session_id,
				getSettings: () => ({
					maxChunks,
					minSimilarityScore,
					temperature,
					maxTokens,
					systemPrompt,
				}),
				callbacks: {
					onComplete: () => {
						// Refresh messages after completion
						fetchMessagesForCurrentSession();
					},
					onError: (error) => {
						toastStore.error(`Chat error: ${error}`);
						fetchMessagesForCurrentSession();
					},
				},
			});
		}

		await chatStream.sendMessage(content);
		// Refresh to get actual messages from server
		await fetchMessagesForCurrentSession();
	}

	async function fetchMessagesForCurrentSession() {
		if (!currentSession) return;
		try {
			const response = await fetch(`/api/chat/sessions/${currentSession.session_id}/messages`);
			if (response.ok) {
				const data = await response.json();
				currentMessages = data.messages;
			}
		} catch {
			// Silently fail - messages will be out of sync but user can refresh
		}
	}

	// Reset chat stream when session changes
	$effect(() => {
		const session = currentSession;
		if (session) {
			untrack(() => {
				if (chatStream) {
					chatStream.cleanup();
				}
				chatStream = useChatStream({
					sessionId: session.session_id,
					getSettings: () => ({
						maxChunks,
						minSimilarityScore,
						temperature,
						maxTokens,
						systemPrompt,
					}),
					callbacks: {
						onComplete: () => {
							fetchMessagesForCurrentSession();
						},
						onError: (error) => {
							toastStore.error(`Chat error: ${error}`);
						},
					},
				});
			});
		}
	});

	onMount(() => {
		// Parse URL parameters for preset values
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const params = new URLSearchParams(hashParts[1]);
			const embeddedDatasetIdParam = params.get('embedded_dataset_id');

			if (embeddedDatasetIdParam) {
				const datasetId = parseInt(embeddedDatasetIdParam, 10);
				if (!isNaN(datasetId)) {
					newSessionEmbeddedDatasetId = datasetId;
					newSessionTitle = generateDefaultTitle();

					// Clear the URL parameter after applying
					window.history.replaceState(null, '', '#/chat');
				}
			}
		}

		fetchSessions();
		fetchEmbeddedDatasets();
		fetchLLMs();
	});

	onDestroy(() => {
		if (chatStream) {
			chatStream.cleanup();
		}
	});
</script>

<div class="flex h-full overflow-hidden bg-gray-50 dark:bg-gray-900">
	<!-- Sidebar with sessions -->
	<ChatSidebar
		{sessions}
		{embeddedDatasets}
		{llms}
		{currentSession}
		onSelectSession={selectSession}
		onNewSession={() => {
			newSessionTitle = generateDefaultTitle();
			newSessionEmbeddedDatasetId = null;
			newSessionLLMId = null;
			showCreateForm = true;
		}}
	/>

	<!-- Main chat area -->
	<div class="flex-1 flex flex-col overflow-hidden">
		{#if !currentSession}
			<div class="flex-1 flex items-center justify-center">
				<PageHeader
					title="Welcome to Chat"
					description="Select a chat session from sidebar or create a new one to get started."
				/>
			</div>
		{:else}
			<!-- Chat header -->
			<div
				class="shrink-0 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-6 py-4"
			>
				<div class="flex items-center justify-between">
					<div>
						<h2 class="text-xl font-bold text-gray-900 dark:text-white">
							{currentSession.title || 'Untitled Chat'}
						</h2>
						<p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
							{getEmbeddedDatasetTitle(currentSession.embedded_dataset_id)} •
							{getLLMTitle(currentSession.llm_id)}
						</p>
					</div>
					<button
						onclick={() => {
							if (currentSession) {
								deleteSession(currentSession);
							}
						}}
						class="px-4 py-2 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors text-sm"
					>
						Delete
					</button>
				</div>
			</div>

			<!-- Messages area -->
			<div class="flex-1 min-h-0 overflow-y-auto" bind:this={messagesContainer}>
				<div class="space-y-4 p-6">
					{#if messagesLoading}
						<div class="flex justify-center">
							<div class="text-gray-500 dark:text-gray-400">Loading messages...</div>
						</div>
					{:else if currentMessages.length === 0}
						<div class="flex items-center justify-center">
							<div class="text-center text-gray-500 dark:text-gray-400">
								<p class="mb-2">Start a conversation</p>
								<p class="text-sm">Type a message below to begin chatting with the RAG system.</p>
							</div>
						</div>
					{:else}
						{#each currentMessages as message (message.message_id)}
							<ChatMessage
								{message}
								sourceDatasetId={embeddedDatasets.find(
									(d) => d.embedded_dataset_id === currentSession?.embedded_dataset_id
								)?.source_dataset_id}
								onRegenerate={async (messageId) => {
									if (chatStream) {
										await chatStream.regenerateMessage(messageId);
										await fetchMessagesForCurrentSession();
									}
								}}
							/>
						{/each}
					{/if}
				</div>
			</div>

			<!-- Input area -->
			<div
				class="shrink-0 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 p-6"
			>
				{#if chatStream && chatStream.streamingState.status}
					<div
						class="mb-4 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg"
					>
						<div class="flex items-center gap-2 text-sm text-blue-600 dark:text-blue-400">
							{#if chatStream.streamingState.status === 'connecting'}
								<span>Connecting to server...</span>
							{:else if chatStream.streamingState.status === 'retrieving'}
								<span>Retrieving relevant documents...</span>
							{:else if chatStream.streamingState.status === 'generating'}
								<span>
									Generating response...
									{#if chatStream.streamingState.progress}
										({chatStream.streamingState.progress.charCount} chars, {chatStream
											.streamingState.progress.elapsedSeconds}s)
									{/if}
								</span>
							{/if}
						</div>
					</div>
				{/if}

				<!-- Chat Settings (Collapsible) -->
				<div class="mb-4">
					<button
						onclick={() => (settingsExpanded = !settingsExpanded)}
						class="flex items-center gap-2 text-sm font-medium text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
					>
						<svg
							class="w-4 h-4 transition-transform {settingsExpanded ? 'rotate-90' : ''}"
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
						RAG & LLM Settings
					</button>
					{#if settingsExpanded}
						<div class="mt-3">
							<ChatSettings
								{maxChunks}
								{minSimilarityScore}
								{temperature}
								{maxTokens}
								{systemPrompt}
								onMaxChunksChange={(v) => (maxChunks = v)}
								onMinSimilarityScoreChange={(v) => (minSimilarityScore = v)}
								onTemperatureChange={(v) => (temperature = v)}
								onMaxTokensChange={(v) => (maxTokens = v)}
								onSystemPromptChange={(v) => (systemPrompt = v)}
							/>
						</div>
					{/if}
				</div>

				<!-- Input -->
				<ChatInput
					bind:value={inputValue}
					disabled={chatStream ? chatStream.isGenerating : false}
					onSend={handleSendMessage}
					onKeyDown={(e) => {
						if (e.key === 'Enter' && !e.shiftKey) {
							e.preventDefault();
							handleSendMessage();
						}
					}}
				/>
			</div>
		{/if}
	</div>

	<!-- Create Session Modal -->
	{#if showCreateForm}
		<div
			class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
			onclick={(e) => {
				if (e.target === e.currentTarget) showCreateForm = false;
			}}
			onkeydown={(e) => {
				if (e.key === 'Escape') showCreateForm = false;
			}}
			role="dialog"
			aria-modal="true"
			tabindex="-1"
		>
			<div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl w-full max-w-md p-6">
				<h3 class="text-xl font-bold text-gray-900 dark:text-white mb-4">Create New Session</h3>

				<div class="space-y-4">
					<div>
						<label
							for="session-title"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Session Title
						</label>
						<input
							id="session-title"
							type="text"
							bind:value={newSessionTitle}
							placeholder="Enter session title..."
							class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						/>
					</div>

					<div>
						<label
							for="embedded-dataset"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Embedded Dataset <span class="text-red-500">*</span>
						</label>
						<select
							id="embedded-dataset"
							bind:value={newSessionEmbeddedDatasetId}
							class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select an embedded dataset...</option>
							{#each embeddedDatasets as dataset (dataset.embedded_dataset_id)}
								<option value={dataset.embedded_dataset_id}>
									{dataset.title}
								</option>
							{/each}
						</select>
					</div>

					<div>
						<label
							for="llm"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							LLM <span class="text-red-500">*</span>
						</label>
						<select
							id="llm"
							bind:value={newSessionLLMId}
							class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select an LLM...</option>
							{#each llms as llm (llm.llm_id)}
								<option value={llm.llm_id}>
									{llm.name} ({llm.provider})
								</option>
							{/each}
						</select>
					</div>
				</div>

				<div class="flex justify-end gap-3 mt-6">
					<button
						onclick={() => (showCreateForm = false)}
						class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
					>
						Cancel
					</button>
					<button
						onclick={createSession}
						disabled={!newSessionEmbeddedDatasetId || !newSessionLLMId}
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
					>
						Create Session
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>
