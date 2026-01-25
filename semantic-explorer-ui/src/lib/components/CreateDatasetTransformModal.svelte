<script lang="ts">
	import { Button, Modal } from 'flowbite-svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface DatasetTransform {
		dataset_transform_id: number;
		title: string;
		source_dataset_id: number;
		embedder_ids: number[];
		is_enabled: boolean;
		job_config: any;
	}

	interface Props {
		open?: boolean;
		datasetId?: number | null;
		editingTransform?: DatasetTransform | null;
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

	let {
		open = $bindable(false),
		datasetId = null,
		editingTransform = null,
		onSuccess,
	}: Props = $props();

	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);

	let selectedDatasetId = $state<number | null>(null);
	let selectedEmbedderIds = $state<number[]>([]);
	let transformTitle = $state('');
	let embeddingBatchSize = $state<number | null>(null);

	// Auto-generate title when opening the modal for new transforms (not when editing)
	$effect(() => {
		if (open && !editingTransform && !transformTitle.startsWith('dataset-transform-')) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			transformTitle = `dataset-transform-${date}-${time}`;
		}
	});

	// Populate form when editing an existing transform
	$effect(() => {
		if (editingTransform) {
			transformTitle = editingTransform.title;
			selectedDatasetId = editingTransform.source_dataset_id;
			selectedEmbedderIds = [...editingTransform.embedder_ids];
			embeddingBatchSize = editingTransform.job_config?.embedding_batch_size || null;
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

	// Fetch data when modal opens
	$effect(() => {
		if (open) {
			fetchDatasets();
			fetchEmbedders();
		}
	});

	async function fetchDatasets() {
		try {
			loadingDatasets = true;
			const response = await fetch('/api/datasets');
			if (!response.ok) throw new Error('Failed to fetch datasets');
			const data = await response.json();
			datasets = data.items ?? [];
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
			const data = await response.json();
			embedders = data.items ?? [];
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

			const url = editingTransform
				? `/api/dataset-transforms/${editingTransform.dataset_transform_id}`
				: '/api/dataset-transforms';
			const method = editingTransform ? 'PATCH' : 'POST';

			const body = editingTransform
				? {
						title: transformTitle.trim(),
						embedder_ids: selectedEmbedderIds,
						job_config:
							embeddingBatchSize !== null
								? { embedding_batch_size: embeddingBatchSize }
								: undefined,
					}
				: {
						title: transformTitle.trim(),
						source_dataset_id: selectedDatasetId,
						embedder_ids: selectedEmbedderIds,
						embedding_batch_size: embeddingBatchSize,
					};

			const response = await fetch(url, {
				method,
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				throw new Error(
					`Failed to ${editingTransform ? 'update' : 'create'} transform: ${response.statusText}`
				);
			}

			const result = await response.json();
			const transformId = editingTransform
				? editingTransform.dataset_transform_id
				: result.transform?.dataset_transform_id;
			const transformTitleValue = transformTitle.trim();

			// Close modal immediately
			resetForm();
			// Notify parent with the transform ID and title
			onSuccess?.(transformId, transformTitleValue);
			toastStore.success(
				`Dataset transform ${editingTransform ? 'updated' : 'created'}${!editingTransform ? '! Embedding generation started.' : ''}`
			);
		} catch (e) {
			const message = formatError(
				e,
				`Failed to ${editingTransform ? 'update' : 'create'} transform`
			);
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
		embeddingBatchSize = null;
		error = null;
		open = false;
		// Don't clear editingTransform - let parent handle that
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
		<h2 class="text-xl font-bold text-gray-900 dark:text-white mb-4">
			{editingTransform ? 'Edit Dataset Transform' : 'Create Dataset Transform'}
		</h2>

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

			<!-- Dataset Selection (only shown when creating, not editing) -->
			{#if !editingTransform}
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
			{:else}
				<!-- Show source dataset info when editing -->
				<div>
					<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
						Source Dataset
					</div>
					{#if datasets.length > 0}
						<div class="text-sm text-gray-500">Loading datasets...</div>
					{:else}
						{@const sourceDataset = datasets.find((d) => d.dataset_id === selectedDatasetId)}
						{#if sourceDataset}
							<div
								class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-gray-50 dark:bg-gray-700 text-sm text-gray-900 dark:text-white"
							>
								{sourceDataset.title}
							</div>
						{:else}
							<div
								class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-gray-50 dark:bg-gray-700 text-sm text-gray-900 dark:text-white"
							>
								Dataset {selectedDatasetId}
							</div>
						{/if}
					{/if}
				</div>
			{/if}

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
		</div>

		<!-- Actions -->
		<div class="flex gap-3 mt-6">
			<Button onclick={createTransform} disabled={isCreating} color="blue" class="flex-1">
				{isCreating
					? editingTransform
						? 'Updating...'
						: 'Creating...'
					: editingTransform
						? 'Update Transform'
						: 'Create Transform'}
			</Button>
			<Button onclick={handleClose} color="alternative" class="flex-1">Cancel</Button>
		</div>
	</div>
</Modal>
