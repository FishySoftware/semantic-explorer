<script lang="ts">
	interface Props {
		error: string;
		onRetry?: () => void;
	}

	let { error, onRetry }: Props = $props();
</script>

<div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
	<p class="text-red-600 dark:text-red-400">{error}</p>
	{#if onRetry}
		<button
			onclick={onRetry}
			class="mt-3 px-4 py-2 bg-red-100 text-red-700 rounded-lg hover:bg-red-200 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50"
		>
			Try Again
		</button>
	{/if}
</div>
