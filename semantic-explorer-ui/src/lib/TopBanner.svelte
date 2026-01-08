<script lang="ts">
	import { Avatar, Navbar, NavBrand, Tooltip } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import svelteLogo from '../assets/logo.png';
	import ThemeToggle from './components/ThemeToggle.svelte';

	let appName = 'Semantic Explorer';
	let userName = '';
	let userEmail = '';
	let userAvatar = '';
	let loading = true;

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

	onMount(() => {
		fetchUserInfo();
	});
</script>

<Navbar fluid class="shrink-0 border-b border-gray-200 dark:border-gray-700">
	<NavBrand href="/#/datasets">
		<img src={svelteLogo} class="mr-3 h-6 sm:h-9" alt="App Logo" />
		<span class="self-center whitespace-nowrap text-xl font-semibold dark:text-white">
			{appName}
		</span>
	</NavBrand>

	<div class="flex items-center gap-3">
		<ThemeToggle />

		{#if loading}
			<span class="text-sm text-gray-400 dark:text-gray-500">Loading...</span>
		{:else if userName}
			<span class="text-sm font-medium dark:text-white">{userName}</span>
			<div id="user-avatar">
				<Avatar src={userAvatar} />
			</div>
			<Tooltip triggeredBy="#user-avatar" placement="bottom">
				{userEmail}
			</Tooltip>
		{/if}
	</div>
</Navbar>
