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
	onRetrievalComplete?: (_messageId: number, _documents: RetrievedDocument[]) => void;
	onContent?: (_messageId: number, _content: string) => void;
	onProgress?: (_progress: StreamingProgress) => void;
	onComplete?: (_messageId: number, _content: string, _documents: RetrievedDocument[]) => void;
	onError?: (_error: string) => void;
}

export interface RetrievedDocument {
	document_id: string | null;
	text: string;
	similarity_score: number;
	item_title: string | null;
}

export interface UseChatStreamOptions {
	sessionId: string;
	getSettings: () => {
		maxChunks: number;
		minSimilarityScore: number;
		temperature: number;
		maxTokens: number;
		systemPrompt: string;
	};
	callbacks: StreamingCallbacks;
}

export interface UseChatStreamResult {
	messages: ChatMessage[];
	isGenerating: boolean;
	streamingState: StreamingState;
	sendMessage: (_content: string) => Promise<void>;
	regenerateMessage: (_messageId: number) => Promise<void>;
	cleanup: () => void;
}

export function useChatStream(options: UseChatStreamOptions): UseChatStreamResult {
	const { sessionId, getSettings, callbacks } = options;

	// State
	let messages = $state<ChatMessage[]>([]);
	let isGenerating = $state(false);
	let streamingState = $state<StreamingState>({
		status: null,
		progress: null,
	});
	let eventSource: EventSource | null = null;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let accumulatedContent = '';
	let actualMessageId: number | null = null;
	let assistantPlaceholderId: number | 0;
	let retrievedDocs: RetrievedDocument[] = [];
	let buffer = '';

	// Cleanup function
	function cleanup() {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}
	}

	// Process SSE data
	function processSSEData(data_str: string): void {
		if (!data_str) return;

		const data = JSON.parse(data_str);
		const eventType = streamingState.status || data.type;

		switch (eventType) {
			case 'connected':
				streamingState.status = 'retrieving';
				if (callbacks.onConnected) callbacks.onConnected();
				break;

			case 'retrieval_complete':
				actualMessageId = data.message_id;
				retrievedDocs = data.documents || [];
				if (data.text) {
					accumulatedContent += data.text;
				}
				if (callbacks.onRetrievalComplete) {
					callbacks.onRetrievalComplete(data.message_id, retrievedDocs);
				}
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
				messages = messages.map((msg: ChatMessage) =>
					msg.message_id === (actualMessageId || assistantPlaceholderId)
						? { ...msg, content: accumulatedContent }
						: msg
				);
				if (callbacks.onContent) {
					callbacks.onContent(actualMessageId || assistantPlaceholderId, accumulatedContent);
				}
				break;
			}

			case 'progress':
				streamingState.progress = {
					messageId: data.message_id,
					charCount: data.char_count,
					elapsedSeconds: data.elapsed_seconds,
				};
				if (callbacks.onProgress) {
					callbacks.onProgress(streamingState.progress);
				}
				break;

			case 'complete': {
				const finalContent = data.content || accumulatedContent;
				messages = messages.map((msg: ChatMessage) =>
					msg.message_id === data.message_id
						? { ...msg, status: 'complete', content: finalContent }
						: msg
				);
				if (callbacks.onComplete) {
					callbacks.onComplete(data.message_id, finalContent, retrievedDocs);
				}
				streamingState.status = null;
				streamingState.progress = null;
				isGenerating = false;
				break;
			}

			case 'error': {
				const error = data.error || 'Streaming error occurred';
				if (callbacks.onError) {
					callbacks.onError(error);
				}
				if (actualMessageId) {
					messages = messages.map((msg: ChatMessage) =>
						msg.message_id === actualMessageId ? { ...msg, status: 'error' } : msg
					);
				}
				streamingState.status = null;
				streamingState.progress = null;
				isGenerating = false;
				break;
			}
		}
	}

	// Start streaming
	async function sendMessage(content: string): Promise<void> {
		if (!content.trim() || isGenerating) return;

		isGenerating = true;
		streamingState.status = 'connecting';
		accumulatedContent = '';
		actualMessageId = Date.now();
		assistantPlaceholderId = Date.now() + 1;
		retrievedDocs = [];

		// Add user message optimistically
		messages = [
			...messages,
			{
				message_id: actualMessageId,
				role: 'user',
				content,
				created_at: new SvelteDate().toISOString(),
				tokens_used: null,
				metadata: null,
				documents_retrieved: null,
				status: 'complete',
			},
		];

		// Add placeholder assistant message
		messages = [
			...messages,
			{
				message_id: assistantPlaceholderId,
				role: 'assistant',
				content: '',
				created_at: new SvelteDate().toISOString(),
				tokens_used: null,
				metadata: null,
				documents_retrieved: 0,
				status: 'incomplete',
				retrieved_documents: [],
			},
		];

		try {
			// Get current settings at the time of sending
			const { maxChunks, minSimilarityScore, temperature, maxTokens, systemPrompt } = getSettings();

			const response = await fetch(`/api/chat/sessions/${sessionId}/messages/stream`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
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
			let currentEventType = '';

			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				buffer += decoder.decode(value, { stream: true });
				const lines = buffer.split('\n');

				// Keep the last incomplete line in buffer
				buffer = lines.pop() || '';

				for (const line of lines) {
					if (line.startsWith('event:')) {
						currentEventType = line.substring(6).trim();
						continue;
					}

					if (!line.startsWith('data:')) continue;

					const data_str = line.substring(5).trim();
					if (!data_str) continue;

					try {
						currentEventType = '';
						processSSEData(data_str);
					} catch (e) {
						console.error('Error parsing SSE data:', e, 'Line:', line);
					}
				}
			}

			// Process any remaining data in buffer after stream ends
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
								if (currentEventType === 'content') {
									const chunk = data.content || data.text || '';
									accumulatedContent += chunk;
									messages = messages.map((msg: ChatMessage) =>
										msg.message_id === (actualMessageId || assistantPlaceholderId)
											? { ...msg, content: accumulatedContent }
											: msg
									);
								} else if (currentEventType === 'complete') {
									messages = messages.map((msg: ChatMessage) =>
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
				if (actualMessageId) {
					messages = messages.map((msg: ChatMessage) =>
						msg.message_id === actualMessageId
							? { ...msg, status: 'complete', content: accumulatedContent }
							: msg
					);
				}
				isGenerating = false;
				streamingState.status = null;
				streamingState.progress = null;
			}
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'An error occurred';
			if (callbacks.onError) {
				callbacks.onError(errorMessage);
			}
			isGenerating = false;
			streamingState.status = null;
			streamingState.progress = null;
		}
	}

	// Regenerate message
	async function regenerateMessage(messageId: number): Promise<void> {
		if (isGenerating) return;

		isGenerating = true;

		// Update message status to incomplete
		messages = messages.map((msg: ChatMessage) =>
			msg.message_id === messageId ? { ...msg, status: 'incomplete', content: '' } : msg
		);

		try {
			const response = await fetch(`/api/chat/messages/${messageId}/regenerate?stream=false`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to regenerate message: ${response.statusText}`);
			}

			const result = await response.json();

			// Update message with new content
			messages = messages.map((msg: ChatMessage) =>
				msg.message_id === messageId
					? {
							...msg,
							content: result.content,
							status: 'complete',
							retrieved_documents: result.retrieved_documents || [],
						}
					: msg
			);

			if (callbacks.onComplete) {
				callbacks.onComplete(messageId, result.content, result.retrieved_documents || []);
			}
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'An error occurred';
			if (callbacks.onError) {
				callbacks.onError(errorMessage);
			}

			// Update message status to error
			messages = messages.map((msg: ChatMessage) =>
				msg.message_id === messageId ? { ...msg, status: 'error' } : msg
			);
		} finally {
			isGenerating = false;
		}
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
		cleanup,
	};
}
