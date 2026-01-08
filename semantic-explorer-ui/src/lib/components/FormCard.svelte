<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		error?: string | null;
		submitting?: boolean;
		submitLabel?: string;
		cancelLabel?: string;
		onSubmit: () => void;
		onCancel: () => void;
		children: Snippet;
	}

	let {
		title,
		error = null,
		submitting = false,
		submitLabel = 'Save',
		cancelLabel = 'Cancel',
		onSubmit,
		onCancel,
		children,
	}: Props = $props();
</script>

<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
	<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
		{title}
	</h2>
	<form
		onsubmit={(e) => {
			e.preventDefault();
			onSubmit();
		}}
	>
		{@render children()}

		{#if error}
			<div
				class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg"
			>
				<p class="text-sm text-red-600 dark:text-red-400">{error}</p>
			</div>
		{/if}

		<div class="flex justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
			<button
				type="button"
				onclick={onCancel}
				disabled={submitting}
				class="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{cancelLabel}
			</button>
			<button
				type="submit"
				disabled={submitting}
				class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{submitting ? 'Saving...' : submitLabel}
			</button>
		</div>
	</form>
</div>
