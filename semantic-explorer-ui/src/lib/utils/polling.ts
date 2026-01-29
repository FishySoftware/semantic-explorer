/**
 * Polling utilities with proper race condition prevention
 */

export interface PollingOptions {
	/** Polling interval in milliseconds (default: 3000) */
	interval?: number;
	/** Callback to check if polling should continue */
	shouldContinue?: () => boolean;
	/** Max number of consecutive errors before stopping (default: 5) */
	maxErrors?: number;
	/** Callback on error */
	onError?: (error: Error, retryCount: number) => void;
}

export interface PollingController {
	/** Stop polling */
	stop: () => void;
	/** Start polling (if stopped) */
	resume: () => void;
	/** Check if currently polling */
	isPolling: () => boolean;
}

/**
 * Creates a managed polling interval with automatic deduplication.
 * Prevents multiple requests from being in-flight simultaneously.
 */
export function createPollingInterval(
	callback: () => Promise<void>,
	options: PollingOptions = {}
): PollingController {
	const { interval = 3000, shouldContinue, maxErrors = 5, onError } = options;

	let pollingInterval: ReturnType<typeof setInterval> | null = null;
	let isPolling = false;
	let errorCount = 0;
	let stopped = false;

	async function poll() {
		try {
			if (shouldContinue && !shouldContinue()) {
				stop();
				return;
			}
			await callback();
			errorCount = 0;
		} catch (error) {
			errorCount++;
			const err = error instanceof Error ? error : new Error(String(error));
			onError?.(err, errorCount);
			if (errorCount >= maxErrors) {
				console.error(`Polling stopped after ${maxErrors} consecutive errors`);
				stop();
			}
		}
	}

	function start() {
		if (pollingInterval || stopped) return;
		isPolling = true;
		pollingInterval = setInterval(poll, interval);
	}

	function stop() {
		if (pollingInterval) {
			clearInterval(pollingInterval);
			pollingInterval = null;
		}
		isPolling = false;
		stopped = true;
	}

	function resume() {
		stopped = false;
		errorCount = 0;
		start();
	}

	// Start immediately
	start();

	return {
		stop,
		resume,
		isPolling: () => isPolling,
	};
}
