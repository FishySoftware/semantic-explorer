<script lang="ts">
	import { Avatar, Dropdown, DropdownItem, Navbar, NavBrand } from 'flowbite-svelte';
	import { DesktopPcSolid, MoonSolid, SunSolid } from 'flowbite-svelte-icons';
	import { onMount } from 'svelte';
	import svelteLogo from '../assets/logo.png';
	import { getTheme, setTheme, type Theme } from './utils/theme';

	let appName = 'Semantic Explorer';
	let userName = $state('');
	let userEmail = $state('');
	let userAvatar = $state('');
	let loading = $state(true);
	let currentTheme = $state<Theme>(getTheme());

	interface User {
		username: string;
		email: string;
		avatar?: string;
	}

	async function fetchUserInfo() {
		try {
			const response = await fetch('/api/users/@me');
			if (response.ok) {
				const user: User = await response.json();
				userName = user.username;
				userEmail = user.email;
				userAvatar = user.avatar || '';
			} else {
				console.error('Failed to fetch user info:', response.statusText);
			}
		} catch (error) {
			console.error('Error fetching user info:', error);
		} finally {
			loading = false;
		}
	}

	function selectTheme(theme: Theme) {
		currentTheme = theme;
		setTheme(theme);
	}

	const getThemeLabel = (theme: Theme) => {
		switch (theme) {
			case 'light':
				return 'Light';
			case 'dark':
				return 'Dark';
			case 'system':
				return 'System';
		}
	};

	onMount(() => {
		fetchUserInfo();
	});
</script>

<Navbar fluid class="shrink-0 border-b border-gray-200 dark:border-gray-700" role="banner">
	<NavBrand href="/#" aria-label="Semantic Explorer">
		<img src={svelteLogo} class="mr-3 h-6 sm:h-9" alt="Semantic Explorer logo" />
		<span class="self-center whitespace-nowrap text-xl font-semibold dark:text-white">
			{appName}
		</span>
	</NavBrand>

	<div class="flex items-center gap-3">
		{#if !loading}
			<div
				class="flex items-center gap-2 cursor-pointer"
				id="user-menu-button"
				role="button"
				tabindex="0"
				aria-label="User menu"
				aria-haspopup="menu"
				aria-expanded="false"
			>
				{#if userName}
					<span class="text-sm font-medium dark:text-white">{userName}</span>
				{/if}
				{#if userAvatar}
					<Avatar src={userAvatar} alt={userName} />
				{:else}
					<Avatar alt="User avatar" />
				{/if}
			</div>

			<Dropdown
				triggeredBy="#user-menu-button"
				class="w-48 bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700"
				role="menu"
				aria-label="User preferences"
			>
				{#if userName}
					<div class="px-4 py-3 text-sm text-gray-900 dark:text-white">
						<div class="font-medium truncate">{userName}</div>
						<div class="text-xs text-gray-500 dark:text-gray-400 truncate">{userEmail}</div>
					</div>
				{/if}

				<div class="px-2 py-1 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase">
					Theme
				</div>

				{#each ['light', 'dark', 'system'] as theme (theme)}
					<DropdownItem
						onclick={() => selectTheme(theme as Theme)}
						role="menuitem"
						class="flex items-center justify-between text-gray-900 dark:text-white cursor-pointer"
						aria-label={`Switch to ${getThemeLabel(theme as Theme)} theme`}
					>
						<span class="flex items-center gap-2">
							{#if theme === 'light'}
								<SunSolid class="w-4 h-4" aria-hidden="true" />
							{:else if theme === 'dark'}
								<MoonSolid class="w-4 h-4" aria-hidden="true" />
							{:else}
								<DesktopPcSolid class="w-4 h-4" aria-hidden="true" />
							{/if}
							{getThemeLabel(theme as Theme)}
						</span>
						{#if currentTheme === theme}
							<span class="w-2 h-2 bg-blue-600 dark:bg-blue-400 rounded-full" aria-hidden="true"
							></span>
						{/if}
					</DropdownItem>
				{/each}
			</Dropdown>
		{/if}
	</div>
</Navbar>
