<script lang="ts">
	import { Modal } from 'flowbite-svelte';
	import ApiExamples from '../ApiExamples.svelte';

	interface Props {
		open?: boolean;
		type: 'collection' | 'dataset';
		resourceId: number;
		examplePayload?: object;
	}

	let { open = $bindable(false), type, resourceId, examplePayload }: Props = $props();

	// Build endpoint URLs using the resourceId
	const collectionFilesEndpoint = $derived(`/api/collections/${resourceId}/files`);
	const datasetItemsEndpoint = $derived(`/api/datasets/${resourceId}/items`);
</script>

<Modal bind:open size="xl" title="API Integration" class="dark:bg-gray-800">
	{#if type === 'collection'}
		<p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
			Use these examples to interact with this collection programmatically.
		</p>

		<div class="mb-4">
			<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
				List files in collection
			</h3>
			<ApiExamples endpoint={`${collectionFilesEndpoint}?page=0&page_size=10`} method="GET" />
		</div>

		<div class="mb-4">
			<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
				Upload files (multipart/form-data)
			</h3>
			<p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
				Note: This endpoint requires multipart/form-data. Use FormData and append files with the key
				"files".
			</p>
			<ApiExamples endpoint={collectionFilesEndpoint} method="POST" />
		</div>

		<div class="mb-4">
			<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
				Download a specific file
			</h3>
			<ApiExamples endpoint={`${collectionFilesEndpoint}/example-file.txt`} method="GET" />
		</div>

		<div>
			<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">Delete a file</h3>
			<ApiExamples endpoint={`${collectionFilesEndpoint}/example-file.txt`} method="DELETE" />
		</div>
	{:else}
		<p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
			Use these examples to upload data to this dataset programmatically.
		</p>

		<div class="mb-4">
			<h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-2">
				Upload dataset items
			</h3>
			<p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
				Send a JSON payload with an array of items. Each item must contain:
			</p>
			<ul class="list-disc list-inside text-sm text-gray-600 dark:text-gray-400 space-y-1 mb-4">
				<li><strong>title</strong>: String - The title/name of the document or item</li>
				<li><strong>chunks</strong>: Array of strings - Text chunks (at least one required)</li>
				<li><strong>metadata</strong>: Object - Any additional metadata as key-value pairs</li>
			</ul>

			<ApiExamples endpoint={datasetItemsEndpoint} method="POST" body={examplePayload} />
		</div>

		<div
			class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg p-4"
		>
			<h4 class="text-sm font-semibold text-yellow-900 dark:text-yellow-300 mb-2">
				Important Notes
			</h4>
			<ul class="list-disc list-inside text-sm text-yellow-800 dark:text-yellow-400 space-y-1">
				<li>Authentication is required via the access_token cookie</li>
				<li>Each item's chunks array must contain at least one chunk</li>
				<li>The items array must contain at least one item</li>
				<li>Metadata can be any valid JSON object</li>
				<li>Response includes "completed" and "failed" arrays with item titles</li>
			</ul>
		</div>
	{/if}
</Modal>
