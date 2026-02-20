export class ApiError extends Error {
	public readonly status: number;
	public readonly details?: string;

	constructor(status: number, message: string, details?: string) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.details = details;
	}
}

async function parseErrorResponse(response: Response): Promise<ApiError> {
	const errorText = await response.text();
	let errorMessage = `Request failed with status ${response.status}`;

	try {
		const errorJson = JSON.parse(errorText);
		if (errorJson.error) {
			errorMessage = errorJson.error;
		}
	} catch {
		if (errorText) {
			errorMessage = errorText;
		}
	}

	return new ApiError(response.status, errorMessage, errorText);
}

export async function apiGet<T>(url: string, signal?: AbortSignal): Promise<T> {
	const response = await fetch(url, { signal });
	if (!response.ok) {
		throw await parseErrorResponse(response);
	}
	return response.json();
}

export async function apiPost<T>(url: string, body?: unknown, signal?: AbortSignal): Promise<T> {
	const options: RequestInit = {
		method: 'POST',
		signal,
	};
	if (body !== undefined) {
		options.headers = { 'Content-Type': 'application/json' };
		options.body = JSON.stringify(body);
	}
	const response = await fetch(url, options);
	if (!response.ok) {
		throw await parseErrorResponse(response);
	}
	return response.json();
}

export async function apiPatch<T>(url: string, body: unknown, signal?: AbortSignal): Promise<T> {
	const response = await fetch(url, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
		signal,
	});
	if (!response.ok) {
		throw await parseErrorResponse(response);
	}
	return response.json();
}

export async function apiDelete(url: string, signal?: AbortSignal): Promise<void> {
	const response = await fetch(url, { method: 'DELETE', signal });
	if (!response.ok) {
		throw await parseErrorResponse(response);
	}
}

export async function apiPostRaw(
	url: string,
	body: unknown,
	signal?: AbortSignal
): Promise<Response> {
	const response = await fetch(url, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
		signal,
	});
	if (!response.ok) {
		throw await parseErrorResponse(response);
	}
	return response;
}

export async function apiPostFormData<T>(
	url: string,
	formData: FormData,
	signal?: AbortSignal
): Promise<T> {
	const response = await fetch(url, {
		method: 'POST',
		body: formData,
		signal,
	});
	if (!response.ok) {
		throw await parseErrorResponse(response);
	}
	return response.json();
}

export function buildQueryString(
	params: Record<string, string | number | boolean | undefined | null>
): string {
	const searchParams = new URLSearchParams();
	for (const [key, value] of Object.entries(params)) {
		if (value !== undefined && value !== null && value !== '') {
			searchParams.append(key, String(value));
		}
	}
	const queryString = searchParams.toString();
	return queryString ? `?${queryString}` : '';
}
