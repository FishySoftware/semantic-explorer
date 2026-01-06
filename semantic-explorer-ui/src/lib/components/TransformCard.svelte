<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		isEnabled: boolean;
		isDeleting?: boolean;
		onToggleEnabled: () => void;
		onTrigger: () => void;
		onEdit: () => void;
		onDelete: () => void;
		details: Snippet;
		stats?: Snippet;
		actions?: Snippet;
	}

	let {
		title,
		isEnabled,
		isDeleting = false,
		onToggleEnabled,
		onTrigger,
		onEdit,
		onDelete,
		details,
		stats,
		actions,
	}: Props = $props();
</script>

<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
	<div class="flex justify-between items-start mb-4">
		<div class="flex-1">
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">
				{title}
			</h3>
			<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
				{@render details()}
			</div>
		</div>
		<div class="flex flex-col gap-2">
			<button
				onclick={onToggleEnabled}
				class="px-3 py-1 text-sm rounded-lg {isEnabled
					? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400'
					: 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400'}"
			>
				{isEnabled ? 'Disable' : 'Enable'}
			</button>
			<button
				onclick={onTrigger}
				class="px-3 py-1 text-sm bg-blue-100 text-blue-700 hover:bg-blue-200 rounded-lg dark:bg-blue-900/20 dark:text-blue-400"
			>
				Trigger
			</button>
			<button
				onclick={onEdit}
				class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300"
			>
				Edit
			</button>
			<button
				onclick={onDelete}
				disabled={isDeleting}
				class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{isDeleting ? 'Deleting...' : 'Delete'}
			</button>
			{#if actions}
				{@render actions()}
			{/if}
		</div>
	</div>

	{#if stats}
		<div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
			{@render stats()}
		</div>
	{/if}
</div>
