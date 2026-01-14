<script lang="ts">
	import { Button, Modal } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		open?: boolean;
		datasetId?: number | null;
		onSuccess?: (_transformId: number, _transformTitle: string) => void;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
		provider: string;
	}

	let { open = $bindable(false), datasetId = null, onSuccess }: Props = $props();

	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);

	let selectedDatasetId = $state<number | null>(null);
	let selectedEmbedderIds = $state<number[]>([]);
	let transformTitle = $state('');
	let wipeCollection = $state(false);
	let embeddingBatchSize = $state<number | null>(null);

	// Auto-generate title when opening the modal for new transforms
	$effect(() => {
		if (open && !transformTitle.startsWith('dataset-transform-')) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			transformTitle = `dataset-transform-${date}-${time}`;
		}
	});

	$effect(() => {
		if (open && datasetId !== null) {
			selectedDatasetId = datasetId;
		}
	});

	let loadingDatasets = $state(true);
	let loadingEmbedders = $state(true);
	let isCreating = $state(false);
	let error = $state<string | null>(null);

	onMount(() => {
		fetchDatasets();
		fetchEmbedders();
	});

	async function fetchDatasets() {
		try {
			loadingDatasets = true;
			const response = await fetch('/api/datasets');
			if (!response.ok) throw new Error('Failed to fetch datasets');
			datasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch datasets:', e);
		} finally {
			loadingDatasets = false;
		}
	}

	async function fetchEmbedders() {
		try {
			loadingEmbedders = true;
			const response = await fetch('/api/embedders');
			if (!response.ok) throw new Error('Failed to fetch embedders');
			embedders = await response.json();
		} catch (e) {
			console.error('Failed to fetch embedders:', e);
		} finally {
			loadingEmbedders = false;
		}
	}

	function toggleEmbedder(embedderId: number) {
		if (selectedEmbedderIds.includes(embedderId)) {
			selectedEmbedderIds = selectedEmbedderIds.filter((id) => id !== embedderId);
		} else {
			selectedEmbedderIds = [...selectedEmbedderIds, embedderId];
		}
	}

	async function createTransform() {
		error = null;

		if (!transformTitle.trim()) {
			error = 'Title is required';
			return;
		}

		if (!selectedDatasetId) {
			error = 'Dataset is required';
			return;
		}

		if (selectedEmbedderIds.length === 0) {
			error = 'At least one embedder is required';
			return;
		}

		try {
			isCreating = true;

			const response = await fetch('/api/dataset-transforms', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: transformTitle.trim(),
					source_dataset_id: selectedDatasetId,
					embedder_ids: selectedEmbedderIds,
					embedding_batch_size: embeddingBatchSize,
					wipe_collection: wipeCollection,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to create transform: ${response.statusText}`);
			}

			const result = await response.json();
			const newTransformId = result.transform?.dataset_transform_id;
			const newTransformTitle = transformTitle.trim();
			// Close modal immediately
			resetForm();
			// Notify parent with the new transform ID and title
			onSuccess?.(newTransformId, newTransformTitle);
			toastStore.success('Dataset transform created! Embedding generation started.');
		} catch (e) {
			const message = formatError(e, 'Failed to create transform');
			error = message;
			toastStore.error(message);
		} finally {
			isCreating = false;
		}
	}

	function resetForm() {
		transformTitle = '';
		selectedDatasetId = datasetId ?? null;
		selectedEmbedderIds = [];
		wipeCollection = false;
		embeddingBatchSize = null;
		error = null;
		open = false;
	}

	function handleClose() {
		open = false;
		error = null;
	}

	$effect(() => {
		if (datasetId && !selectedDatasetId) {
			selectedDatasetId = datasetId;
		}
	});
</script>

<Modal bind:open onclose={handleClose}>
	<div class="w-full max-w-4xl mx-auto px-4 py-4">
		<h2 class="text-xl font-bold text-gray-900 dark:text-white mb-4">Create Dataset Transform</h2>

		{#if error}
			<div
				class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded text-red-700 dark:text-red-400 text-sm"
			>
				{error}
			</div>
		{/if}

		<div class="space-y-4">
			<!-- Title -->
			<div>
				<label
					for="transform-title"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Title <span class="text-red-500">*</span>
				</label>
				<input
					id="transform-title"
					type="text"
					bind:value={transformTitle}
					placeholder="e.g., FAQ Transform"
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				/>
			</div>

			<!-- Dataset Selection -->
			<div>
				<label
					for="dataset-select"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Source Dataset <span class="text-red-500">*</span>
				</label>
				{#if loadingDatasets}
					<div class="text-sm text-gray-500">Loading datasets...</div>
				{:else}
					<select
						id="dataset-select"
						bind:value={selectedDatasetId}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					>
						<option value={null}>Select a dataset</option>
						{#each datasets as dataset (dataset.dataset_id)}
							<option value={dataset.dataset_id}>{dataset.title}</option>
						{/each}
					</select>
				{/if}
			</div>

			<!-- Embedders -->
			<div>
				<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
					Embedders <span class="text-red-500">*</span>
				</div>
				{#if loadingEmbedders}
					<div class="text-sm text-gray-500">Loading embedders...</div>
				{:else if embedders.length === 0}
					<div class="text-sm text-gray-500">No embedders available</div>
				{:else}
					<div
						class="space-y-2 border border-gray-300 dark:border-gray-600 rounded-lg p-3 dark:bg-gray-700"
					>
						{#each embedders as embedder (embedder.embedder_id)}
							<label class="flex items-center gap-2 cursor-pointer">
								<input
									type="checkbox"
									checked={selectedEmbedderIds.includes(embedder.embedder_id)}
									onchange={() => toggleEmbedder(embedder.embedder_id)}
									class="w-4 h-4 text-blue-600 rounded focus:ring-2"
								/>
								<span class="text-sm text-gray-700 dark:text-gray-300">
									{embedder.name}
									<span class="text-xs text-gray-500 dark:text-gray-400">({embedder.provider})</span
									>
								</span>
							</label>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Embedding Batch Size -->
			<div>
				<label
					for="embedding-batch-size"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Embedding Batch Size <span class="text-xs text-gray-500 dark:text-gray-400"
						>(optional)</span
					>
				</label>
				<input
					id="embedding-batch-size"
					type="number"
					bind:value={embeddingBatchSize}
					min="1"
					max="1000"
					placeholder="Leave empty for default"
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				/>
				<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
					Number of embeddings to process per batch. Lower values use less memory, higher values
					process faster.
				</p>
			</div>

			<!-- Wipe Collection Checkbox -->
			<label class="flex items-center gap-2 cursor-pointer">
				<input
					type="checkbox"
					bind:checked={wipeCollection}
					class="w-4 h-4 text-blue-600 rounded focus:ring-2"
				/>
				<span class="text-sm text-gray-700 dark:text-gray-300">
					Wipe existing Qdrant collection
					<span class="text-xs text-gray-500 dark:text-gray-400"
						>(Warning: This deletes all existing data)</span
					>
				</span>
			</label>
		</div>

		<!-- Actions -->
		<div class="flex gap-3 mt-6">
			<Button onclick={createTransform} disabled={isCreating} color="blue" class="flex-1">
				{isCreating ? 'Creating...' : 'Create Transform'}
			</Button>
			<Button onclick={handleClose} color="alternative" class="flex-1">Cancel</Button>
		</div>
	</div>
</Modal>
