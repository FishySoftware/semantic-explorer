import type { Embedder, PaginatedResponse } from '$lib/types/models';
import { apiDelete, apiGet, apiPatch, apiPost, buildQueryString } from '$lib/utils/api';

export function getEmbedders(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedResponse<Embedder>> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedResponse<Embedder>>(`/api/embedders${query}`, signal);
}

export function getEmbedder(embedderId: number, signal?: AbortSignal): Promise<Embedder> {
	return apiGet<Embedder>(`/api/embedders/${embedderId}`, signal);
}

export function createEmbedder(
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<Embedder> {
	return apiPost<Embedder>('/api/embedders', body, signal);
}

export function updateEmbedder(
	embedderId: number,
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<Embedder> {
	return apiPatch<Embedder>(`/api/embedders/${embedderId}`, body, signal);
}

export function deleteEmbedder(embedderId: number, signal?: AbortSignal): Promise<void> {
	return apiDelete(`/api/embedders/${embedderId}`, signal);
}

export function testEmbedder(
	embedderId: number,
	signal?: AbortSignal
): Promise<{ success: boolean; message: string; model?: string; dimensions?: number }> {
	return apiPost<{ success: boolean; message: string; model?: string; dimensions?: number }>(
		`/api/embedders/${embedderId}/test`,
		undefined,
		signal
	);
}

export function getEmbeddingInferenceModels(
	signal?: AbortSignal
): Promise<Array<{ id: string; name: string }>> {
	return apiGet<Array<{ id: string; name: string }>>('/api/embedding-inference/models', signal);
}
