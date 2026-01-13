<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import DOMPurify from 'dompurify';
	import { marked } from 'marked';
	import { onMount } from 'svelte';
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
		item_title: string | null;
	}

	interface ChatMessage {
		message_id: number;
		role: string;
		content: string;
		documents_retrieved: number | null;
		status: string; // 'complete', 'incomplete', 'error'
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
	let messageError = $state<string | null>(null);

	// Streaming state
	let isGenerating = $state(false);
	let queuedAction: (() => Promise<void>) | null = $state(null);
	let streamingProgress = $state<{
		messageId: number;
		charCount: number;
		elapsedSeconds: number;
	} | null>(null);
	let streamingStatus = $state<'connecting' | 'retrieving' | 'generating' | null>(null);

	// RAG Configuration
	let maxChunks = $state(20);
	let minSimilarityScore = $state(0.2);
	let showRagSettings = $state(false);

	// LLM Configuration
	let temperature = $state(0.7);
	let maxTokens = $state(2000);

	// Track expanded state of retrieved documents per message (collapsed by default)
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

			// Use the title that's already pre-filled (or trim if edited)
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

	async function sendMessageStreaming() {
		if (!messageInput.trim() || !currentSession || isGenerating) {
			return;
		}

		try {
			isGenerating = true;
			streamingStatus = 'connecting';
			messageError = null;

			const userMessageId = Date.now();
			const userContent = messageInput;
			messageInput = ''; // Clear input immediately (optimistic)

			// Add user message optimistically
			currentMessages = [
				...currentMessages,
				{
					message_id: userMessageId,
					role: 'user',
					content: userContent,
					documents_retrieved: null,
					status: 'complete',
					created_at: new Date().toISOString(),
				},
			];

			// Add placeholder assistant message
			const assistantPlaceholderId = Date.now() + 1;
			currentMessages = [
				...currentMessages,
				{
					message_id: assistantPlaceholderId,
					role: 'assistant',
					content: '',
					documents_retrieved: 0,
					status: 'incomplete',
					created_at: new Date().toISOString(),
					retrieved_documents: [],
				},
			];

			// Use fetch with POST for streaming
			const response = await fetch(
				`/api/chat/sessions/${currentSession.session_id}/messages/stream`,
				{
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						content: userContent,
						max_context_documents: maxChunks,
						min_similarity_score: minSimilarityScore,
						temperature: temperature,
						max_tokens: maxTokens,
					}),
				}
			);

			if (!response.ok) {
				throw new Error(`HTTP error! status: ${response.status}`);
			}

			let accumulatedContent = '';
			let actualMessageId: number | null = null;
			let retrievedDocs: RetrievedDocument[] = [];

			// Process the streaming response
			const reader = response.body?.getReader();
			if (!reader) {
				throw new Error('Response body is not readable');
			}

			const decoder = new TextDecoder();
			let buffer = '';

			let currentEventType = '';

			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				buffer += decoder.decode(value, { stream: true });
				const lines = buffer.split('\n');

				// Keep the last incomplete line in the buffer
				buffer = lines.pop() || '';

				for (const line of lines) {
					// Track the event type from 'event:' lines (standard SSE format)
					if (line.startsWith('event:')) {
						currentEventType = line.substring(6).trim();
						continue;
					}

					if (!line.startsWith('data:')) continue;

					const data_str = line.substring(5).trim();
					if (!data_str) continue;

					try {
						const data = JSON.parse(data_str);
						// Use the event type from the 'event:' line, or fall back to data.type for compatibility
						const eventType = currentEventType || data.type;
						// Reset for next event
						currentEventType = '';

						if (eventType === 'connected') {
							streamingStatus = 'retrieving';
						} else if (eventType === 'retrieval_complete') {
							actualMessageId = data.message_id;
							retrievedDocs = data.documents || [];
							if (data.text) {
								accumulatedContent += data.text;
							}

							// Update placeholder with actual message ID and documents
							currentMessages = currentMessages.map((msg) =>
								msg.message_id === assistantPlaceholderId
									? {
											...msg,
											message_id: actualMessageId!,
											retrieved_documents: retrievedDocs,
											documents_retrieved: retrievedDocs.length,
											content: accumulatedContent,
										}
									: msg
							) as ChatMessage[];
							streamingStatus = 'generating';
							// Initialize progress immediately
							streamingProgress = {
								messageId: actualMessageId!,
								charCount: 0,
								elapsedSeconds: 0,
							};
						} else if (eventType === 'content') {
							const chunk = data.content || data.text || '';
							accumulatedContent += chunk;
							currentMessages = currentMessages.map((msg) =>
								msg.message_id === (actualMessageId || assistantPlaceholderId)
									? { ...msg, content: accumulatedContent }
									: msg
							);
						} else if (eventType === 'progress') {
							streamingProgress = {
								messageId: data.message_id,
								charCount: data.char_count,
								elapsedSeconds: data.elapsed_seconds,
							};
						} else if (eventType === 'complete') {
							const finalContent = data.content || accumulatedContent;
							currentMessages = currentMessages.map((msg) =>
								msg.message_id === data.message_id
									? { ...msg, status: 'complete', content: finalContent }
									: msg
							);

							streamingProgress = null;
							streamingStatus = null;
							isGenerating = false;

							// Execute queued action if any
							if (queuedAction) {
								const action = queuedAction;
								queuedAction = null;
								action();
							}
						} else if (eventType === 'error') {
							const error = data.error || 'Streaming error occurred';
							messageError = error;
							toastStore.error(error);

							// Update message status to error
							if (actualMessageId) {
								currentMessages = currentMessages.map((msg) =>
									msg.message_id === actualMessageId ? { ...msg, status: 'error' } : msg
								);
							}

							streamingProgress = null;
							streamingStatus = null;
							isGenerating = false;
						}
					} catch (e) {
						console.error('Error parsing SSE data:', e, 'Line:', line);
					}
				}
			}

			// Process any remaining data in the buffer after stream ends
			if (buffer.trim()) {
				const remainingLines = buffer.split('\n');
				for (const line of remainingLines) {
					if (line.startsWith('event:')) {
						currentEventType = line.substring(6).trim();
					} else if (line.startsWith('data:')) {
						const data_str = line.substring(5).trim();
						if (data_str) {
							try {
								const data = JSON.parse(data_str);
								const eventType = currentEventType || data.type;
								if (eventType === 'content') {
									const chunk = data.content || data.text || '';
									accumulatedContent += chunk;
								} else if (eventType === 'complete') {
									currentMessages = currentMessages.map((msg) =>
										msg.message_id === data.message_id
											? { ...msg, status: 'complete', content: accumulatedContent }
											: msg
									);
								}
							} catch (e) {
								console.error('Error parsing remaining SSE data:', e);
							}
						}
						currentEventType = '';
					}
				}
			}

			// Ensure cleanup happens after stream ends
			if (isGenerating) {
				// Stream ended without a complete event, update UI anyway
				if (actualMessageId) {
					currentMessages = currentMessages.map((msg) =>
						msg.message_id === actualMessageId
							? { ...msg, status: 'complete', content: accumulatedContent }
							: msg
					);
				}
				isGenerating = false;
				streamingStatus = null;
				streamingProgress = null;
			}
		} catch (error) {
			messageError = error instanceof Error ? error.message : 'An error occurred';
			toastStore.error(messageError);
			isGenerating = false;
			streamingStatus = null;
			streamingProgress = null;
		}
	}

	async function regenerateMessage(messageId: number) {
		if (isGenerating) {
			// Queue the regeneration
			queuedAction = () => regenerateMessage(messageId);
			toastStore.info('Regeneration queued - will start when current generation completes');
			return;
		}

		try {
			isGenerating = true;
			messageError = null;

			// Update message status to incomplete
			currentMessages = currentMessages.map((msg) =>
				msg.message_id === messageId ? { ...msg, status: 'incomplete', content: '' } : msg
			);

			const response = await fetch(`/api/chat/messages/${messageId}/regenerate?stream=false`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to regenerate message: ${response.statusText}`);
			}

			const result = await response.json();

			// Update message with new content
			currentMessages = currentMessages.map((msg) =>
				msg.message_id === messageId
					? {
							...msg,
							content: result.content,
							status: 'complete',
							retrieved_documents: result.retrieved_documents || [],
						}
					: msg
			);

			toastStore.success('Message regenerated successfully');
		} catch (e) {
			const message = formatError(e, 'Failed to regenerate message');
			messageError = message;
			toastStore.error(message);

			// Update message status to error
			currentMessages = currentMessages.map((msg) =>
				msg.message_id === messageId ? { ...msg, status: 'error' } : msg
			);
		} finally {
			isGenerating = false;

			// Execute queued action if any
			if (queuedAction) {
				const action = queuedAction;
				queuedAction = null;
				action();
			}
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

	function getLLMTitle(id: number): string {
		const llm = llms.find((l) => l.llm_id === id);
		return llm ? `${llm.name} (${llm.provider})` : `LLM ${id}`;
	}

	async function renderMarkdown(
		content: string,
		retrievedDocs?: RetrievedDocument[]
	): Promise<string> {
		let processedContent = content;

		if (retrievedDocs && retrievedDocs.length > 0) {
			// Get all unique document titles
			const titles = new Set(
				retrievedDocs.map((doc) => doc.item_title).filter((title): title is string => !!title)
			);

			// Apply blue styling to each title when it appears in the content
			titles.forEach((title) => {
				// Escape special regex characters in title
				const escapedTitle = title.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
				// Replace title with styled version, but only in text (not in URLs or markdown links)
				const regex = new RegExp(`(?<!\\[)\\b${escapedTitle}\\b(?![\\]\\(])`, 'g');
				processedContent = processedContent.replace(
					regex,
					`**<span style="color: rgb(37 99 235)">${title}</span>**`
				);
			});
		}

		const html = marked.parse(processedContent) as string;
		// Sanitize HTML to prevent XSS attacks
		const sanitizedHtml = DOMPurify.sanitize(html, {
			ALLOWED_TAGS: [
				'h1',
				'h2',
				'h3',
				'h4',
				'h5',
				'h6',
				'p',
				'br',
				'hr',
				'ul',
				'ol',
				'li',
				'blockquote',
				'pre',
				'code',
				'span',
				'strong',
				'em',
				'a',
				'table',
				'thead',
				'tbody',
				'tr',
				'th',
				'td',
				'img',
			],
			ALLOWED_ATTR: ['href', 'target', 'rel', 'class', 'style', 'src', 'alt', 'title'],
			ALLOW_DATA_ATTR: false,
		});
		// Apply syntax highlighting to code blocks after rendering
		const div = document.createElement('div');
		div.innerHTML = sanitizedHtml;

		const codeBlocks = div.querySelectorAll('pre code');
		if (codeBlocks.length > 0) {
			const hljs = await loadHighlightJS();
			codeBlocks.forEach((block) => {
				hljs.highlightElement(block as HTMLElement);
			});
		}

		return div.innerHTML;
	}

	function getDeduplicatedReferences(
		retrievedDocs: RetrievedDocument[]
	): Array<{ title: string; count: number }> {
		const refMap: Record<string, number> = {};
		retrievedDocs.forEach((doc) => {
			const title = doc.item_title || 'Unknown';
			refMap[title] = (refMap[title] || 0) + 1;
		});
		return Object.entries(refMap)
			.map(([title, count]) => ({ title, count }))
			.sort((a, b) => b.count - a.count);
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
					showCreateForm = true;

					// Clear the URL parameter after applying
					window.history.replaceState(null, '', '#/chat');
				}
			}
		}

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
					// Pre-fill the title with a generated default
					newSessionTitle = generateDefaultTitle();
					// Auto-select first items
					if (embeddedDatasets.length > 0 && !newSessionEmbeddedDatasetId) {
						newSessionEmbeddedDatasetId = embeddedDatasets[0].embedded_dataset_id;
					}
					if (llms.length > 0 && !newSessionLLMId) {
						newSessionLLMId = llms[0].llm_id;
					}
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
						Session Title
					</label>
					<input
						id="session-title"
						type="text"
						bind:value={newSessionTitle}
						placeholder="Title of the chat session"
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
										{#if message.content}
											<div class="prose prose-sm dark:prose-invert max-w-none">
												{#await renderMarkdown(message.content, message.retrieved_documents) then html}
													{@html html}
												{/await}
											</div>
										{:else if message.status === 'incomplete' && isGenerating}
											<!-- Show loading state while streaming but no content yet -->
											<div class="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
												<svg
													class="animate-spin h-4 w-4"
													xmlns="http://www.w3.org/2000/svg"
													fill="none"
													viewBox="0 0 24 24"
												>
													<circle
														class="opacity-25"
														cx="12"
														cy="12"
														r="10"
														stroke="currentColor"
														stroke-width="4"
													></circle>
													<path
														class="opacity-75"
														fill="currentColor"
														d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
													></path>
												</svg>
												<span>Thinking...</span>
											</div>
										{/if}
										{#if message.status === 'incomplete' && !isGenerating}
											<!-- Only show retry/complete button when NOT actively generating -->
											<div class="mt-3 pt-3 border-t border-current">
												<button
													onclick={() => regenerateMessage(message.message_id)}
													disabled={isGenerating}
													class="text-xs px-3 py-1 bg-yellow-500 hover:bg-yellow-600 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
												>
													⚡ Complete
												</button>
												<span class="ml-2 text-xs opacity-70">Generation incomplete</span>
											</div>
										{:else if message.status === 'error'}
											<div class="mt-3 pt-3 border-t border-current">
												<button
													onclick={() => regenerateMessage(message.message_id)}
													disabled={isGenerating}
													class="text-xs px-3 py-1 bg-yellow-500 hover:bg-yellow-600 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
												>
													🔄 Retry
												</button>
												<span class="ml-2 text-xs text-red-400">Generation failed</span>
											</div>
										{/if}
									{/if}
									{#if message.role === 'assistant' && message.retrieved_documents && message.retrieved_documents.length > 0 && message.status === 'complete'}
										<div class="mt-3 pt-3 border-t border-gray-400 dark:border-gray-500">
											<div class="text-xs font-semibold mb-2 text-gray-600 dark:text-gray-300">
												References
											</div>
											<div class="space-y-1.5">
												{#each getDeduplicatedReferences(message.retrieved_documents) as ref (ref.title)}
													<div class="flex items-center gap-2">
														<a
															href="#/datasets/{embeddedDatasets.find(
																(d) => d.embedded_dataset_id === currentSession?.embedded_dataset_id
															)?.source_dataset_id}/details?search={encodeURIComponent(ref.title)}"
															class="text-blue-600 dark:text-blue-400 hover:underline font-semibold text-xs"
														>
															{ref.title}
														</a>
														<span class="text-gray-500 dark:text-gray-400 text-xs"
															>× {ref.count}</span
														>
													</div>
												{/each}
											</div>
											<button
												onclick={() => {
													expandedDocs[message.message_id] = !expandedDocs[message.message_id];
												}}
												class="flex items-center justify-between w-full text-xs font-semibold mt-3 hover:opacity-80 transition-opacity text-gray-600 dark:text-gray-300"
											>
												<span>Retrieved Chunks ({message.retrieved_documents.length})</span>
												<span class="ml-2">
													{expandedDocs[message.message_id] === true ? '▼' : '▶'}
												</span>
											</button>
											{#if expandedDocs[message.message_id] === true}
												<div class="space-y-2 mt-2">
													{#each message.retrieved_documents as doc, idx (doc.document_id || idx)}
														<div class="text-xs p-2 rounded bg-gray-300 dark:bg-gray-600">
															<div class="flex items-start justify-between mb-1">
																<span class="font-semibold text-[10px] opacity-70">
																	{doc.item_title || `Chunk ${idx + 1}`}
																</span>
																<span class="font-mono text-[10px] opacity-70">
																	Score: {doc.similarity_score.toFixed(3)}
																</span>
															</div>
															<p class="mb-1 leading-relaxed">{doc.text}</p>
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

				<!-- Chat Settings -->
				<div class="mb-4">
					<button
						onclick={() => {
							showRagSettings = !showRagSettings;
						}}
						class="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200 transition-colors"
					>
						<span>{showRagSettings ? '▼' : '▶'}</span>
						<span>Chat Settings</span>
					</button>

					{#if showRagSettings}
						<div
							class="mt-3 p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg border border-gray-200 dark:border-gray-600"
						>
							<div class="grid grid-cols-2 gap-4">
								<div>
									<label
										for="max-chunks"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
									>
										Max Chunks: <span class="font-bold">{maxChunks}</span>
									</label>
									<input
										id="max-chunks"
										type="range"
										min="1"
										max="100"
										step="1"
										bind:value={maxChunks}
										class="slider w-full"
									/>
									<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
										<span>1</span>
										<span>100</span>
									</div>
								</div>
								<div>
									<label
										for="min-score"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
									>
										Min Similarity: <span class="font-bold">{minSimilarityScore.toFixed(2)}</span>
									</label>
									<input
										id="min-score"
										type="range"
										min="0"
										max="1"
										step="0.05"
										bind:value={minSimilarityScore}
										class="slider w-full"
									/>
									<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
										<span>0.00</span>
										<span>1.00</span>
									</div>
								</div>
							</div>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-3">
								Control how many chunks are retrieved and the minimum similarity score threshold.
							</p>

							<hr class="my-4 border-gray-200 dark:border-gray-600" />

							<div class="grid grid-cols-2 gap-4">
								<div>
									<label
										for="temperature"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
									>
										Temperature: <span class="font-bold">{temperature.toFixed(2)}</span>
									</label>
									<input
										id="temperature"
										type="range"
										min="0"
										max="2"
										step="0.1"
										bind:value={temperature}
										class="slider w-full"
									/>
									<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
										<span>0.0 (Precise)</span>
										<span>2.0 (Creative)</span>
									</div>
								</div>
								<div>
									<label
										for="max-tokens"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
									>
										Max Tokens: <span class="font-bold">{maxTokens}</span>
									</label>
									<input
										id="max-tokens"
										type="number"
										min="100"
										max="8000"
										step="100"
										bind:value={maxTokens}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
									/>
									<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">100-8000 tokens</div>
								</div>
							</div>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-3">
								Control the LLM response creativity and maximum length.
							</p>
						</div>
					{/if}
				</div>

				{#if queuedAction}
					<div
						class="mb-4 p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg"
					>
						<p class="text-sm text-yellow-600 dark:text-yellow-400">
							⏳ Regeneration queued - will start when current generation completes
						</p>
					</div>
				{/if}

				{#if streamingStatus}
					<div
						class="mb-4 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg"
					>
						<div class="flex items-center gap-2 text-sm text-blue-600 dark:text-blue-400">
							<svg
								class="animate-spin h-4 w-4"
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
							>
								<circle
									class="opacity-25"
									cx="12"
									cy="12"
									r="10"
									stroke="currentColor"
									stroke-width="4"
								></circle>
								<path
									class="opacity-75"
									fill="currentColor"
									d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
								></path>
							</svg>
							{#if streamingStatus === 'connecting'}
								<span>Connecting to server...</span>
							{:else if streamingStatus === 'retrieving'}
								<span>Retrieving relevant documents...</span>
							{:else if streamingStatus === 'generating'}
								<span>
									Generating response...
									{#if streamingProgress}
										({streamingProgress.charCount} chars, {streamingProgress.elapsedSeconds}s)
									{/if}
								</span>
							{/if}
						</div>
					</div>
				{/if}

				<div class="flex gap-3">
					<input
						type="text"
						bind:value={messageInput}
						onkeydown={(e) => {
							if (e.key === 'Enter' && !e.shiftKey) {
								e.preventDefault();
								sendMessageStreaming();
							}
						}}
						placeholder="Type your message... (Enter to send)"
						disabled={isGenerating}
						class="flex-1 px-4 py-3 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 disabled:opacity-50"
					/>
					<button
						onclick={sendMessageStreaming}
						disabled={!messageInput.trim() || isGenerating}
						class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
					>
						{isGenerating ? 'Generating...' : 'Send'}
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

	:global(.slider) {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: 0.5rem;
		border-radius: 0.5rem;
		background: linear-gradient(to right, rgb(229 231 235), rgb(229 231 235));
		outline: none;
		cursor: pointer;
	}

	:global(.dark .slider) {
		background: linear-gradient(to right, rgb(75 85 99), rgb(75 85 99));
	}

	:global(.slider::-webkit-slider-thumb) {
		-webkit-appearance: none;
		appearance: none;
		width: 1.25rem;
		height: 1.25rem;
		border-radius: 50%;
		background: rgb(37 99 235);
		cursor: pointer;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
	}

	:global(.slider::-moz-range-thumb) {
		width: 1.25rem;
		height: 1.25rem;
		border-radius: 50%;
		background: rgb(37 99 235);
		cursor: pointer;
		border: none;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
	}

	:global(.slider::-webkit-slider-runnable-track) {
		background: linear-gradient(to right, rgb(229 231 235), rgb(229 231 235));
		height: 0.5rem;
		border-radius: 0.5rem;
	}

	:global(.dark .slider::-webkit-slider-runnable-track) {
		background: linear-gradient(to right, rgb(75 85 99), rgb(75 85 99));
	}

	:global(.slider::-moz-range-track) {
		background: transparent;
		border: none;
	}

	:global(.slider::-moz-range-progress) {
		background: rgb(37 99 235);
		height: 0.5rem;
		border-radius: 0.5rem;
	}
</style>
