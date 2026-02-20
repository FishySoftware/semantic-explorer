import type { ChatMessage } from '$lib/types/models';
import { SvelteDate } from 'svelte/reactivity';

export interface StreamingProgress {
	messageId: number;
	charCount: number;
	elapsedSeconds: number;
}

export interface StreamingState {
	status: 'connecting' | 'retrieving' | 'generating' | null;
	progress: StreamingProgress | null;
}

export interface StreamingCallbacks {
	onConnected?: () => void;
	onRetrievalComplete?: (messageId: number, documents: RetrievedDocument[]) => void;
	onContent?: (messageId: number, content: string) => void;
	onProgress?: (progress: StreamingProgress) => void;
	onComplete?: (messageId: number, content: string, documents: RetrievedDocument[]) => void;
	onError?: (error: string) => void;
}

export interface RetrievedDocument {
	document_id: string | null;
	text: string;
	similarity_score: number;
	item_title: string | null;
}

export interface ChatStreamSettings {
	maxChunks: number;
	minSimilarityScore: number;
	temperature: number;
	maxTokens: number;
	systemPrompt: string;
}

export interface UseChatStreamOptions {
	sessionId: string;
	getSettings: () => ChatStreamSettings;
	callbacks: StreamingCallbacks;
}

export interface UseChatStreamResult {
	messages: ChatMessage[];
	isGenerating: boolean;
	streamingState: StreamingState;
	sendMessage: (content: string) => Promise<void>;
	regenerateMessage: (messageId: number) => Promise<void>;
	setMessages: (msgs: ChatMessage[]) => void;
	cleanup: () => void;
}

let tempIdCounter = -1;

function nextTempId(): number {
	return tempIdCounter--;
}

