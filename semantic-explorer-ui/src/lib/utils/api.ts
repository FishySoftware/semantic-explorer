export interface ErrorNotification {
	message: string;
	details?: string;
}

export async function handleApiResponse<T>(response: Response): Promise<T> {
	if (!response.ok) {
		const errorText = await response.text();
		console.error(`API Error [${response.status}]:`, errorText);

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

		throw new Error(errorMessage);
	}
	return response.json();
}

export async function apiCall<T>(url: string, options?: RequestInit): Promise<T> {
	try {
		const response = await fetch(url, options);
		return await handleApiResponse<T>(response);
	} catch (error) {
		console.error('API call failed:', error);
		throw error;
	}
}

export function showError(error: Error | unknown): string {
	if (error instanceof Error) {
		return error.message;
	}
	return String(error);
}
