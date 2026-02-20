<script lang="ts">
	import { CodeOutline } from 'flowbite-svelte-icons';
	import { copyToClipboard } from './utils/ui-helpers';

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
	let copied = $state(false);

	const TOKEN_PLACEHOLDER = '<YOUR_ACCESS_TOKEN>';

	function getCurlExample(): string {
		const baseUrl = window.location.origin;
		const fullUrl = `${baseUrl}${endpoint}`;

		let curl = `curl '${fullUrl}'`;

		curl += ` \\\n  -H 'Authorization: Bearer ${TOKEN_PLACEHOLDER}'`;

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

		python += `headers = {'Authorization': 'Bearer ${TOKEN_PLACEHOLDER}'}\n`;

		if (body) {
			python += `data = ${JSON.stringify(body, null, 2)}\n\n`;
		}

		python += `response = requests.${method.toLowerCase()}(\n`;
		python += `    '${fullUrl}',\n`;
		python += `    headers=headers`;

		if (body) {
			python += `,\n    json=data`;
		}

		python += `\n)\n\n`;
		python += `print(response.json())`;

		return python;
	}

	async function copyText(text: string) {
		try {
			await copyToClipboard(text);
			copied = true;
			setTimeout(() => (copied = false), 2000);
		} catch (err) {
			console.error('Failed to copy:', err);
		}
	}
</script>

<div class="mt-4">
	<button
		onclick={() => (showExamples = !showExamples)}
		class="inline-flex items-center gap-2 text-sm text-blue-600 dark:text-blue-400 hover:underline"
	>
		<CodeOutline class="w-4 h-4" />
		{showExamples ? 'Hide' : 'Show'} API Examples
	</button>

	{#if showExamples}
		<div class="mt-4 space-y-4">
			<div
				class="p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg text-blue-800 dark:text-blue-400 text-sm"
			>
				Use <code>POST /api/auth/device</code> (OAuth2 Device Authorization Grant) to obtain an access
				token for programmatic API use.
			</div>

			<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4">
				<div class="flex items-center justify-between mb-2">
					<h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300">CURL</h4>
					<button
						onclick={() => copyText(getCurlExample())}
						class="text-xs px-2 py-1 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 rounded transition-colors text-gray-900 dark:text-gray-100"
					>
						{copied ? '✓ Copied!' : 'Copy'}
					</button>
				</div>
				<pre
					class="text-xs bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700 overflow-x-auto"><code
						class="text-gray-900 dark:text-gray-100">{getCurlExample()}</code
					></pre>
			</div>

			<div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4">
				<div class="flex items-center justify-between mb-2">
					<h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300">Python (requests)</h4>
					<button
						onclick={() => copyText(getPythonExample())}
						class="text-xs px-2 py-1 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 rounded transition-colors text-gray-900 dark:text-gray-100"
					>
						{copied ? '✓ Copied!' : 'Copy'}
					</button>
				</div>
				<pre
					class="text-xs bg-white dark:bg-gray-800 p-3 rounded border border-gray-200 dark:border-gray-700 overflow-x-auto"><code
						class="text-gray-900 dark:text-gray-100">{getPythonExample()}</code
					></pre>
			</div>
		</div>
	{/if}
</div>
