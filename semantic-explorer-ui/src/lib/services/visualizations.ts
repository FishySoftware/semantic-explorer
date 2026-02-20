import type {
	PaginatedResponse,
	Visualization,
	VisualizationConfig,
	VisualizationTransform,
} from '$lib/types/models';
import { apiDelete, apiGet, apiPatch, apiPost, buildQueryString } from '$lib/utils/api';

export function getVisualizationTransforms(
	params?: { limit?: number; offset?: number; search?: string },
	signal?: AbortSignal
): Promise<PaginatedResponse<VisualizationTransform>> {
	const query = buildQueryString(params ?? {});
	return apiGet<PaginatedResponse<VisualizationTransform>>(
		`/api/visualization-transforms${query}`,
		signal
	);
}

export function getVisualizationTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<VisualizationTransform> {
	return apiGet<VisualizationTransform>(`/api/visualization-transforms/${transformId}`, signal);
}

export function createVisualizationTransform(
	body: {
		title: string;
		embedded_dataset_id: number;
		visualization_config: Partial<VisualizationConfig>;
	},
	signal?: AbortSignal
): Promise<VisualizationTransform> {
	return apiPost<VisualizationTransform>('/api/visualization-transforms', body, signal);
}

export function updateVisualizationTransform(
	transformId: number,
	body: Record<string, unknown>,
	signal?: AbortSignal
): Promise<VisualizationTransform> {
	return apiPatch<VisualizationTransform>(
		`/api/visualization-transforms/${transformId}`,
		body,
		signal
	);
}

export function deleteVisualizationTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<void> {
	return apiDelete(`/api/visualization-transforms/${transformId}`, signal);
}

export function triggerVisualizationTransform(
	transformId: number,
	signal?: AbortSignal
): Promise<unknown> {
	return apiPost<unknown>(
		`/api/visualization-transforms/${transformId}/trigger`,
		undefined,
		signal
	);
}

export function getVisualizationTransformStats(
	transformId: number,
	signal?: AbortSignal
): Promise<Record<string, unknown>> {
	return apiGet<Record<string, unknown>>(
		`/api/visualization-transforms/${transformId}/stats`,
		signal
	);
}

export function getVisualizationsByTransform(
	transformId: number,
	params?: { limit?: number; offset?: number },
	signal?: AbortSignal
): Promise<Visualization[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<Visualization[]>(
		`/api/visualization-transforms/${transformId}/visualizations${query}`,
		signal
	);
}

export function getVisualization(
	transformId: number,
	visualizationId: number,
	signal?: AbortSignal
): Promise<Visualization> {
	return apiGet<Visualization>(
		`/api/visualization-transforms/${transformId}/visualizations/${visualizationId}`,
		signal
	);
}

export function getVisualizationDownloadUrl(transformId: number, visualizationId: number): string {
	return `/api/visualization-transforms/${transformId}/visualizations/${visualizationId}/download`;
}

export function getRecentVisualizations(
	params?: { limit?: number },
	signal?: AbortSignal
): Promise<Visualization[]> {
	const query = buildQueryString(params ?? {});
	return apiGet<Visualization[]>(`/api/visualizations/recent${query}`, signal);
}
