import type {
	Collection,
	CollectionTransform,
	CollectionTransformStats,
	PaginatedCollectionList,
	PaginatedFiles,
	ProcessedFile,
} from '$lib/types/models';
import {
	apiDelete,
	apiGet,
	apiPatch,
	apiPost,
	apiPostFormData,
	buildQueryString,
} from '$lib/utils/api';

export function getCollections(
	params?: { limit?: number; offset?: number; search?: string },
	signal?: AbortSignal
): Promise<PaginatedCollectionList> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedCollectionList>(`/api/collections${query}`, signal);
}

export function getCollection(collectionId: number, signal?: AbortSignal): Promise<Collection> {
	return apiGet<Collection>(`/api/collections/${collectionId}`, signal);
}

export function searchCollections(
	params: { query: string; limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<PaginatedCollectionList> {
	const query = buildQueryString(params);
	return apiGet<PaginatedCollectionList>(`/api/collections/search${query}`, signal);
}

export function createCollection(
	body: { title: string; details?: string; tags?: string[] },
	signal?: AbortSignal
): Promise<Collection> {
	return apiPost<Collection>('/api/collections', body, signal);
}

export function updateCollection(
	collectionId: number,
	body: Partial<{ title: string; details: string; tags: string[]; is_public: boolean }>,
	signal?: AbortSignal
): Promise<Collection> {
	return apiPatch<Collection>(`/api/collections/${collectionId}`, body, signal);
}

export function deleteCollection(collectionId: number, signal?: AbortSignal): Promise<void> {
	return apiDelete(`/api/collections/${collectionId}`, signal);
}

export function getCollectionFiles(
	collectionId: number,
	params?: { page?: number; page_size?: number; continuation_token?: string },
	signal?: AbortSignal
): Promise<PaginatedFiles> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedFiles>(`/api/collections/${collectionId}/files${query}`, signal);
}

export function uploadCollectionFiles(
	collectionId: number,
	formData: FormData,
	signal?: AbortSignal
): Promise<{ uploaded_files: string[] }> {
	return apiPostFormData<{ uploaded_files: string[] }>(
		`/api/collections/${collectionId}/files`,
		formData,
		signal
	);
}

export function deleteCollectionFile(
	collectionId: number,
	fileKey: string,
	signal?: AbortSignal
): Promise<void> {
	return apiDelete(`/api/collections/${collectionId}/files/${encodeURIComponent(fileKey)}`, signal);
}

export function getCollectionTransforms(
	collectionId: number,
	signal?: AbortSignal
): Promise<CollectionTransform[]> {
	return apiGet<CollectionTransform[]>(`/api/collections/${collectionId}/transforms`, signal);
}

export function getCollectionFailedFiles(
	collectionId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<ProcessedFile[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<ProcessedFile[]>(`/api/collections/${collectionId}/failed-files${query}`, signal);
}

export function getAllowedFileTypes(signal?: AbortSignal): Promise<string[]> {
	return apiGet<string[]>('/api/collections-allowed-file-types', signal);
}

export function getCollectionFileDownloadUrl(collectionId: number, fileKey: string): string {
	return `/api/collections/${collectionId}/files/${encodeURIComponent(fileKey)}`;
}

export function getCollectionTransformById(
	transformId: number,
	signal?: AbortSignal
): Promise<CollectionTransform> {
	return apiGet<CollectionTransform>(`/api/collection-transforms/${transformId}`, signal);
}

export function getCollectionTransformStats(
	transformId: number,
	signal?: AbortSignal
): Promise<CollectionTransformStats> {
	return apiGet<CollectionTransformStats>(
		`/api/collection-transforms/${transformId}/stats`,
		signal
	);
}

export function getCollectionTransformProcessedFiles(
	transformId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<ProcessedFile[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<ProcessedFile[]>(
		`/api/collection-transforms/${transformId}/processed-files${query}`,
		signal
	);
}

export function createCollectionTransform(
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<CollectionTransform> {
	return apiPost<CollectionTransform>('/api/collection-transforms', body, signal);
}

export function updateCollectionTransform(
	transformId: number,
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<CollectionTransform> {
	return apiPatch<CollectionTransform>(`/api/collection-transforms/${transformId}`, body, signal);
}

export function deleteCollectionTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<void> {
	return apiDelete(`/api/collection-transforms/${transformId}`, signal);
}

export function triggerCollectionTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<unknown> {
	return apiPost<unknown>(`/api/collection-transforms/${transformId}/trigger`, undefined, signal);
}

export function retryFailedCollectionTransformFiles(
	transformId: number,
	signal?: AbortSignal
): Promise<unknown> {
	return apiPost<unknown>(
		`/api/collection-transforms/${transformId}/retry-failed`,
		undefined,
		signal
	);
}
