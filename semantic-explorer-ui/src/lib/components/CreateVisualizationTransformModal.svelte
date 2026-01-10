<script lang="ts">
	import { Button, Modal } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		isOpen?: boolean;
		presetEmbeddedDatasetId?: number | null;
		onClose?: () => void;
		onSuccess?: () => void;
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		source_dataset_id: number;
		embedder_id: number;
		collection_name: string;
	}

	let {
		isOpen = $bindable(false),
		presetEmbeddedDatasetId = null,
		onClose,
		onSuccess,
	}: Props = $props();

	let embeddedDatasets = $state<EmbeddedDataset[]>([]);

	let selectedEmbeddedDatasetId = $state<number | null>(null);
	let transformTitle = $state('');

	// Topic naming fields
	let topicNamingLlmId = $state<number | null>(null);

	$effect(() => {
		if (isOpen && !transformTitle.startsWith('visualization-')) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			transformTitle = `visualization-${date}-${time}`;
		}
	});

	$effect(() => {
		if (isOpen && presetEmbeddedDatasetId !== null && !selectedEmbeddedDatasetId) {
			selectedEmbeddedDatasetId = presetEmbeddedDatasetId;
		}
	});

	// UMAP Configuration
	let umapNNeighbors = $state(15);
	let umapMinDist = $state(0.1);
	let umapMetric = $state('cosine');
	// HDBSCAN Configuration
	let hdbscanMinClusterSize = $state(5);
	let hdbscanMinSamples = $state(1);
	// Datamapplot Visualization Configuration
	let minFontsize = $state(12);
	let maxFontsize = $state(24);
	let fontFamily = $state('Arial, sans-serif');
	let darkmode = $state(true);
	let noiseColor = $state('#999999');
	let labelWrapWidth = $state(16);
	let useMedoids = $state(false);
	let clusterBoundaryPolygons = $state(true);
	let polygonAlpha = $state(0.3);

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

			const response = await fetch('/api/visualization-transforms', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: transformTitle.trim(),
					embedded_dataset_id: selectedEmbeddedDatasetId,
					llm_id: topicNamingLlmId,
					// UMAP parameters
					n_neighbors: umapNNeighbors,
					min_dist: umapMinDist,
					metric: umapMetric,
					// HDBSCAN parameters
					min_cluster_size: hdbscanMinClusterSize,
					min_samples: hdbscanMinSamples,
					// Datamapplot visualization parameters
					min_fontsize: minFontsize,
					max_fontsize: maxFontsize,
					font_family: fontFamily,
					darkmode: darkmode,
					noise_color: noiseColor,
					label_wrap_width: labelWrapWidth,
					use_medoids: useMedoids,
					cluster_boundary_polygons: clusterBoundaryPolygons,
					polygon_alpha: polygonAlpha,
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
		selectedEmbeddedDatasetId = presetEmbeddedDatasetId ?? null;
		umapNNeighbors = 15;
		umapMinDist = 0.1;
		umapMetric = 'cosine';
		hdbscanMinClusterSize = 5;
		hdbscanMinSamples = 1;
		topicNamingLlmId = null;
		// Reset datamapplot visualization parameters
		minFontsize = 12;
		maxFontsize = 24;
		fontFamily = 'Arial, sans-serif';
		darkmode = true;
		noiseColor = '#999999';
		labelWrapWidth = 16;
		useMedoids = false;
		clusterBoundaryPolygons = true;
		polygonAlpha = 0.3;
		error = null;
		isOpen = false;
		onClose?.();
	}

	function handleClose() {
		isOpen = false;
		error = null;
		onClose?.();
	}

	$effect(() => {
		if (presetEmbeddedDatasetId && !selectedEmbeddedDatasetId) {
			selectedEmbeddedDatasetId = presetEmbeddedDatasetId;
		}
	});
</script>

