import type {
	CreateStandaloneEmbeddedDatasetRequest,
	EmbeddedDataset,
	EmbeddedDatasetStats,
	PaginatedEmbeddedDatasetList,
	PaginatedResponse,
	ProcessedBatch,
	PushVectorsRequest,
	PushVectorsResponse,
	Visualization,
} from '$lib/types/models';
import { apiDelete, apiGet, apiPatch, apiPost, buildQueryString } from '$lib/utils/api';

export function getEmbeddedDatasets(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedEmbeddedDatasetList> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedEmbeddedDatasetList>(`/api/embedded-datasets${query}`, signal);
}

export function getEmbeddedDataset(
	embeddedDatasetId: number,
	signal?: AbortSignal
): Promise<EmbeddedDataset> {
	return apiGet<EmbeddedDataset>(`/api/embedded-datasets/${embeddedDatasetId}`, signal);
}

export function updateEmbeddedDataset(
	embeddedDatasetId: number,
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<EmbeddedDataset> {
	return apiPatch<EmbeddedDataset>(`/api/embedded-datasets/${embeddedDatasetId}`, body, signal);
}

export function deleteEmbeddedDataset(
	embeddedDatasetId: number,
	signal?: AbortSignal
): Promise<void> {
	return apiDelete(`/api/embedded-datasets/${embeddedDatasetId}`, signal);
}

export function getEmbeddedDatasetStats(
	embeddedDatasetId: number,
	signal?: AbortSignal
): Promise<EmbeddedDatasetStats> {
	return apiGet<EmbeddedDatasetStats>(`/api/embedded-datasets/${embeddedDatasetId}/stats`, signal);
}

export function getEmbeddedDatasetPoints(
	embeddedDatasetId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedResponse<Record<string, unknown>>> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedResponse<Record<string, unknown>>>(
		`/api/embedded-datasets/${embeddedDatasetId}/points${query}`,
		signal
	);
}

export function getEmbeddedDatasetProcessedBatches(
	embeddedDatasetId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<ProcessedBatch[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<ProcessedBatch[]>(
		`/api/embedded-datasets/${embeddedDatasetId}/processed-batches${query}`,
		signal
	);
}

export function getEmbeddedDatasetVisualizations(
	embeddedDatasetId: number,
	signal?: AbortSignal
): Promise<Visualization[]> {
	return apiGet<Visualization[]>(
		`/api/embedded-datasets/${embeddedDatasetId}/visualizations`,
		signal
	);
}

export function createStandaloneEmbeddedDataset(
	body: CreateStandaloneEmbeddedDatasetRequest,
	signal?: AbortSignal
): Promise<EmbeddedDataset> {
	return apiPost<EmbeddedDataset>('/api/embedded-datasets/standalone', body, signal);
}

export function pushVectors(
	embeddedDatasetId: number,
	body: PushVectorsRequest,
	signal?: AbortSignal
): Promise<PushVectorsResponse> {
	return apiPost<PushVectorsResponse>(
		`/api/embedded-datasets/${embeddedDatasetId}/push-vectors`,
		body,
		signal
	);
}

export function getEmbeddedDatasetsByDataset(
	datasetId: number,
	signal?: AbortSignal
): Promise<EmbeddedDataset[]> {
	return apiGet<EmbeddedDataset[]>(`/api/datasets/${datasetId}/embedded-datasets`, signal);
}
