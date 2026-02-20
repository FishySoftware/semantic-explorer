import type { SearchResponse } from '$lib/types/models';
import { apiPost } from '$lib/utils/api';

export interface SearchRequest {
	query: string;
	embedded_dataset_ids: number[];
	limit?: number;
	search_mode?: 'documents' | 'chunks';
	min_score?: number;
}

export function search(body: SearchRequest, signal?: AbortSignal): Promise<SearchResponse> {
	return apiPost<SearchResponse>('/api/search', body, signal);
}
