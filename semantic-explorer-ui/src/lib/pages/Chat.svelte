<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { marked } from 'marked';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	// Lazy load highlight.js
	let hljsModule: typeof import('highlight.js').default | null = null;
	let hljsLoaded = false;

	async function loadHighlightJS() {
		if (!hljsLoaded) {
			const [hljs] = await Promise.all([
				import('highlight.js'),
				import('highlight.js/styles/github-dark.css'),
			]);
			hljsModule = hljs.default;
			hljsLoaded = true;
		}
		return hljsModule!;
	}

	// Configure marked options
	marked.setOptions({
		breaks: true,
		gfm: true,
	});

	interface ChatSession {
		session_id: string;
		embedded_dataset_id: number;
		llm_id: number;
		title: string | null;
		created_at: string;
		updated_at: string;
	}

	interface RetrievedDocument {
		document_id: string | null;
		text: string;
		similarity_score: number;
		source: string | null;
	}

	interface ChatMessage {
		message_id: number;
		role: string;
		content: string;
		documents_retrieved: number | null;
		created_at: string;
		retrieved_documents?: RetrievedDocument[];
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		embedder_name: string;
		source_dataset_title: string;
		source_dataset_id: number;
	}

	interface LLM {
		llm_id: number;
		name: string;
		provider: string;
	}

	let sessions = $state<ChatSession[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let llms = $state<LLM[]>([]);
	let loading = $state(true);

	let currentSession = $state<ChatSession | null>(null);
	let currentMessages = $state<ChatMessage[]>([]);
	let messagesLoading = $state(false);

	let showCreateForm = $state(false);
	let newSessionTitle = $state('');
	let newSessionEmbeddedDatasetId = $state<number | null>(null);
	let newSessionLLMId = $state<number | null>(null);
	let creatingSession = $state(false);
	let createSessionError = $state<string | null>(null);

	let messageInput = $state('');
	let sendingMessage = $state(false);
	let messageError = $state<string | null>(null);

	// Track expanded state of retrieved documents per message
	let expandedDocs = $state<Record<number, boolean>>({});

	async function fetchSessions() {
		try {
			loading = true;
			messageError = null;
			const response = await fetch('/api/chat/sessions');
			if (!response.ok) {
				throw new Error(`Failed to fetch chat sessions: ${response.statusText}`);
			}
			const data = await response.json();
			sessions = data.sessions;
		} catch (e) {
			const message = formatError(e, 'Failed to fetch chat sessions');
			messageError = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchEmbeddedDatasets() {
		try {
			const response = await fetch('/api/embedded-datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			embeddedDatasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch embedded datasets:', e);
		}
	}

	async function fetchLLMs() {
		try {
			const response = await fetch('/api/llms');
			if (!response.ok) {
				throw new Error(`Failed to fetch LLMs: ${response.statusText}`);
			}
			llms = await response.json();
		} catch (e) {
			console.error('Failed to fetch LLMs:', e);
		}
	}

	async function createSession() {
		if (!newSessionEmbeddedDatasetId || !newSessionLLMId) {
			createSessionError = 'Please select both an embedded dataset and an LLM';
			return;
		}

		try {
			creatingSession = true;
			createSessionError = null;

			const response = await fetch('/api/chat/sessions', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					embedded_dataset_id: newSessionEmbeddedDatasetId,
					llm_id: newSessionLLMId,
					title: newSessionTitle || null,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to create chat session: ${response.statusText}`);
			}

			const newSession = await response.json();
			sessions = [...sessions, newSession];
			toastStore.success('Chat session created successfully');

			// Select the new session
			currentSession = newSession;
			currentMessages = [];
			messageInput = '';

			resetCreateForm();
		} catch (e) {
			const message = formatError(e, 'Failed to create chat session');
			createSessionError = message;
			toastStore.error(message);
		} finally {
			creatingSession = false;
		}
	}

	async function selectSession(session: ChatSession) {
		try {
			messagesLoading = true;
			currentSession = session;
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

	async function sendMessage() {
		if (!messageInput.trim() || !currentSession) {
			return;
		}

		try {
			sendingMessage = true;
			messageError = null;

			const response = await fetch(`/api/chat/sessions/${currentSession.session_id}/messages`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					content: messageInput,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to send message: ${response.statusText}`);
			}

			const result = await response.json();

			// Add user message
			currentMessages = [
				...currentMessages,
				{
					message_id: currentMessages.length + 1,
					role: 'user',
					content: messageInput,
					documents_retrieved: null,
					created_at: new Date().toISOString(),
				},
			];

			// Add assistant message with retrieved documents
			currentMessages = [
				...currentMessages,
				{
					message_id: currentMessages.length + 1,
					role: 'assistant',
					content: result.content,
					documents_retrieved: result.documents_retrieved,
					created_at: new Date().toISOString(),
					retrieved_documents: result.retrieved_documents || [],
				},
			];

			messageInput = '';
		} catch (e) {
			const message = formatError(e, 'Failed to send message');
			messageError = message;
			toastStore.error(message);
		} finally {
			sendingMessage = false;
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
			const message = formatError(e, 'Failed to delete session');
			toastStore.error(message);
		}
	}

	function resetCreateForm() {
		showCreateForm = false;
		newSessionTitle = '';
		newSessionEmbeddedDatasetId = null;
		newSessionLLMId = null;
		createSessionError = null;
	}

	function getEmbeddedDatasetTitle(id: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === id);
		return dataset ? `${dataset.title} (${dataset.embedder_name})` : `Dataset ${id}`;
	}

	function getSourceDatasetId(embeddedDatasetId: number): number | null {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === embeddedDatasetId);
		return dataset ? dataset.source_dataset_id : null;
	}

	function getLLMTitle(id: number): string {
		const llm = llms.find((l) => l.llm_id === id);
		return llm ? `${llm.name} (${llm.provider})` : `LLM ${id}`;
	}

	async function renderMarkdown(content: string): Promise<string> {
		const html = marked.parse(content) as string;
		// Apply syntax highlighting to code blocks after rendering
		const div = document.createElement('div');
		div.innerHTML = html;

		const codeBlocks = div.querySelectorAll('pre code');
		if (codeBlocks.length > 0) {
			const hljs = await loadHighlightJS();
			codeBlocks.forEach((block) => {
				hljs.highlightElement(block as HTMLElement);
			});
		}

		return div.innerHTML;
	}

	onMount(() => {
		fetchSessions();
		fetchEmbeddedDatasets();
		fetchLLMs();
	});
