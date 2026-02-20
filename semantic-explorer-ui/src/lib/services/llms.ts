import type { LLM, PaginatedLLMList } from '$lib/types/models';
import { apiDelete, apiGet, apiPatch, apiPost, buildQueryString } from '$lib/utils/api';

export function getLLMs(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedLLMList> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedLLMList>(`/api/llms${query}`, signal);
}

export function getLLM(llmId: number, signal?: AbortSignal): Promise<LLM> {
	return apiGet<LLM>(`/api/llms/${llmId}`, signal);
}

export function createLLM(body: Record<string, unknown>, signal?: AbortSignal): Promise<LLM> {
	return apiPost<LLM>('/api/llms', body, signal);
}

export function updateLLM(
	llmId: number,
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<LLM> {
	return apiPatch<LLM>(`/api/llms/${llmId}`, body, signal);
}

export function deleteLLM(llmId: number, signal?: AbortSignal): Promise<void> {
	return apiDelete(`/api/llms/${llmId}`, signal);
}

export function getLLMInferenceModels(
	signal?: AbortSignal
): Promise<{ models: Array<{ id: string; name: string }> }> {
	return apiGet<{ models: Array<{ id: string; name: string }> }>(
		'/api/llm-inference/models',
		signal
	);
}
