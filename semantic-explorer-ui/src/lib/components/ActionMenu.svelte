<script lang="ts">
	import { Button, Dropdown, DropdownDivider, DropdownItem } from 'flowbite-svelte';
	import { DotsVerticalOutline } from 'flowbite-svelte-icons';

	interface Action {
		label: string;
		handler: () => void;
		isDangerous?: boolean;
		isDividerBefore?: boolean;
	}

	interface Props {
		actions: Action[];
		id?: string;
	}

	let { actions = [], id = `action-menu-${Math.random().toString(36).substr(2, 9)}` }: Props =
		$props();
</script>

<div>
	<Button color="alternative" size="xs" class="p-1.5" {id} title="More actions">
		<DotsVerticalOutline class="w-5 h-5" />
	</Button>

	<Dropdown
		triggeredBy={`#${id}`}
		class="w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg"
	>
		{#each actions as action, index (index)}
			{#if action.isDividerBefore && index > 0}
				<DropdownDivider />
			{/if}
			<DropdownItem
				onclick={action.handler}
				class={action.isDangerous
					? 'list-none text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/30 px-4 py-2 text-sm'
					: 'list-none text-gray-900 dark:text-gray-100 hover:bg-gray-100 dark:hover:bg-gray-700 px-4 py-2 text-sm'}
			>
				{action.label}
			</DropdownItem>
		{/each}
	</Dropdown>
</div>

<style>
	:global([role='tooltip'] li) {
		list-style: none;
	}
</style>
