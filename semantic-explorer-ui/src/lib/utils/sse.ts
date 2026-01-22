/**
 * SSE (Server-Sent Events) utility for real-time transform status updates.
 * Provides production-grade connection management with:
 * - Exponential backoff reconnection
 * - Configurable max retries
 * - Automatic cleanup on unmount
 * - Type-safe event handling
 */

export interface SSEConnectionOptions {
	/** The SSE endpoint URL */
	url: string;
	/** Maximum number of reconnection attempts (default: 10) */
	maxReconnectAttempts?: number;
	/** Initial reconnection delay in ms (default: 1000) */
	initialDelay?: number;
	/** Maximum reconnection delay in ms (default: 60000) */
	maxDelay?: number;
	/** Heartbeat timeout in ms - reconnect if no heartbeat received (default: 90000) */
	heartbeatTimeout?: number;
	/** Callback when connected */
	onConnect?: () => void;
	/** Callback when a status event is received */
	onStatus?: (data: unknown) => void;
	/** Callback when connection is closed */
	onClose?: () => void;
	/** Callback when an error occurs */
	onError?: (error: Event) => void;
	/** Callback when max reconnection attempts reached */
	onMaxRetriesReached?: () => void;
}

export interface SSEConnection {
	/** Manually disconnect the SSE connection */
	disconnect: () => void;
	/** Manually reconnect the SSE connection */
	reconnect: () => void;
	/** Check if currently connected */
	isConnected: () => boolean;
	/** Get current reconnection attempt count */
	getReconnectAttempts: () => number;
}

/**
 * Creates a managed SSE connection with automatic reconnection.
 * Returns control functions for manual management.
 *
 * @example
 * ```typescript
 * const sse = createSSEConnection({
 *   url: '/api/collection-transforms/stream?collection_id=1',
 *   onStatus: (data) => {
 *     if (data.collection_transform_id === myTransformId) {
 *       refreshData();
 *     }
 *   },
 *   onMaxRetriesReached: () => {
 *     showError('Connection lost. Please refresh the page.');
 *   }
 * });
 *
 * // In onDestroy:
 * sse.disconnect();
 * ```
 */
export function createSSEConnection(options: SSEConnectionOptions): SSEConnection {
	const {
		url,
		maxReconnectAttempts = 10,
		initialDelay = 1000,
		maxDelay = 60000,
		heartbeatTimeout = 90000,
		onConnect,
		onStatus,
		onClose,
		onError,
		onMaxRetriesReached,
	} = options;

	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let heartbeatTimer: ReturnType<typeof setTimeout> | null = null;
	let isDisconnecting = false;
	let isPageVisible = true;

	// Handle page visibility changes
	function handleVisibilityChange() {
		if (typeof document === 'undefined') return;

		isPageVisible = !document.hidden;

		if (isPageVisible && eventSource === null && !isDisconnecting) {
			// Page became visible and we're not connected - try to reconnect
			reconnectAttempts = 0; // Reset attempts when page becomes visible
			connect();
		} else if (!isPageVisible && reconnectTimer) {
			// Page became hidden - pause reconnection attempts
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}
	}

	// Setup visibility listener
	if (typeof document !== 'undefined') {
		document.addEventListener('visibilitychange', handleVisibilityChange);
	}

	function resetHeartbeatTimer(): void {
		if (heartbeatTimer) {
			clearTimeout(heartbeatTimer);
			heartbeatTimer = null;
		}

		// Set a new timeout for heartbeat
		heartbeatTimer = setTimeout(() => {
			console.warn('SSE: Heartbeat timeout - no heartbeat received, reconnecting...');
			eventSource?.close();
			eventSource = null;
			if (!isDisconnecting) {
				scheduleReconnect();
			}
		}, heartbeatTimeout);
	}

	function connect(): void {
		// Don't connect if we're intentionally disconnecting
		if (isDisconnecting) return;

		// Close existing connection if any
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}

		try {
			eventSource = new EventSource(url, { withCredentials: true });

			eventSource.addEventListener('connected', () => {
				reconnectAttempts = 0;
				resetHeartbeatTimer(); // Start heartbeat monitoring
				onConnect?.();
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const data = JSON.parse((event as MessageEvent).data);
					resetHeartbeatTimer(); // Reset heartbeat on any message
					onStatus?.(data);
				} catch (e) {
					console.error('Failed to parse SSE status event:', e);
				}
			});

			eventSource.addEventListener('closed', () => {
				if (heartbeatTimer) {
					clearTimeout(heartbeatTimer);
					heartbeatTimer = null;
				}
				onClose?.();
				if (!isDisconnecting) {
					scheduleReconnect();
				}
			});

			// Handle heartbeat - reset timeout
			eventSource.addEventListener('heartbeat', () => {
				resetHeartbeatTimer();
			});

			eventSource.onerror = (error) => {
				if (heartbeatTimer) {
					clearTimeout(heartbeatTimer);
					heartbeatTimer = null;
				}
				onError?.(error);

				// Only close if not already closed
				if (eventSource && eventSource.readyState !== EventSource.CLOSED) {
					eventSource.close();
				}
				eventSource = null;

				if (!isDisconnecting) {
					scheduleReconnect();
				}
			};
		} catch (e) {
			console.error('Failed to create SSE connection:', e);
			if (!isDisconnecting) {
				scheduleReconnect();
			}
		}
	}

	function scheduleReconnect(): void {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}

		// Don't reconnect if page is hidden
		if (!isPageVisible) {
			console.log('SSE: Page is hidden, deferring reconnection until page is visible');
			return;
		}

		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error(`SSE: Max reconnection attempts (${maxReconnectAttempts}) reached for ${url}`);
			onMaxRetriesReached?.();
			return;
		}

		// Exponential backoff with jitter
		const baseDelay = Math.min(initialDelay * Math.pow(2, reconnectAttempts), maxDelay);
		const jitter = Math.random() * 0.3 * baseDelay; // Add up to 30% jitter
		const delay = baseDelay + jitter;

		reconnectAttempts++;

		console.log(
			`SSE: Scheduling reconnection attempt ${reconnectAttempts}/${maxReconnectAttempts} in ${Math.round(delay)}ms`
		);

		reconnectTimer = setTimeout(() => {
			connect();
		}, delay);
	}

	function disconnect(): void {
		isDisconnecting = true;

		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}

		if (heartbeatTimer) {
			clearTimeout(heartbeatTimer);
			heartbeatTimer = null;
		}

		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}

		// Remove visibility listener
		if (typeof document !== 'undefined') {
			document.removeEventListener('visibilitychange', handleVisibilityChange);
		}

		reconnectAttempts = 0;
	}

	function reconnect(): void {
		isDisconnecting = false;
		reconnectAttempts = 0;
		connect();
	}

	function isConnected(): boolean {
		return eventSource !== null && eventSource.readyState === EventSource.OPEN;
	}

	function getReconnectAttempts(): number {
		return reconnectAttempts;
	}

	// Start connection immediately
	connect();

	return {
		disconnect,
		reconnect,
		isConnected,
		getReconnectAttempts,
	};
}

/**
 * Helper to build SSE URL with query parameters.
 * Ensures proper encoding of values.
 */
export function buildSSEUrl(
	baseUrl: string,
	params: Record<string, string | number | undefined>
): string {
	const url = new URL(baseUrl, window.location.origin);
	for (const [key, value] of Object.entries(params)) {
		if (value !== undefined) {
			url.searchParams.set(key, String(value));
		}
	}
	return url.pathname + url.search;
}
