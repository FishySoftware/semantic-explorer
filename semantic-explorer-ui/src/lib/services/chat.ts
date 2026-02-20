import type { ChatMessage, ChatSession } from '$lib/types/models';
import { apiDelete, apiGet, apiPost, apiPostRaw, buildQueryString } from '$lib/utils/api';

export interface ChatSessionsResponse {
	sessions: ChatSession[];
	total_count: number;
	limit: number;
	offset: number;
}

export interface ChatMessagesResponse {
	messages: ChatMessage[];
}

export function getChatSessions(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<ChatSessionsResponse> {
	const query = buildQueryString(params ?? {});
	return apiGet<ChatSessionsResponse>(`/api/chat/sessions${query}`, signal);
}

export function getChatSession(sessionId: string, signal?: AbortSignal): Promise<ChatSession> {
	return apiGet<ChatSession>(`/api/chat/sessions/${sessionId}`, signal);
}

export function createChatSession(
	body: { embedded_dataset_id: number; llm_id: number; title: string },
	signal?: AbortSignal
): Promise<ChatSession> {
	return apiPost<ChatSession>('/api/chat/sessions', body, signal);
}

export function deleteChatSession(sessionId: string, signal?: AbortSignal): Promise<void> {
	return apiDelete(`/api/chat/sessions/${sessionId}`, signal);
}

export function getChatMessages(
	sessionId: string,
	signal?: AbortSignal
): Promise<ChatMessagesResponse> {
	return apiGet<ChatMessagesResponse>(`/api/chat/sessions/${sessionId}/messages`, signal);
}

export interface StreamMessageBody {
	content: string;
	max_context_documents: number;
	min_similarity_score: number;
	temperature: number;
	max_tokens: number;
	system_prompt: string | null;
}

export function streamChatMessage(
	sessionId: string,
	body: StreamMessageBody,
	signal?: AbortSignal
): Promise<Response> {
	return apiPostRaw(`/api/chat/sessions/${sessionId}/messages/stream`, body, signal);
}

export function regenerateChatMessage(
	messageId: number,
	signal?: AbortSignal
): Promise<{
	content: string;
	retrieved_documents?: Array<{
		document_id: string | null;
		text: string;
		similarity_score: number;
		item_title: string | null;
	}>;
}> {
	return apiPost<{
		content: string;
		retrieved_documents?: Array<{
			document_id: string | null;
			text: string;
			similarity_score: number;
			item_title: string | null;
		}>;
	}>(`/api/chat/messages/${messageId}/regenerate?stream=false`, undefined, signal);
}