</script>

<div class="flex h-full overflow-hidden bg-gray-50 dark:bg-gray-900">
	<!-- Sidebar with sessions -->
	<div
		class="w-72 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col overflow-hidden"
	>
		<div class="p-6 border-b border-gray-200 dark:border-gray-700">
			<h1 class="text-2xl font-bold text-gray-900 dark:text-white">Chat</h1>
			<p class="text-sm text-gray-600 dark:text-gray-400 mt-1">RAG-powered conversations</p>
		</div>

		<button
			onclick={() => {
				if (showCreateForm) {
					resetCreateForm();
				} else {
					showCreateForm = true;
				}
			}}
			class="mx-4 my-4 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors text-sm font-medium"
		>
			{showCreateForm ? 'Cancel' : '+ New Chat'}
		</button>

		{#if showCreateForm}
			<div class="mx-4 mb-4 p-4 bg-blue-50 dark:bg-blue-900/10 rounded-lg">
				<div class="mb-3">
					<label
						for="session-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Session Title (Optional)
					</label>
					<input
						id="session-title"
						type="text"
						bind:value={newSessionTitle}
						placeholder="My conversation..."
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
					/>
				</div>

				<div class="mb-3">
					<label
						for="session-dataset"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Embedded Dataset
					</label>
					<select
						id="session-dataset"
						bind:value={newSessionEmbeddedDatasetId}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
					>
						<option value={null}>Select dataset...</option>
						{#each embeddedDatasets as dataset (dataset.embedded_dataset_id)}
							<option value={dataset.embedded_dataset_id}>{dataset.title}</option>
						{/each}
					</select>
				</div>

				<div class="mb-3">
					<label
						for="session-llm"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						LLM
					</label>
					<select
						id="session-llm"
						bind:value={newSessionLLMId}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
					>
						<option value={null}>Select LLM...</option>
						{#each llms as llm (llm.llm_id)}
							<option value={llm.llm_id}>{llm.name}</option>
						{/each}
					</select>
				</div>

				{#if createSessionError}
					<div
						class="mb-3 p-2 bg-red-100 dark:bg-red-900/20 rounded text-sm text-red-700 dark:text-red-400"
					>
						{createSessionError}
					</div>
				{/if}

				<button
					onclick={createSession}
					disabled={creatingSession}
					class="w-full px-3 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 text-sm font-medium"
				>
					{creatingSession ? 'Creating...' : 'Create'}
				</button>
			</div>
		{/if}

		<div class="flex-1 overflow-y-auto">
			{#if loading}
				<div class="p-4 text-center text-gray-500 dark:text-gray-400">Loading...</div>
			{:else if sessions.length === 0}
				<div class="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
					No chat sessions yet. Create one to get started!
				</div>
			{:else}
				{#each sessions as session (session.session_id)}
					<button
						onclick={() => selectSession(session)}
						class="w-full text-left px-4 py-3 border-l-4 transition-colors {currentSession?.session_id ===
						session.session_id
							? 'border-blue-600 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400'
							: 'border-transparent hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300'}"
					>
						<div class="font-medium text-sm">{session.title || 'Untitled'}</div>
						<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							{getEmbeddedDatasetTitle(session.embedded_dataset_id)}
						</div>
					</button>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Main chat area -->
	<div class="flex-1 flex flex-col overflow-hidden">
		{#if !currentSession}
			<div class="flex-1 flex items-center justify-center">
				<PageHeader
					title="Welcome to Chat"
					description="Select a chat session from the sidebar or create a new one to get started."
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
						onclick={() => deleteSession(currentSession)}
						class="px-4 py-2 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors text-sm"
					>
						Delete
					</button>
				</div>
			</div>

			<!-- Messages area -->
			<div class="flex-1 min-h-0 overflow-y-auto">
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
							<div class="flex {message.role === 'user' ? 'justify-end' : 'justify-start'} gap-4">
								<div
									class="max-w-2xl {message.role === 'user'
										? 'bg-blue-600 text-white'
										: 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white'} rounded-lg px-4 py-2"
								>
									{#if message.role === 'user'}
										<p class="text-sm">{message.content}</p>
									{:else}
										<div class="prose prose-sm dark:prose-invert max-w-none">
											{#await renderMarkdown(message.content) then html}
												{@html html}
											{/await}
										</div>
									{/if}
									{#if message.role === 'assistant' && message.retrieved_documents && message.retrieved_documents.length > 0}
										<div class="mt-3 pt-3 border-t border-current opacity-75">
											<button
												onclick={() => {
													expandedDocs[message.message_id] = !expandedDocs[message.message_id];
												}}
												class="flex items-center justify-between w-full text-xs font-semibold mb-2 hover:opacity-80 transition-opacity"
											>
												<span>
													Retrieved {message.documents_retrieved} chunk{message.documents_retrieved !==
													1
														? 's'
														: ''}
												</span>
												<span class="ml-2">
													{expandedDocs[message.message_id] ? '▼' : '▶'}
												</span>
											</button>
											{#if expandedDocs[message.message_id]}
												<div class="space-y-2 mt-2">
													{#each message.retrieved_documents as doc, idx (doc.document_id || idx)}
														<div class="text-xs p-2 rounded bg-gray-300 dark:bg-gray-600">
															<div class="flex items-start justify-between mb-1">
																<span class="font-semibold text-[10px] opacity-70">
																	Chunk {idx + 1}
																</span>
																<span class="font-mono text-[10px] opacity-70">
																	Score: {doc.similarity_score.toFixed(3)}
																</span>
															</div>
															<p class="mb-1 leading-relaxed">{doc.text}</p>
															{#if doc.source}
																<a
																	href="ui#/datasets/{getSourceDatasetId(
																		currentSession?.embedded_dataset_id || 0
																	)}/details?q={encodeURIComponent(doc.source)}"
																	class="text-[10px] underline hover:font-semibold transition-all opacity-70 hover:opacity-100"
																	target="_blank"
																>
																	Source: {doc.source}
																</a>
															{/if}
														</div>
													{/each}
												</div>
											{/if}
										</div>
									{/if}
								</div>
							</div>
						{/each}
					{/if}
				</div>
			</div>

			<!-- Input area -->
			<div
				class="shrink-0 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 p-6"
			>
				{#if messageError}
					<div
						class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg"
					>
						<p class="text-sm text-red-600 dark:text-red-400">{messageError}</p>
					</div>
				{/if}

				<div class="flex gap-3">
					<input
						type="text"
						bind:value={messageInput}
						onkeydown={(e) => {
							if (e.key === 'Enter' && !e.shiftKey) {
								e.preventDefault();
								sendMessage();
							}
						}}
						placeholder="Type your message... (Enter to send)"
						disabled={sendingMessage}
						class="flex-1 px-4 py-3 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 disabled:opacity-50"
					/>
					<button
						onclick={sendMessage}
						disabled={!messageInput.trim() || sendingMessage}
						class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
					>
						{sendingMessage ? 'Sending...' : 'Send'}
					</button>
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	:global(body) {
		margin: 0;
		padding: 0;
		overflow: hidden;
	}

	/* Style the markdown content in messages */
	:global(.prose code) {
		padding: 0.125rem 0.25rem;
		border-radius: 0.25rem;
		background-color: rgb(55 65 81);
		color: rgb(243 244 246);
		font-size: 0.875em;
	}

	:global(.prose pre) {
		border-radius: 0.5rem;
		background-color: rgb(31 41 55);
		padding: 1rem;
		overflow-x: auto;
		margin: 0.5rem 0;
	}

	:global(.prose pre code) {
		background-color: transparent;
		padding: 0;
		color: inherit;
		font-size: 0.875rem;
	}

	:global(.prose p) {
		margin: 0.5rem 0;
		line-height: 1.625;
	}

	:global(.prose ul),
	:global(.prose ol) {
		margin: 0.5rem 0;
		margin-left: 1rem;
		padding-left: 0.5rem;
	}

	:global(.prose li) {
		margin: 0.25rem 0;
	}

	:global(.prose h1),
	:global(.prose h2),
	:global(.prose h3),
	:global(.prose h4),
	:global(.prose h5),
	:global(.prose h6) {
		margin-top: 1rem;
		margin-bottom: 0.5rem;
		font-weight: 600;
		line-height: 1.25;
	}

	:global(.prose h1) {
		font-size: 1.5rem;
	}

	:global(.prose h2) {
		font-size: 1.25rem;
	}

	:global(.prose h3) {
		font-size: 1.125rem;
	}

	:global(.prose blockquote) {
		border-left: 4px solid rgb(156 163 175);
		padding-left: 1rem;
		font-style: italic;
		margin: 0.5rem 0;
		color: rgb(156 163 175);
	}

	:global(.prose a) {
		color: rgb(96 165 250);
		text-decoration: underline;
	}

	:global(.prose a:hover) {
		color: rgb(147 197 253);
	}

	:global(.prose table) {
		border-collapse: collapse;
		width: 100%;
		margin: 0.5rem 0;
		border: 1px solid rgb(75 85 99);
	}

	:global(.prose th),
	:global(.prose td) {
		border: 1px solid rgb(75 85 99);
		padding: 0.5rem 0.75rem;
		text-align: left;
	}

	:global(.prose th) {
		background-color: rgb(55 65 81);
		font-weight: 600;
	}

	:global(.prose strong) {
		font-weight: 600;
	}

	:global(.prose em) {
		font-style: italic;
	}

	:global(.prose hr) {
		border: none;
		border-top: 1px solid rgb(75 85 99);
		margin: 1rem 0;
	}
</style>
