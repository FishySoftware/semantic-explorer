<script lang="ts">
	import { ExclamationCircleSolid } from 'flowbite-svelte-icons';

	interface Props {
		error: string;
		onRetry?: () => void;
	}

	let { error, onRetry }: Props = $props();
</script>

<div
	class="bg-red-50 dark:bg-red-900/10 border border-red-300 dark:border-red-800 rounded-lg p-5"
	role="alert"
	aria-live="assertive"
	aria-label={error}
>
	<div class="flex items-start gap-3">
		<ExclamationCircleSolid
			class="w-6 h-6 text-red-600 dark:text-red-400 shrink-0 mt-0.5"
			aria-hidden="true"
		/>
		<div class="flex-1">
			<p class="text-red-700 dark:text-red-300 font-medium mb-3">{error}</p>
			{#if onRetry}
				<button
					onclick={onRetry}
					class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900 text-sm font-medium"
				>
					Try Again
				</button>
			{/if}
		</div>
	</div>
</div>
