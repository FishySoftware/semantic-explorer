<script lang="ts">
	import { onMount } from 'svelte';

	let {
		endpoint,
		method = 'GET',
		body = null,
	} = $props<{
		endpoint: string;
		method?: string;
		body?: any;
	}>();

	let showExamples = $state(false);
	let accessToken = $state<string>('');
	let copied = $state(false);

	function getAccessToken(): string {
		const cookies = document.cookie.split(';');
		for (const cookie of cookies) {
			const [name, value] = cookie.trim().split('=');
			if (name === 'access_token') {
				return value;
			}
		}
		return '';
	}

	function getCurlExample(): string {
		const baseUrl = window.location.origin;
		const fullUrl = `${baseUrl}${endpoint}`;

		let curl = `curl '${fullUrl}'`;

		if (accessToken) {
			curl += ` \\\n  -b 'access_token=${accessToken}'`;
		}

		if (method !== 'GET') {
			curl += ` \\\n  -X ${method}`;
		}

		if (body) {
			curl += ` \\\n  -H 'Content-Type: application/json'`;
			curl += ` \\\n  -d '${JSON.stringify(body, null, 2)}'`;
		}

		return curl;
	}

	function getPythonExample(): string {
		const baseUrl = window.location.origin;
		const fullUrl = `${baseUrl}${endpoint}`;

		let python = `import requests\n\n`;

		if (accessToken) {
			python += `cookies = {'access_token': '${accessToken}'}\n`;
		}

		if (body) {
			python += `data = ${JSON.stringify(body, null, 2)}\n\n`;
		}

		python += `response = requests.${method.toLowerCase()}(\n`;
		python += `    '${fullUrl}'`;

		if (accessToken) {
			python += `,\n    cookies=cookies`;
		}

		if (body) {
			python += `,\n    json=data`;
		}

		python += `\n)\n\n`;
		python += `print(response.json())`;

		return python;
	}

	async function copyToClipboard(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = true;
			setTimeout(() => (copied = false), 2000);
		} catch (err) {
			console.error('Failed to copy:', err);
		}
	}

	onMount(() => {
		accessToken = getAccessToken();
	});
</script>

<div class="mt-4">
	<button
		onclick={() => (showExamples = !showExamples)}
		class="inline-flex items-center gap-2 text-sm text-blue-600 dark:text-blue-400 hover:underline"
	>
		<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				stroke-width="2"
				d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
			></path>
		</svg>
		{showExamples ? 'Hide' : 'Show'} API Examples
	</button>

	{#if showExamples}
		<div class="mt-4 space-y-4">
			{#if !accessToken}
				<div
					class="p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg text-yellow-800 dark:text-yellow-400 text-sm"
				>
					⚠️ No access token found in cookies. You may need to authenticate first.
				</div>
			{/if}

			<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4">
				<div class="flex items-center justify-between mb-2">
					<h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300">CURL</h4>
					<button
						onclick={() => copyToClipboard(getCurlExample())}
						class="text-xs px-2 py-1 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 rounded transition-colors"
					>
						{copied ? '✓ Copied!' : 'Copy'}
					</button>
				</div>
				<pre
					class="text-xs bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700 overflow-x-auto"><code
						>{getCurlExample()}</code
					></pre>
			</div>

			<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4">
				<div class="flex items-center justify-between mb-2">
					<h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300">Python (requests)</h4>
					<button
						onclick={() => copyToClipboard(getPythonExample())}
						class="text-xs px-2 py-1 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 rounded transition-colors"
					>
						{copied ? '✓ Copied!' : 'Copy'}
					</button>
				</div>
				<pre
					class="text-xs bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700 overflow-x-auto"><code
						>{getPythonExample()}</code
					></pre>
			</div>
		</div>
	{/if}
</div>
