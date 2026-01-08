<script lang="ts">
	import type { Snippet } from 'svelte';

	export interface Tab {
		id: string;
		label: string;
		icon?: string;
	}

	interface Props {
		tabs: Tab[];
		activeTabId?: string;
		onChange?: (_tabId: string) => void;
		children: Snippet<[string]>; // Pass tab id to the snippet
	}

	let { tabs, activeTabId = tabs[0]?.id, onChange, children }: Props = $props();

	let currentTabId = $derived(activeTabId ?? tabs[0]?.id ?? '');

	function handleTabClick(tabId: string) {
		currentTabId = tabId;
		onChange?.(tabId);
	}

	function handleKeyDown(event: KeyboardEvent, _tabId: string) {
		const currentIndex = tabs.findIndex((t) => t.id === currentTabId);
		let newIndex = currentIndex;

		if (event.key === 'ArrowRight') {
			event.preventDefault();
			newIndex = (currentIndex + 1) % tabs.length;
		} else if (event.key === 'ArrowLeft') {
			event.preventDefault();
			newIndex = (currentIndex - 1 + tabs.length) % tabs.length;
		} else if (event.key === 'Home') {
			event.preventDefault();
			newIndex = 0;
		} else if (event.key === 'End') {
			event.preventDefault();
			newIndex = tabs.length - 1;
		}

		if (newIndex !== currentIndex) {
			const newTabId = tabs[newIndex]?.id;
			if (newTabId) {
				handleTabClick(newTabId);
			}
		}
	}
</script>

<div class="w-full">
	<!-- Tab buttons -->
	<div class="border-b border-gray-200 dark:border-gray-700">
		<div class="flex flex-wrap gap-2 sm:gap-0">
			{#each tabs as tab (tab.id)}
				<button
					type="button"
					onclick={() => handleTabClick(tab.id)}
					onkeydown={(e) => handleKeyDown(e, tab.id)}
					role="tab"
					aria-selected={currentTabId === tab.id}
					aria-controls={`${tab.id}-panel`}
					class="px-4 py-3 font-medium text-sm border-b-2 transition-colors
						{currentTabId === tab.id
						? 'text-blue-600 dark:text-blue-400 border-blue-600 dark:border-blue-400'
						: 'text-gray-600 dark:text-gray-400 border-transparent hover:text-gray-900 dark:hover:text-gray-200 hover:border-gray-300 dark:hover:border-gray-600'}"
				>
					{#if tab.icon}
						<span class="mr-2">{tab.icon}</span>
					{/if}
					{tab.label}
				</button>
			{/each}
		</div>
	</div>

	<!-- Tab content -->
	<div class="mt-4">
		{@render children(currentTabId)}
	</div>
</div>

<style>
	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	:global(.animate-fadeIn) {
		animation: fadeIn 150ms ease-in-out;
	}
</style>
