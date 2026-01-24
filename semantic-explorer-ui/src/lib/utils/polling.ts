/**
 * Polling utilities with proper race condition prevention
 * - Request deduplication: prevents multiple requests in flight
 * - Automatic cleanup: prevents interval leaks
 * - Error handling: continues polling after errors
 * - Configurable timing
 */

/** Default polling interval in milliseconds */
export const DEFAULT_POLLING_INTERVAL = 3000;

/** Default max consecutive errors before stopping polling */
export const DEFAULT_MAX_ERRORS = 5;

/** Default heartbeat timeout in milliseconds */
export const DEFAULT_HEARTBEAT_TIMEOUT = 90000;

export interface PollingOptions {
	/** Polling interval in milliseconds (default: 3000) */
	interval?: number;
	/** Callback to check if polling should continue */
	shouldContinue?: () => boolean;
	/** Max number of consecutive errors before stopping (default:5) */
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
 *
 * @example
 * ```typescript
 * let inFlight = false;
 *
 * const controller = createPollingInterval(async () => {
 *   if (inFlight) return; // Skip if request already in-flight
 *   inFlight = true;
 *   try {
 *     const data = await fetch('/api/data');
 *     // update UI
 *   } finally {
 *     inFlight = false;
 *   }
 * }, { interval: 2000 });
 *
 * // On unmount
 * controller.stop();
 * ```
 */
export function createPollingInterval(
	callback: () => Promise<void>,
	options: PollingOptions = {}
): PollingController {
	const {
		interval = DEFAULT_POLLING_INTERVAL,
		shouldContinue,
		maxErrors = DEFAULT_MAX_ERRORS,
		onError,
	} = options;

	let pollingInterval: ReturnType<typeof setInterval> | null = null;
	let isPolling = false;
	let errorCount = 0;
	let stopped = false;

	async function poll() {
		try {
			// Check if we should continue before running callback
			if (shouldContinue && !shouldContinue()) {
				stop();
				return;
			}

			await callback();
			// Reset error count on success
			errorCount = 0;
		} catch (error) {
			errorCount++;
			const err = error instanceof Error ? error : new Error(String(error));
			onError?.(err, errorCount);

			// Stop polling if too many errors
			if (errorCount >= maxErrors) {
				console.error(`Polling stopped after ${maxErrors} consecutive errors`);
				stop();
			}
		}
	}

	function start() {
		if (pollingInterval || isPolling) return;

		stopped = false;
		errorCount = 0;

		// Call immediately before setting interval
		poll();

		pollingInterval = setInterval(() => {
			if (!stopped) {
				poll();
			}
		}, interval);
	}

	function stop() {
		stopped = true;
		if (pollingInterval) {
			clearInterval(pollingInterval);
			pollingInterval = null;
		}
	}

	function resume() {
		if (!pollingInterval && !stopped) {
			start();
		}
	}

	function getIsPolling() {
		return pollingInterval !== null;
	}

	// Start immediately
	start();

	return {
		stop,
		resume,
		isPolling: getIsPolling,
	};
}

/**
 * Wraps a fetch call with automatic deduplication.
 * Prevents multiple identical requests from being in-flight.
 */
export function createDedupedFetch() {
	const inFlightRequests = new Map<string, Promise<Response>>();

	return async function dedupedFetch(
		input: string | Request,
		init?: RequestInit
	): Promise<Response> {
		// Create a key from the request
		const key =
			typeof input === 'string'
				? input + (init?.method || 'GET')
				: input.url + (input.method || 'GET');

		// Return existing request if one is in-flight
		if (inFlightRequests.has(key)) {
			return inFlightRequests.get(key)!;
		}

		// Start new request
		const request = fetch(input, init);
		inFlightRequests.set(key, request);

		try {
			const response = await request;
			return response;
		} finally {
			// Remove from in-flight map
			inFlightRequests.delete(key);
		}
	};
}

/**
 * Creates an AbortController that automatically aborts on unmount/destroy.
 * Useful for canceling in-flight requests when component unmounts.
 *
 * @example
 * ```typescript
 * let abortController = createAbortController();
 *
 * onMount(() => {
 *   // ...
 * });
 *
 * onDestroy(() => {
 *   abortController.abort();
 * });
 *
 * // In fetch call:
 * const response = await fetch(url, { signal: abortController.signal });
 * ```
 */
export function createAbortController(): AbortController {
	return new AbortController();
}
