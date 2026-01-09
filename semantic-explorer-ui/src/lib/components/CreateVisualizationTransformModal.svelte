<script lang="ts">
	import { Button, Modal } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		open?: boolean;
		embeddedDatasetId?: number | null;
		onSuccess?: () => void;
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		source_dataset_id: number;
		embedder_id: number;
		collection_name: string;
	}

	let { open = $bindable(false), embeddedDatasetId = null, onSuccess }: Props = $props();

	let embeddedDatasets = $state<EmbeddedDataset[]>([]);

	let selectedEmbeddedDatasetId = $state<number | null>(null);
	let transformTitle = $state('');

	// Topic naming fields
	let topicNamingMode = $state('tfidf');
	let topicNamingLlmId = $state<number | null>(null);

	// Auto-generate title when opening the modal for new transforms
	$effect(() => {
		if (open && !transformTitle.startsWith('visualization-')) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			transformTitle = `visualization-${date}-${time}`;
		}
	});

	$effect(() => {
		if (open && embeddedDatasetId !== null) {
			selectedEmbeddedDatasetId = embeddedDatasetId;
		}
	});

	// UMAP Configuration
	let umapNNeighbors = $state(15);
	let umapNComponents = $state(2); // 2D or 3D
	let umapMinDist = $state(0.1);
	let umapMetric = $state('cosine');

	// HDBSCAN Configuration
	let hdbscanMinClusterSize = $state(5);
	let hdbscanMinSamples = $state(1);

	let loadingDatasets = $state(true);
	let isCreating = $state(false);
	let error = $state<string | null>(null);

	onMount(() => {
		fetchEmbeddedDatasets();
	});

	async function fetchEmbeddedDatasets() {
		try {
			loadingDatasets = true;
			const response = await fetch('/api/embedded-datasets');
			if (!response.ok) throw new Error('Failed to fetch embedded datasets');
			embeddedDatasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch embedded datasets:', e);
		} finally {
			loadingDatasets = false;
		}
	}

	async function createTransform() {
		error = null;

		if (!transformTitle.trim()) {
			error = 'Title is required';
			return;
		}

		if (!selectedEmbeddedDatasetId) {
			error = 'Embedded Dataset is required';
			return;
		}

		try {
			isCreating = true;

			const visualizationConfig = {
				umap_config: {
					n_neighbors: umapNNeighbors,
					n_components: umapNComponents,
					min_dist: umapMinDist,
					metric: umapMetric,
				},
				hdbscan_config: {
					min_cluster_size: hdbscanMinClusterSize,
					min_samples: hdbscanMinSamples,
				},
				topic_naming_mode: topicNamingMode,
				topic_naming_llm_id: topicNamingLlmId,
			};

			const response = await fetch('/api/visualization-transforms', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: transformTitle.trim(),
					embedded_dataset_id: selectedEmbeddedDatasetId,
					visualization_config: visualizationConfig,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to create visualization: ${response.statusText}`);
			}

			resetForm();
			onSuccess?.();
		} catch (e) {
			const message = formatError(e, 'Failed to create visualization transform');
			error = message;
			toastStore.error(message);
		} finally {
			isCreating = false;
		}
	}

	function resetForm() {
		transformTitle = '';
		selectedEmbeddedDatasetId = embeddedDatasetId ?? null;
		umapNNeighbors = 15;
		umapNComponents = 2;
		umapMinDist = 0.1;
		umapMetric = 'cosine';
		hdbscanMinClusterSize = 5;
		hdbscanMinSamples = 1;
		topicNamingMode = 'tfidf';
		topicNamingLlmId = null;
		error = null;
		open = false;
	}

	function handleClose() {
		open = false;
		error = null;
	}

	$effect(() => {
		if (embeddedDatasetId && !selectedEmbeddedDatasetId) {
			selectedEmbeddedDatasetId = embeddedDatasetId;
		}
	});
</script>

<Modal bind:open onclose={handleClose}>
	<div class="w-full max-w-4xl mx-auto px-4 py-4">
		<h2 class="text-xl font-bold text-gray-900 dark:text-white mb-4">
			Create Visualization Transform
		</h2>

		{#if error}
			<div
				class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded text-red-700 dark:text-red-400 text-sm"
			>
				{error}
			</div>
		{/if}

		<div class="space-y-4 max-h-[70vh] overflow-y-auto">
			<!-- Title -->
			<div>
				<label
					for="viz-title"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Title <span class="text-red-500">*</span>
				</label>
				<input
					id="viz-title"
					type="text"
					bind:value={transformTitle}
					placeholder="e.g., Embedding Visualization 2D"
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				/>
			</div>

			<!-- Embedded Dataset Selection -->
			<div>
				<label
					for="dataset-select"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Embedded Dataset <span class="text-red-500">*</span>
				</label>
				{#if loadingDatasets}
					<div class="text-sm text-gray-500">Loading embedded datasets...</div>
				{:else if embeddedDatasets.length === 0}
					<div class="text-sm text-gray-500">No embedded datasets available</div>
				{:else}
					<select
						id="dataset-select"
						bind:value={selectedEmbeddedDatasetId}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					>
						<option value={null}>Select an embedded dataset</option>
						{#each embeddedDatasets as dataset (dataset.embedded_dataset_id)}
							<option value={dataset.embedded_dataset_id}>{dataset.title}</option>
						{/each}
					</select>
				{/if}
			</div>

			<!-- Topic Naming Mode -->
			<div>
				<label
					for="topic-naming-mode"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Topic Naming Mode
				</label>
				<select
					id="topic-naming-mode"
					bind:value={topicNamingMode}
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				>
					<option value="tfidf">TF-IDF</option>
					<option value="llm">LLM</option>
				</select>
			</div>

			<!-- Topic Naming LLM ID (shown when mode is LLM) -->
			{#if topicNamingMode === 'llm'}
				<div>
					<label
						for="topic-naming-llm"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						LLM for Topic Naming <span class="text-xs text-gray-500 dark:text-gray-400"
							>(optional)</span
						>
					</label>
					<input
						id="topic-naming-llm"
						type="number"
						bind:value={topicNamingLlmId}
						placeholder="LLM ID"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					/>
				</div>
			{/if}

			<!-- UMAP Configuration Section -->
			<div class="border-t border-gray-200 dark:border-gray-700 pt-4">
				<h3 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">UMAP Configuration</h3>

				<div class="space-y-3">
					<!-- Dimensionality -->
					<div>
						<label
							for="umap-components"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Dimensionality
						</label>
						<select
							id="umap-components"
							bind:value={umapNComponents}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						>
							<option value={2}>2D (2 components)</option>
							<option value={3}>3D (3 components)</option>
						</select>
					</div>

					<!-- N Neighbors -->
					<div>
						<label
							for="umap-neighbors"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							N Neighbors (1-100)
						</label>
						<input
							id="umap-neighbors"
							type="number"
							min="1"
							max="100"
							bind:value={umapNNeighbors}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Higher = preserve more global structure
						</p>
					</div>

					<!-- Min Distance -->
					<div>
						<label
							for="umap-mindist"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Min Distance (0-1)
						</label>
						<input
							id="umap-mindist"
							type="number"
							min="0"
							max="1"
							step="0.01"
							bind:value={umapMinDist}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Lower = tighter clusters</p>
					</div>

					<!-- Metric -->
					<div>
						<label
							for="umap-metric"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Distance Metric
						</label>
						<select
							id="umap-metric"
							bind:value={umapMetric}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						>
							<option value="cosine">Cosine</option>
							<option value="euclidean">Euclidean</option>
							<option value="manhattan">Manhattan</option>
							<option value="hamming">Hamming</option>
						</select>
					</div>
				</div>
			</div>

			<!-- HDBSCAN Configuration Section -->
			<div class="border-t border-gray-200 dark:border-gray-700 pt-4">
				<h3 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">HDBSCAN Clustering</h3>

				<div class="space-y-3">
					<!-- Min Cluster Size -->
					<div>
						<label
							for="hdbscan-size"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Min Cluster Size (1-1000)
						</label>
						<input
							id="hdbscan-size"
							type="number"
							min="1"
							max="1000"
							bind:value={hdbscanMinClusterSize}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Minimum points to form a cluster
						</p>
					</div>

					<!-- Min Samples -->
					<div>
						<label
							for="hdbscan-samples"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Min Samples (1-100)
						</label>
						<input
							id="hdbscan-samples"
							type="number"
							min="1"
							max="100"
							bind:value={hdbscanMinSamples}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Affects cluster density threshold
						</p>
					</div>
				</div>
			</div>
		</div>

		<!-- Actions -->
		<div class="flex gap-3 mt-6">
			<Button onclick={createTransform} disabled={isCreating} color="blue" class="flex-1">
				{isCreating ? 'Creating...' : 'Create Visualization'}
			</Button>
			<Button onclick={handleClose} color="alternative" class="flex-1">Cancel</Button>
		</div>
	</div>
</Modal>
