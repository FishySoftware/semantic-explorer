<script lang="ts">
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	const { transformId, onBack } = $props<{ transformId: number; onBack: () => void }>();

	let transform = $state<any>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(async () => {
		loading = true;
		error = null;
		try {
			const response = await fetch(`/api/transforms/${transformId}`);
			if (!response.ok) throw new Error('Failed to fetch transform details');
			transform = await response.json();
		} catch (e) {
			error = formatError(e, 'Failed to load transform details');
			toastStore.error(error);
		} finally {
			loading = false;
		}
	});
</script>

<div class="mb-4">
	<button class="text-blue-600 hover:underline" onclick={onBack}>&larr; Back to Transforms</button>
</div>

{#if loading}
	<div>Loading...</div>
{:else if error}
	<div class="text-red-600">{error}</div>
{:else if transform}
	<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
		<h2 class="text-2xl font-bold mb-2">{transform.title}</h2>
		<div class="mb-2 text-gray-600 dark:text-gray-300">Transform ID: {transform.transform_id}</div>
		<div class="mb-2">Job Type: <span class="font-mono">{transform.job_type}</span></div>
		<div class="mb-2">Enabled: {transform.is_enabled ? 'Yes' : 'No'}</div>
		<div class="mb-2">Created: {transform.created_at}</div>
		<div class="mb-2">Updated: {transform.updated_at}</div>
		<div class="mb-4">
			<h3 class="font-semibold mb-1">Job Config</h3>
			<pre class="bg-gray-100 dark:bg-gray-900 rounded p-2 overflow-x-auto text-xs">{JSON.stringify(
					transform.job_config,
					null,
					2
				)}</pre>
		</div>
		{#if transform.job_config?.chunking}
			<div class="mb-4">
				<h3 class="font-semibold mb-1">Chunking Config</h3>
				<pre
					class="bg-gray-100 dark:bg-gray-900 rounded p-2 overflow-x-auto text-xs">{JSON.stringify(
						transform.job_config.chunking,
						null,
						2
					)}</pre>
			</div>
		{/if}
		{#if transform.job_config?.extraction}
			<div class="mb-4">
				<h3 class="font-semibold mb-1">Extraction Config</h3>
				<pre
					class="bg-gray-100 dark:bg-gray-900 rounded p-2 overflow-x-auto text-xs">{JSON.stringify(
						transform.job_config.extraction,
						null,
						2
					)}</pre>
			</div>
		{/if}
	</div>
{/if}
