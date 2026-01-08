<script lang="ts">
	import { onDestroy } from 'svelte';
	import { toastStore, type ToastMessage } from '../utils/notifications';

	let toasts = $state<ToastMessage[]>([]);

	const unsubscribe = toastStore.subscribe((messages) => {
		toasts = messages;
	});

	onDestroy(() => {
		unsubscribe();
	});

	function getVariantClasses(variant: ToastMessage['variant']) {
		switch (variant) {
			case 'success':
				return 'border-green-500 bg-green-50 text-green-900 dark:border-green-500 dark:bg-green-900/80 dark:text-green-50';
			case 'error':
				return 'border-red-500 bg-red-50 text-red-900 dark:border-red-500 dark:bg-red-900/80 dark:text-red-50';
			case 'warning':
				return 'border-yellow-500 bg-yellow-50 text-yellow-900 dark:border-yellow-500 dark:bg-yellow-900/80 dark:text-yellow-50';
			default:
				return 'border-blue-500 bg-blue-50 text-blue-900 dark:border-blue-500 dark:bg-blue-900/80 dark:text-blue-50';
		}
	}
</script>

{#if toasts.length > 0}
	<div class="pointer-events-none fixed top-4 right-4 z-50 flex w-full max-w-sm flex-col gap-3">
		{#each toasts as toast (toast.id)}
			<div
				class={`pointer-events-auto overflow-hidden rounded-lg border shadow-lg transition-all ${getVariantClasses(
					toast.variant
				)}`}
			>
				<div class="flex items-start gap-3 p-4">
					<div class="flex-1">
						{#if toast.title}
							<p class="text-sm font-semibold">{toast.title}</p>
						{/if}
						<p class="text-sm leading-5">{toast.message}</p>
					</div>
					<button
						title="Dismiss"
						class="rounded-md p-1 text-xs font-semibold uppercase text-current opacity-75 transition hover:opacity-100"
						onclick={() => toastStore.dismiss(toast.id)}
					>
						Dismiss
					</button>
				</div>
			</div>
		{/each}
	</div>
{/if}