export function useChatStream(options: UseChatStreamOptions): UseChatStreamResult {
	const { sessionId, getSettings, callbacks } = options;

	let messages = $state<ChatMessage[]>([]);
	let isGenerating = $state(false);
	let streamingState = $state<StreamingState>({
		status: null,
		progress: null,
	});

	let abortController: AbortController | null = null;
	let accumulatedContent = '';
	let actualMessageId: number | null = null;
	let assistantPlaceholderId = 0;
	let retrievedDocs: RetrievedDocument[] = [];
	let buffer = '';

	function resetStreamState() {
		streamingState.status = null;
		streamingState.progress = null;
		isGenerating = false;
	}

	function cleanup() {
		if (abortController) {
			abortController.abort();
			abortController = null;
		}
		resetStreamState();
	}

	function updateMessage(targetId: number, updater: (msg: ChatMessage) => ChatMessage) {
		messages = messages.map((msg) => (msg.message_id === targetId ? updater(msg) : msg));
	}

	function processSSEData(dataStr: string): void {
		if (!dataStr) return;

		const data = JSON.parse(dataStr);
		const eventType = streamingState.status || data.type;

		switch (eventType) {
			case 'connected':
				streamingState.status = 'retrieving';
				callbacks.onConnected?.();
				break;

			case 'retrieval_complete':
				actualMessageId = data.message_id;
				retrievedDocs = data.documents || [];
				if (data.text) {
					accumulatedContent += data.text;
				}
				callbacks.onRetrievalComplete?.(data.message_id, retrievedDocs);
				streamingState.status = 'generating';
				streamingState.progress = {
					messageId: actualMessageId!,
					charCount: 0,
					elapsedSeconds: 0,
				};
				break;

			case 'content': {
				const chunk = data.content || data.text || '';
				accumulatedContent += chunk;
				const resolvedId = actualMessageId || assistantPlaceholderId;
				updateMessage(resolvedId, (msg) => ({ ...msg, content: accumulatedContent }));
				callbacks.onContent?.(resolvedId, accumulatedContent);
				break;
			}

			case 'progress':
				streamingState.progress = {
					messageId: data.message_id,
					charCount: data.char_count,
					elapsedSeconds: data.elapsed_seconds,
				};
				callbacks.onProgress?.(streamingState.progress);
				break;

			case 'complete': {
				const finalContent = data.content || accumulatedContent;
				updateMessage(data.message_id, (msg) => ({
					...msg,
					status: 'complete',
					content: finalContent,
				}));
				callbacks.onComplete?.(data.message_id, finalContent, retrievedDocs);
				resetStreamState();
				break;
			}

			case 'error': {
				const error = data.error || 'Streaming error occurred';
				callbacks.onError?.(error);
				if (actualMessageId) {
					updateMessage(actualMessageId, (msg) => ({ ...msg, status: 'error' }));
				}
				resetStreamState();
				break;
			}
		}
	}

	function processBufferedLines(lines: string[]): void {
		for (const line of lines) {
			if (line.startsWith('event:')) {
				continue;
			}
			if (!line.startsWith('data:')) continue;

			const dataStr = line.substring(5).trim();
			if (!dataStr) continue;

			try {
				processSSEData(dataStr);
			} catch (parseError) {
				console.error('Error parsing SSE data:', parseError, 'Line:', line);
			}
		}
	}

	async function sendMessage(content: string): Promise<void> {
		if (!content.trim() || isGenerating) return;

		cleanup();
		abortController = new AbortController();

		isGenerating = true;
		streamingState.status = 'connecting';
		accumulatedContent = '';
		actualMessageId = null;
		assistantPlaceholderId = nextTempId();
		retrievedDocs = [];
		buffer = '';

		const userTempId = nextTempId();
		const now = new SvelteDate().toISOString();

		messages = [
			...messages,
			{
				message_id: userTempId,
				role: 'user',
				content,
				created_at: now,
				tokens_used: null,
				metadata: null,
				documents_retrieved: null,
				status: 'complete',
			},
			{
				message_id: assistantPlaceholderId,
				role: 'assistant',
				content: '',
				created_at: now,
				tokens_used: null,
				metadata: null,
				documents_retrieved: 0,
				status: 'incomplete',
				retrieved_documents: [],
			},
		];

		try {
			const { maxChunks, minSimilarityScore, temperature, maxTokens, systemPrompt } = getSettings();

			const response = await fetch(`/api/chat/sessions/${sessionId}/messages/stream`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				signal: abortController.signal,
				body: JSON.stringify({
					content,
					max_context_documents: maxChunks,
					min_similarity_score: minSimilarityScore,
					temperature,
					max_tokens: maxTokens,
					system_prompt: systemPrompt || null,
				}),
			});

			if (!response.ok) {
				throw new Error(`HTTP error! status: ${response.status}`);
			}

			const reader = response.body?.getReader();
			if (!reader) {
				throw new Error('Response body is not readable');
			}

			const decoder = new TextDecoder();

			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				buffer += decoder.decode(value, { stream: true });
				const lines = buffer.split('\n');
				buffer = lines.pop() || '';
				processBufferedLines(lines);
			}

			if (buffer.trim()) {
				const remainingLines = buffer.split('\n');
				processBufferedLines(remainingLines);
			}

			if (isGenerating) {
				if (actualMessageId) {
					updateMessage(actualMessageId, (msg) => ({
						...msg,
						status: 'complete',
						content: accumulatedContent,
					}));
				}
				resetStreamState();
			}
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') {
				return;
			}
			const errorMessage = error instanceof Error ? error.message : 'An error occurred';
			callbacks.onError?.(errorMessage);
			resetStreamState();
		} finally {
			abortController = null;
		}
	}

	async function regenerateMessage(messageId: number): Promise<void> {
		if (isGenerating) return;

		cleanup();
		abortController = new AbortController();
		isGenerating = true;

		updateMessage(messageId, (msg) => ({ ...msg, status: 'incomplete', content: '' }));

		try {
			const response = await fetch(`/api/chat/messages/${messageId}/regenerate?stream=false`, {
				method: 'POST',
				signal: abortController.signal,
			});

			if (!response.ok) {
				throw new Error(`Failed to regenerate message: ${response.statusText}`);
			}

			const result = await response.json();

			updateMessage(messageId, (msg) => ({
				...msg,
				content: result.content,
				status: 'complete',
				retrieved_documents: result.retrieved_documents || [],
			}));

			callbacks.onComplete?.(messageId, result.content, result.retrieved_documents || []);
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') {
				return;
			}
			const errorMessage = error instanceof Error ? error.message : 'An error occurred';
			callbacks.onError?.(errorMessage);
			updateMessage(messageId, (msg) => ({ ...msg, status: 'error' }));
		} finally {
			isGenerating = false;
			abortController = null;
		}
	}

	function setMessages(msgs: ChatMessage[]) {
		messages = msgs;
	}

	return {
		get messages() {
			return messages;
		},
		get isGenerating() {
			return isGenerating;
		},
		streamingState,
		sendMessage,
		regenerateMessage,
		setMessages,
		cleanup,
	};
}
