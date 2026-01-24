<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import ChatMessage from '../components/ChatMessage.svelte';
	import ChatInput from '../components/ChatInput.svelte';
	import ChatSettings from '../components/ChatSettings.svelte';
	import ChatSidebar from '../components/ChatSidebar.svelte';
	import type {
		ChatSession,
		EmbeddedDataset,
		LLM,
		ChatMessage as ChatMessageType,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { useChatStream } from '../composables/useChatStream.svelte';

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

	// RAG Configuration
	let maxChunks = $state(20);
	let minSimilarityScore = $state(0.2);

	// LLM Configuration
	let temperature = $state(0.7);
	let maxTokens = $state(2000);

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
	}

	async function fetchSessions() {
		try {
			const response = await fetch('/api/chat/sessions');
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
			embeddedDatasets = data.embedded_datasets;
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

			// Initialize chat stream
			chatStream = useChatStream({
				sessionId: newSession.session_id,
				content: '',
				maxChunks,
				minSimilarityScore,
				temperature,
				maxTokens,
				callbacks: {},
			});

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

			// Initialize chat stream
			chatStream = useChatStream({
				sessionId: session.session_id,
				content: '',
				maxChunks,
				minSimilarityScore,
				temperature,
				maxTokens,
				callbacks: {},
			});

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
		onNewSession={async () => {
			newSessionTitle = generateDefaultTitle();
			// Auto-select first items
			if (embeddedDatasets.length > 0 && !newSessionEmbeddedDatasetId) {
				newSessionEmbeddedDatasetId = embeddedDatasets[0].embedded_dataset_id;
			}
			if (llms.length > 0 && !newSessionLLMId) {
				newSessionLLMId = llms[0].llm_id;
			}
			await createSession();
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
								onRegenerate={(messageId) => {
									if (chatStream) {
										chatStream.regenerateMessage(messageId);
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

				<!-- Chat Settings -->
				<ChatSettings
					{maxChunks}
					{minSimilarityScore}
					{temperature}
					{maxTokens}
					onMaxChunksChange={(v) => (maxChunks = v)}
					onMinSimilarityScoreChange={(v) => (minSimilarityScore = v)}
					onTemperatureChange={(v) => (temperature = v)}
					onMaxTokensChange={(v) => (maxTokens = v)}
				/>

				<!-- Input -->
				<ChatInput
					value=""
					disabled={chatStream ? chatStream.isGenerating : false}
					onSend={() => {
						if (chatStream) {
							chatStream.sendMessage();
						}
					}}
					onKeyDown={(e) => {
						if (e.key === 'Enter' && !e.shiftKey) {
							e.preventDefault();
							if (chatStream) {
								chatStream.sendMessage();
							}
						}
					}}
				/>

				{#if chatStream && chatStream.streamingState.status}
					<div class="flex gap-3">
						<button
							onclick={() => {
								if (chatStream && chatStream.messages.length > 0) {
									const lastMessage = chatStream.messages[chatStream.messages.length - 1];
									chatStream.regenerateMessage(lastMessage.message_id);
								}
							}}
							disabled={chatStream ? chatStream.isGenerating : false}
							class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
						>
							{chatStream ? (chatStream.isGenerating ? 'Generating...' : 'Send') : 'Send'}
						</button>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>
