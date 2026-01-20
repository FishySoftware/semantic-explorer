<script lang="ts">
	import { Button, Dropdown, DropdownItem } from 'flowbite-svelte';
	import { DesktopPcSolid, MoonSolid, SunSolid } from 'flowbite-svelte-icons';
	import { getTheme, setTheme, type Theme } from '../utils/theme';

	let currentTheme = $state<Theme>(getTheme());

	function selectTheme(theme: Theme) {
		currentTheme = theme;
		setTheme(theme);
	}

	const getLabel = (theme: Theme) => {
		switch (theme) {
			case 'light':
				return 'Light';
			case 'dark':
				return 'Dark';
			case 'system':
				return 'System';
		}
	};
</script>

<div class="flex items-center">
	<Button
		class="inline-flex items-center rounded-lg text-sm p-2cursor-pointer"
		size="sm"
		id="theme-menu-button"
	>
		{#if currentTheme === 'light'}
			<SunSolid class="w-5 h-5" />
		{:else if currentTheme === 'dark'}
			<MoonSolid class="w-5 h-5" />
		{:else}
			<DesktopPcSolid class="w-5 h-5" />
		{/if}
	</Button>

	<Dropdown
		triggeredBy="#theme-menu-button"
		class="w-36 bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 list-none"
	>
		{#each ['light', 'dark', 'system'] as theme (theme)}
			<DropdownItem
				onclick={() => selectTheme(theme as Theme)}
				class="flex items-center justify-between text-gray-900 dark:text-white hover:bg-gray-50 dark:hover:bg-gray-700 list-none"
			>
				<span class="flex items-center gap-2">
					{#if theme === 'light'}
						<SunSolid class="w-4 h-4" />
					{:else if theme === 'dark'}
						<MoonSolid class="w-4 h-4" />
					{:else}
						<DesktopPcSolid class="w-4 h-4" />
					{/if}
					{getLabel(theme as Theme)}
				</span>
				{#if currentTheme === theme}
					<span class="w-2 h-2 bg-blue-600 dark:bg-blue-400 rounded-full"></span>
				{/if}
			</DropdownItem>
		{/each}
	</Dropdown>
</div>
