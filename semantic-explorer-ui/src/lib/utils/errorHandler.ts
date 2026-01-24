import { toastStore } from './notifications';

/**
 * Sets up global error handlers for uncaught errors and unhandled promise rejections.
 * This should be called once during app initialization.
 */
export function setupGlobalErrorHandler(): void {
	// Handle uncaught errors
	window.addEventListener('error', (event) => {
		console.error('Global error:', event.error);
		const message = event.error?.message || 'An unexpected error occurred';
		toastStore.error(message);
	});

	// Handle unhandled promise rejections
	window.addEventListener('unhandledrejection', (event) => {
		console.error('Unhandled promise rejection:', event.reason);
		const message =
			event.reason instanceof Error
				? event.reason.message
				: typeof event.reason === 'string'
					? event.reason
					: 'An unexpected error occurred';
		toastStore.error(message);
		event.preventDefault();
	});
}
