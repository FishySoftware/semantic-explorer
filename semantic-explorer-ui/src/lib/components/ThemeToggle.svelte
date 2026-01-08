<script lang="ts">
	import { Button, Dropdown, DropdownItem } from 'flowbite-svelte';
	import { SunSolid, MoonSolid, DesktopPcSolid } from 'flowbite-svelte-icons';
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
		class="inline-flex items-center rounded-lg text-sm p-2 text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-700"
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

	<Dropdown triggeredBy="#theme-menu-button" class="w-36">
		{#each ['light', 'dark', 'system'] as theme (theme)}
			<DropdownItem
				onclick={() => selectTheme(theme as Theme)}
				class="flex items-center justify-between"
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
