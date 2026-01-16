<script lang="ts">
	interface Props {
		message?: string;
	}

	let { message = 'Loading...' }: Props = $props();
</script>

<div class="text-center py-12" role="status" aria-label={message} aria-live="polite">
	<div
		class="inline-block animate-spin rounded-full h-12 w-12 border-4 border-gray-200 dark:border-gray-700 border-t-blue-600 dark:border-t-blue-400 mb-4"
		aria-hidden="true"
	></div>
	<p class="text-gray-600 dark:text-gray-400 font-medium">{message}</p>
	<p class="text-sm text-gray-500 dark:text-gray-500 mt-2">This may take a few moments...</p>
</div>