<Modal bind:open={isOpen} onclose={handleClose} size="xl" class="max-w-6xl">
	<div class="w-full max-w-6xl mx-auto px-4 py-4">
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

		<div class="p-4 space-y-4 max-h-[70vh] overflow-y-auto">
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

			<!-- Topic Naming LLM ID (shown when mode is LLM) -->
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
			<!-- UMAP Configuration Section -->
			<div class="border-t border-gray-200 dark:border-gray-700 pt-4">
				<h3 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">UMAP Configuration</h3>

				<div class="space-y-3">
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

			<!-- Datamapplot Visualization Configuration Section -->
			<div class="border-t border-gray-200 dark:border-gray-700 pt-4">
				<h3 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">
					Visualization Settings
				</h3>

				<div class="space-y-3">
					<!-- Min Font Size -->
					<div>
						<label
							for="min-fontsize"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Min Font Size (8-48)
						</label>
						<input
							id="min-fontsize"
							type="number"
							min="8"
							max="48"
							step="0.5"
							bind:value={minFontsize}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Minimum font size for cluster labels
						</p>
					</div>

					<!-- Max Font Size -->
					<div>
						<label
							for="max-fontsize"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Max Font Size (8-48)
						</label>
						<input
							id="max-fontsize"
							type="number"
							min="8"
							max="48"
							step="0.5"
							bind:value={maxFontsize}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Maximum font size for cluster labels
						</p>
					</div>

					<!-- Font Family -->
					<div>
						<label
							for="font-family"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Font Family
						</label>
						<input
							id="font-family"
							type="text"
							bind:value={fontFamily}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Font family for labels (e.g., Arial, sans-serif)
						</p>
					</div>

					<!-- Darkmode -->
					<div class="flex items-center gap-2">
						<input
							id="darkmode"
							type="checkbox"
							bind:checked={darkmode}
							class="w-4 h-4 text-blue-600 rounded border-gray-300 focus:ring-2 focus:ring-blue-500"
						/>
						<label for="darkmode" class="text-xs font-medium text-gray-700 dark:text-gray-300">
							Dark Mode Theme
						</label>
					</div>

					<!-- Noise Color -->
					<div>
						<label
							for="noise-color"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Noise Point Color
						</label>
						<input
							id="noise-color"
							type="text"
							bind:value={noiseColor}
							placeholder="#999999"
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Hex color for unclustered points (e.g., #999999)
						</p>
					</div>

					<!-- Label Wrap Width -->
					<div>
						<label
							for="label-wrap-width"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Label Wrap Width (8-40)
						</label>
						<input
							id="label-wrap-width"
							type="number"
							min="8"
							max="40"
							bind:value={labelWrapWidth}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Character count before wrapping labels
						</p>
					</div>

					<!-- Use Medoids -->
					<div class="flex items-center gap-2">
						<input
							id="use-medoids"
							type="checkbox"
							bind:checked={useMedoids}
							class="w-4 h-4 text-blue-600 rounded border-gray-300 focus:ring-2 focus:ring-blue-500"
						/>
						<label for="use-medoids" class="text-xs font-medium text-gray-700 dark:text-gray-300">
							Use Medoids (instead of centroids for cluster positions)
						</label>
					</div>

					<!-- Cluster Boundary Polygons -->
					<div class="flex items-center gap-2">
						<input
							id="cluster-boundary-polygons"
							type="checkbox"
							bind:checked={clusterBoundaryPolygons}
							class="w-4 h-4 text-blue-600 rounded border-gray-300 focus:ring-2 focus:ring-blue-500"
						/>
						<label
							for="cluster-boundary-polygons"
							class="text-xs font-medium text-gray-700 dark:text-gray-300"
						>
							Draw Cluster Boundary Polygons
						</label>
					</div>

					<!-- Polygon Alpha -->
					<div>
						<label
							for="polygon-alpha"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Polygon Transparency (0.0-1.0)
						</label>
						<input
							id="polygon-alpha"
							type="number"
							min="0.0"
							max="1.0"
							step="0.1"
							bind:value={polygonAlpha}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
						<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
							Transparency of cluster boundary polygons (0=invisible, 1=opaque)
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
