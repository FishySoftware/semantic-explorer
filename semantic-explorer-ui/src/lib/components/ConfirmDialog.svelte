<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	type Variant = 'primary' | 'danger';

	const dispatch = createEventDispatcher<{ confirm: void; cancel: void }>();

	let {
		open = false,
		title = 'Confirm action',
		message = '',
		confirmLabel = 'Confirm',
		cancelLabel = 'Cancel',
		variant = 'danger' as Variant,
	} = $props<{
		open?: boolean;
		title?: string;
		message?: string;
		confirmLabel?: string;
		cancelLabel?: string;
		variant?: Variant;
	}>();

	function confirm() {
		dispatch('confirm');
	}

	function cancel() {
		dispatch('cancel');
	}

	function confirmClasses() {
		switch (variant) {
			case 'primary':
				return 'bg-blue-600 hover:bg-blue-700 focus-visible:ring-blue-500';
			case 'danger':
			default:
				return 'bg-red-600 hover:bg-red-700 focus-visible:ring-red-500';
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			cancel();
		}
	}
</script>

{#if open}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center px-4 py-6"
		onkeydown={handleKeydown}
		role="presentation"
	>
		<div
			class="absolute inset-0 bg-gray-900/60"
			onclick={cancel}
			aria-hidden="true"
			role="presentation"
		></div>
		<div
			role="alertdialog"
			aria-modal="true"
			aria-labelledby="dialog-title"
			aria-describedby="dialog-message"
			class="relative w-full max-w-md rounded-lg border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
		>
			<div class="border-b border-gray-200 px-5 py-4 dark:border-gray-700">
				<h2 id="dialog-title" class="text-lg font-semibold text-gray-900 dark:text-white">
					{title}
				</h2>
			</div>
			<div id="dialog-message" class="px-5 py-4 text-sm text-gray-600 dark:text-gray-300">
				<p>{message}</p>
			</div>
			<div
				class="flex items-center justify-end gap-3 border-t border-gray-200 px-5 py-4 dark:border-gray-700"
			>
				<button
					onclick={cancel}
					type="button"
					class="rounded-md border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 transition hover:bg-gray-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
				>
					{cancelLabel}
				</button>
				<button
					onclick={confirm}
					type="button"
					class={`rounded-md px-4 py-2 text-sm font-semibold text-white transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 ${confirmClasses()}`}
				>
					{confirmLabel}
				</button>
			</div>
		</div>
	</div>
{/if}
