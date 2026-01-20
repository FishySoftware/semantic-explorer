<script lang="ts">
	import { onMount } from 'svelte';

	interface Props {
		/** Fallback content to show when an error occurs */
		fallback?: import('svelte').Snippet<[{ error: Error; reset: () => void }]>;
		/** Callback when an error is caught */
		onError?: (_error: Error) => void;
		/** Child content */
		children: import('svelte').Snippet;
	}

	let { fallback, onError, children }: Props = $props();

	let error = $state<Error | null>(null);
	let hasError = $state(false);

	function handleError(e: Error) {
		error = e;
		hasError = true;
		onError?.(e);
		console.error('ErrorBoundary caught error:', e);
	}

	function reset() {
		error = null;
		hasError = false;
	}

	// Listen for unhandled errors in this component's subtree
	onMount(() => {
		const errorHandler = (event: ErrorEvent) => {
			// Only handle if it's a render/runtime error, not a network error
			if (event.error instanceof Error) {
				handleError(event.error);
				event.preventDefault();
			}
		};

		const rejectionHandler = (event: PromiseRejectionEvent) => {
			if (event.reason instanceof Error) {
				handleError(event.reason);
				event.preventDefault();
			}
		};

		window.addEventListener('error', errorHandler);
		window.addEventListener('unhandledrejection', rejectionHandler);

		return () => {
			window.removeEventListener('error', errorHandler);
			window.removeEventListener('unhandledrejection', rejectionHandler);
		};
	});
</script>

{#if hasError && error}
	{#if fallback}
		{@render fallback({ error, reset })}
	{:else}
		<!-- Default error UI -->
		<div
			class="min-h-50 flex items-center justify-center p-8 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg"
		>
			<div class="text-center max-w-md">
				<div class="mb-4">
					<svg
						class="mx-auto h-12 w-12 text-red-500 dark:text-red-400"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
						/>
					</svg>
				</div>
				<h3 class="text-lg font-semibold text-red-800 dark:text-red-200 mb-2">
					Something went wrong
				</h3>
				<p class="text-sm text-red-600 dark:text-red-400 mb-4">
					{error.message || 'An unexpected error occurred'}
				</p>
				<div class="flex gap-3 justify-center">
					<button
						onclick={reset}
						class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg text-sm font-medium transition-colors"
					>
						Try Again
					</button>
					<button
						onclick={() => window.location.reload()}
						class="px-4 py-2 border border-red-300 dark:border-red-700 text-red-700 dark:text-red-300 hover:bg-red-100 dark:hover:bg-red-900/40 rounded-lg text-sm font-medium transition-colors"
					>
						Reload Page
					</button>
				</div>
				{#if import.meta.env.DEV}
					<details class="mt-4 text-left">
						<summary class="cursor-pointer text-xs text-red-500 dark:text-red-400">
							Error Details (Dev Mode)
						</summary>
						<pre
							class="mt-2 p-2 bg-red-100 dark:bg-red-900/40 rounded text-xs overflow-auto max-h-40 text-red-800 dark:text-red-200">{error.stack}</pre>
					</details>
				{/if}
			</div>
		</div>
	{/if}
{:else}
	{@render children()}
{/if}
