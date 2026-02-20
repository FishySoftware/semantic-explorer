import type {
	Dataset,
	DatasetItemChunks,
	DatasetItemSummary,
	DatasetTransform,
	DatasetTransformBatch,
	DatasetTransformStats,
	DetailedStatsResponse,
	PaginatedItems,
	PaginatedResponse,
} from '$lib/types/models';
import { apiDelete, apiGet, apiPatch, apiPost, buildQueryString } from '$lib/utils/api';

export function getDatasets(
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedResponse<Dataset>> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedResponse<Dataset>>(`/api/datasets${query}`, signal);
}

export function getDataset(datasetId: number, signal?: AbortSignal): Promise<Dataset> {
	return apiGet<Dataset>(`/api/datasets/${datasetId}`, signal);
}

export function createDataset(
	body: { title: string; details?: string; tags?: string[] },
	signal?: AbortSignal
): Promise<Dataset> {
	return apiPost<Dataset>('/api/datasets', body, signal);
}

export function updateDataset(
	datasetId: number,
	body: Partial<{ title: string; details: string; tags: string[]; is_public: boolean }>,
	signal?: AbortSignal
): Promise<Dataset> {
	return apiPatch<Dataset>(`/api/datasets/${datasetId}`, body, signal);
}

export function deleteDataset(datasetId: number, signal?: AbortSignal): Promise<void> {
	return apiDelete(`/api/datasets/${datasetId}`, signal);
}

export function getDatasetItems(
	datasetId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedItems> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedItems>(`/api/datasets/${datasetId}/items-summary${query}`, signal);
}

export function getDatasetItemChunks(
	datasetId: number,
	itemId: number,
	signal?: AbortSignal
): Promise<DatasetItemChunks> {
	return apiGet<DatasetItemChunks>(`/api/datasets/${datasetId}/items/${itemId}/chunks`, signal);
}

export function deleteDatasetItem(
	datasetId: number,
	itemId: number,
	signal?: AbortSignal
): Promise<void> {
	return apiDelete(`/api/datasets/${datasetId}/items/${itemId}`, signal);
}

export function createDatasetItems(
	datasetId: number,
	body: { items: Array<{ title: string; content: string }> },
	signal?: AbortSignal
): Promise<{ created_count: number }> {
	return apiPost<{ created_count: number }>(`/api/datasets/${datasetId}/items`, body, signal);
}

export function getDatasetTransforms(
	params?: { limit?: number; offset?: number; dataset_id?: number },
	signal?: AbortSignal
): Promise<PaginatedResponse<DatasetTransform>> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedResponse<DatasetTransform>>(`/api/dataset-transforms${query}`, signal);
}

export function getDatasetTransformsByDataset(
	datasetId: number,
	signal?: AbortSignal
): Promise<DatasetTransform[]> {
	return apiGet<DatasetTransform[]>(`/api/datasets/${datasetId}/transforms`, signal);
}

export function getDatasetTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<DatasetTransform> {
	return apiGet<DatasetTransform>(`/api/dataset-transforms/${transformId}`, signal);
}

export function createDatasetTransform(
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<DatasetTransform> {
	return apiPost<DatasetTransform>('/api/dataset-transforms', body, signal);
}

export function updateDatasetTransform(
	transformId: number,
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<DatasetTransform> {
	return apiPatch<DatasetTransform>(`/api/dataset-transforms/${transformId}`, body, signal);
}

export function deleteDatasetTransform(transformId: number, signal?: AbortSignal): Promise<void> {
	return apiDelete(`/api/dataset-transforms/${transformId}`, signal);
}

export function triggerDatasetTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<unknown> {
	return apiPost<unknown>(`/api/dataset-transforms/${transformId}/trigger`, undefined, signal);
}

export function retryFailedDatasetTransformBatches(
	transformId: number,
	signal?: AbortSignal
): Promise<unknown> {
	return apiPost<unknown>(`/api/dataset-transforms/${transformId}/retry-failed`, undefined, signal);
}

export function retryDatasetTransformBatch(
	transformId: number,
	batchId: number,
	signal?: AbortSignal
): Promise<unknown> {
	return apiPost<unknown>(
		`/api/dataset-transforms/${transformId}/batches/${batchId}/retry`,
		undefined,
		signal
	);
}

export function getDatasetTransformStats(
	transformId: number,
	signal?: AbortSignal
): Promise<DatasetTransformStats> {
	return apiGet<DatasetTransformStats>(`/api/dataset-transforms/${transformId}/stats`, signal);
}

export function getDatasetTransformDetailedStats(
	transformId: number,
	signal?: AbortSignal
): Promise<DetailedStatsResponse> {
	return apiGet<DetailedStatsResponse>(
		`/api/dataset-transforms/${transformId}/detailed-stats`,
		signal
	);
}

export function getDatasetTransformBatches(
	transformId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedResponse<DatasetTransformBatch>> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedResponse<DatasetTransformBatch>>(
		`/api/dataset-transforms/${transformId}/batches${query}`,
		signal
	);
}

export function getDatasetItemsByTitle(
	datasetId: number,
	title: string,
	signal?: AbortSignal
): Promise<DatasetItemSummary[]> {
	const query = buildQueryString({ title });
	return apiGet<DatasetItemSummary[]>(`/api/datasets/${datasetId}/items-summary${query}`, signal);
}
