import type {
	Collection,
	Dataset,
	Embedder,
	LLM,
	MarketplaceCollection,
	MarketplaceDataset,
	MarketplaceEmbedder,
	MarketplaceLLM,
} from '$lib/types/models';
import { apiGet, apiPost, buildQueryString } from '$lib/utils/api';

export function getPublicCollections(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<MarketplaceCollection[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceCollection[]>(`/api/marketplace/collections${query}`, signal);
}

export function getRecentPublicCollections(
	params?: { limit?: number },
	signal?: AbortSignal
): Promise<MarketplaceCollection[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceCollection[]>(`/api/marketplace/collections/recent${query}`, signal);
}

export function getPublicDatasets(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<MarketplaceDataset[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceDataset[]>(`/api/marketplace/datasets${query}`, signal);
}

export function getRecentPublicDatasets(
	params?: { limit?: number },
	signal?: AbortSignal
): Promise<MarketplaceDataset[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceDataset[]>(`/api/marketplace/datasets/recent${query}`, signal);
}

export function getPublicEmbedders(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<MarketplaceEmbedder[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceEmbedder[]>(`/api/marketplace/embedders${query}`, signal);
}

export function getRecentPublicEmbedders(
	params?: { limit?: number },
	signal?: AbortSignal
): Promise<MarketplaceEmbedder[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceEmbedder[]>(`/api/marketplace/embedders/recent${query}`, signal);
}

export function getPublicLLMs(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<MarketplaceLLM[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceLLM[]>(`/api/marketplace/llms${query}`, signal);
}

export function getRecentPublicLLMs(
	params?: { limit?: number },
	signal?: AbortSignal
): Promise<MarketplaceLLM[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<MarketplaceLLM[]>(`/api/marketplace/llms/recent${query}`, signal);
}

export function grabCollection(collectionId: number, signal?: AbortSignal): Promise<Collection> {
	return apiPost<Collection>(
		`/api/marketplace/collections/${collectionId}/grab`,
		undefined,
		signal
	);
}

export function grabDataset(datasetId: number, signal?: AbortSignal): Promise<Dataset> {
	return apiPost<Dataset>(`/api/marketplace/datasets/${datasetId}/grab`, undefined, signal);
}

export function grabEmbedder(embedderId: number, signal?: AbortSignal): Promise<Embedder> {
	return apiPost<Embedder>(`/api/marketplace/embedders/${embedderId}/grab`, undefined, signal);
}

export function grabLLM(llmId: number, signal?: AbortSignal): Promise<LLM> {
	return apiPost<LLM>(`/api/marketplace/llms/${llmId}/grab`, undefined, signal);
}
