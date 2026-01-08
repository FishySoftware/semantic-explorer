<script lang="ts">
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		resourceType: 'collections' | 'datasets' | 'embedders' | 'llms';
		resourceId: number;
		onGrabComplete?: () => void;
	}

	let { resourceType, resourceId, onGrabComplete }: Props = $props();
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(async () => {
		await performGrab();
	});

	async function performGrab() {
		try {
			loading = true;
			error = null;

			const response = await fetch(`/api/marketplace/${resourceType}/${resourceId}/grab`, {
				method: 'POST',
			});

			if (!response.ok) {
				throw new Error(`Failed to grab ${resourceType}: ${response.statusText}`);
			}

			const newResource = await response.json();

			toastStore.success(
				`${resourceType.charAt(0).toUpperCase() + resourceType.slice(1, -1)} grabbed successfully!`
			);

			// Navigate to the new resource
			if (resourceType === 'collections' && newResource.collection_id) {
				window.location.hash = `#/collections/${newResource.collection_id}/details`;
			} else if (resourceType === 'datasets' && newResource.dataset_id) {
				window.location.hash = `#/datasets/${newResource.dataset_id}/details`;
			} else if (resourceType === 'embedders' && newResource.name) {
				window.location.hash = `#/embedders?name=${encodeURIComponent(newResource.name)}`;
			} else if (resourceType === 'llms' && newResource.name) {
				window.location.hash = `#/llms?name=${encodeURIComponent(newResource.name)}`;
			}

			if (onGrabComplete) {
				onGrabComplete();
			}
		} catch (e) {
			const message = formatError(e, `Failed to grab ${resourceType.slice(0, -1)}`);
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}
</script>

<div class="max-w-md mx-auto">
	{#if loading}
		<div class="flex flex-col items-center justify-center py-12">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mb-4"></div>
			<p class="text-gray-600 dark:text-gray-400">Grabbing {resourceType}...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<h3 class="text-lg font-semibold text-red-700 dark:text-red-400 mb-2">Error</h3>
			<p class="text-red-700 dark:text-red-400 mb-4">{error}</p>
			<button
				onclick={() => (window.location.hash = '#/marketplace')}
				class="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
			>
				Back to Marketplace
			</button>
		</div>
	{/if}
</div>
