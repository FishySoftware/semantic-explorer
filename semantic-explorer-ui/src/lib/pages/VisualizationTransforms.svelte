<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import type { VisualizationConfig, VisualizationTransform } from '../types/visualizations';

	// Helper function for tooltip display with hover persistence
	function showTooltip(event: MouseEvent, text: string) {
		const button = event.target as HTMLElement;
		const tooltip = document.createElement('div');
		tooltip.className =
			'fixed bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 px-3 py-2 rounded text-sm z-50 max-w-md';
		tooltip.textContent = text;
		tooltip.style.pointerEvents = 'auto';
		document.body.appendChild(tooltip);

		const updatePosition = () => {
			const rect = button.getBoundingClientRect();
			tooltip.style.left = rect.left + rect.width / 2 - tooltip.offsetWidth / 2 + 'px';
			tooltip.style.top = rect.top - tooltip.offsetHeight - 5 + 'px';
		};

		updatePosition();

		const hideTooltip = () => {
			tooltip.remove();
			button.removeEventListener('mouseleave', hideTooltip);
			tooltip.removeEventListener('mouseleave', hideTooltip);
		};

		button.addEventListener('mouseleave', hideTooltip);
		tooltip.addEventListener('mouseleave', hideTooltip);
	}

	// Info icon SVG component
	function InfoIcon() {
		return `<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg"><path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"></path></svg>`;
	}

	interface EmbeddedDataset {
		embedded_dataset_id: number;
		title: string;
		embedder_name: string;
		source_dataset_title: string;
	}

	interface Stats {
		visualization_transform_id: number;
		total_points: number;
		total_clusters: number;
		noise_points: number;
	}

	let transforms = $state<VisualizationTransform[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let llms = $state<Array<{ llm_id: number; name: string; provider: string }>>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let editingTransform = $state<VisualizationTransform | null>(null);
	let newTitle = $state('');
	let newEmbeddedDatasetId = $state<number | null>(null);

	// Default configuration values - must match backend defaults in crates/core/src/models.rs
	const DEFAULT_CONFIG: VisualizationConfig = {
		n_neighbors: 15,
		min_dist: 0.1,
		metric: 'cosine',
		min_cluster_size: 15,
		min_samples: 5,
		topic_naming_llm_id: null,
		// Datamapplot create_interactive_plot parameters
		inline_data: true,
		noise_label: 'Unlabelled',
		noise_color: '#999999',
		color_label_text: true,
		label_wrap_width: 16,
		width: '100%',
		height: 800,
		darkmode: true,
		palette_hue_shift: 0.0,
		palette_hue_radius_dependence: 1.0,
		palette_theta_range: 0.19634954084936207, // π/16 - must match backend
		use_medoids: false,
		cluster_boundary_polygons: true,
		polygon_alpha: 0.1,
		cvd_safer: false,
		enable_topic_tree: true,
		// Datamapplot render_html parameters
		title: null,
		sub_title: null,
		title_font_size: 36,
		sub_title_font_size: 18,
		text_collision_size_scale: 3.0,
		text_min_pixel_size: 12.0,
		text_max_pixel_size: 36.0,
		font_family: 'Roboto',
		font_weight: 600,
		tooltip_font_family: 'Roboto',
		tooltip_font_weight: 400,
		logo: null,
		logo_width: 256,
		line_spacing: 0.95,
		min_fontsize: 12,
		max_fontsize: 24,
		text_outline_width: 8,
		text_outline_color: '#eeeeeedd',
		point_size_scale: null,
		point_hover_color: '#aa0000bb',
		point_radius_min_pixels: 0.01,
		point_radius_max_pixels: 24,
		point_line_width_min_pixels: 0.001,
		point_line_width_max_pixels: 3,
		point_line_width: 0.001,
		cluster_boundary_line_width: 1.0,
		initial_zoom_fraction: 1.0,
		background_color: null,
		background_image: null,
	};

	// Merge loaded config with defaults to handle missing fields from older records
	function applyDefaults(loadedConfig: Partial<VisualizationConfig>): VisualizationConfig {
		return { ...DEFAULT_CONFIG, ...loadedConfig };
	}

	let config = $state<VisualizationConfig>({ ...DEFAULT_CONFIG });
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<VisualizationTransform | null>(null);

	$effect(() => {
		if (showCreateForm && !editingTransform && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `visualization-${date}-${time}`;
		}
	});

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/visualization-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			const rawTransforms = await response.json();
			
			// Apply defaults to all loaded transforms to handle missing fields from older records
			transforms = rawTransforms.map((t: VisualizationTransform) => ({
				...t,
				visualization_config: applyDefaults(t.visualization_config),
			}));

			// Fetch stats for each transform
			for (const transform of transforms) {
				fetchStatsForTransform(transform.visualization_transform_id);
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch visualization transforms');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/visualization-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function fetchEmbeddedDatasets() {
		try {
			const response = await fetch('/api/embedded-datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			embeddedDatasets = await response.json();
			// Auto-select first option if available and nothing selected
			if (embeddedDatasets.length > 0 && newEmbeddedDatasetId === null) {
				newEmbeddedDatasetId = embeddedDatasets[0].embedded_dataset_id;
			}
		} catch (e) {
			console.error('Failed to fetch embedded datasets:', e);
		}
	}

	async function fetchLLMs() {
		try {
			const response = await fetch('/api/llms');
			if (!response.ok) {
				throw new Error(`Failed to fetch LLMs: ${response.statusText}`);
			}
			llms = await response.json();
			// Auto-select first option if available and nothing selected
			if (llms.length > 0 && config.topic_naming_llm_id === null) {
				config.topic_naming_llm_id = llms[0].llm_id;
			}
		} catch (e) {
			console.error('Failed to fetch LLMs:', e);
		}
	}

	async function createTransform() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		if (!newEmbeddedDatasetId) {
			createError = 'Embedded Dataset is required';
			return;
		}

		try {
			creating = true;
			createError = null;

			const url = editingTransform
				? `/api/visualization-transforms/${editingTransform.visualization_transform_id}`
				: '/api/visualization-transforms';
			const method = editingTransform ? 'PATCH' : 'POST';

			const body = editingTransform
				? {
						title: newTitle,
						visualization_config: config,
					}
				: {
						title: newTitle,
						embedded_dataset_id: newEmbeddedDatasetId,
						llm_id: config.topic_naming_llm_id,
						n_neighbors: config.n_neighbors,
						min_dist: config.min_dist,
						metric: config.metric,
						min_cluster_size: config.min_cluster_size,
						min_samples: config.min_samples,
						min_fontsize: config.min_fontsize,
						max_fontsize: config.max_fontsize,
						font_family: config.font_family,
						darkmode: config.darkmode,
						noise_color: config.noise_color,
						label_wrap_width: config.label_wrap_width,
						use_medoids: config.use_medoids,
						cluster_boundary_polygons: config.cluster_boundary_polygons,
						polygon_alpha: config.polygon_alpha,
					};

			const response = await fetch(url, {
				method,
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				throw new Error(
					`Failed to ${editingTransform ? 'update' : 'create'} visualization transform: ${response.statusText}`
				);
			}

			const savedTransform = await response.json();

			if (editingTransform) {
				transforms = transforms.map((t) =>
					t.visualization_transform_id === savedTransform.visualization_transform_id
						? savedTransform
						: t
				);
				toastStore.success('Visualization transform updated successfully');
			} else {
				transforms = [...transforms, savedTransform];
				toastStore.success('Visualization transform created successfully');
			}

			resetForm();
		} catch (e) {
			const message = formatError(
				e,
				`Failed to ${editingTransform ? 'update' : 'create'} visualization transform`
			);
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	async function toggleEnabled(transform: VisualizationTransform) {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${transform.visualization_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: !transform.is_enabled,
					}),
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const updated = await response.json();
			transforms = transforms.map((t) =>
				t.visualization_transform_id === updated.visualization_transform_id ? updated : t
			);

			toastStore.success(
				`Visualization transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle visualization transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/visualization-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({ visualization_transform_id: transformId }),
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger transform: ${response.statusText}`);
			}

			toastStore.success('Visualization transform triggered successfully');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger visualization transform'));
		}
	}

	function openEditForm(transform: VisualizationTransform) {
		editingTransform = transform;
		newTitle = transform.title;
		newEmbeddedDatasetId = transform.embedded_dataset_id;
		// Apply defaults to handle missing fields from older database records
		config = applyDefaults(transform.visualization_config);
		showCreateForm = true;
	}

	function resetForm() {
		newTitle = '';
		newEmbeddedDatasetId = null;
		config = { ...DEFAULT_CONFIG };
		showCreateForm = false;
		editingTransform = null;
		createError = null;
	}

	function requestDeleteTransform(transform: VisualizationTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.visualization_transform_id;
			const response = await fetch(
				`/api/visualization-transforms/${target.visualization_transform_id}`,
				{
					method: 'DELETE',
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to delete visualization transform: ${response.statusText}`);
			}

			transforms = transforms.filter(
				(t) => t.visualization_transform_id !== target.visualization_transform_id
			);
			toastStore.success('Visualization transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete visualization transform'));
		} finally {
			deleting = null;
		}
	}

	onMount(() => {
		fetchTransforms();
		fetchEmbeddedDatasets();
		fetchLLMs();
		
		// Check URL parameters for create action and embedded dataset ID
		const urlParams = new URLSearchParams(window.location.hash.split('?')[1] || '');
		const shouldCreate = urlParams.get('create') === 'true';
		const embeddedDatasetId = urlParams.get('embedded_dataset_id');
		
		if (shouldCreate) {
			showCreateForm = true;
			// Remove the URL parameters after processing
			const cleanHash = window.location.hash.split('?')[0];
			window.history.replaceState(null, '', cleanHash);
		}
		
		if (embeddedDatasetId) {
			newEmbeddedDatasetId = parseInt(embeddedDatasetId, 10);
		}
	});

	let filteredTransforms = $derived(
		transforms.filter((t) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return t.title.toLowerCase().includes(query) || t.owner.toLowerCase().includes(query);
		})
	);

	function getEmbeddedDatasetTitle(embeddedDatasetId: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === embeddedDatasetId);
		return dataset
			? `${dataset.title} (${dataset.embedded_dataset_id})`
			: `Embedded Dataset ${embeddedDatasetId}`;
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Visualization Transforms"
		description="Create visualizations of Embedded Datasets using UMAP dimensionality reduction and HDBSCAN clustering. Visualizations help explore semantic relationships and discover topics in your data."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Visualization Transforms</h1>
		<button
			onclick={() => {
				if (showCreateForm) {
					resetForm();
				} else {
					showCreateForm = true;
				}
			}}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Visualization Transform'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingTransform ? 'Edit Visualization Transform' : 'Create New Visualization Transform'}
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					createTransform();
				}}
			>
				<div class="mb-4">
					<label
						for="transform-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Title
					</label>
					<input
						id="transform-title"
						type="text"
						bind:value={newTitle}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						placeholder="Enter visualization title..."
					/>
				</div>

				{#if !editingTransform}
					<div class="mb-4">
						<label
							for="embedded-dataset-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Embedded Dataset
						</label>
						<select
							id="embedded-dataset-select"
							bind:value={newEmbeddedDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select an embedded dataset...</option>
							{#each embeddedDatasets as dataset (dataset.embedded_dataset_id)}
								<option value={dataset.embedded_dataset_id}>
									{dataset.title}
								</option>
							{/each}
						</select>
					</div>
				{/if}

				<div
					class="mb-6 p-4 bg-blue-50 dark:bg-blue-900/10 border border-blue-200 dark:border-blue-800 rounded-lg"
				>
					<h3 class="text-sm font-semibold text-blue-900 dark:text-blue-300 mb-3">
						UMAP Parameters (Dimensionality Reduction)
					</h3>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="n-neighbors"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								N Neighbors
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Controls local vs global structure. Low values (2-5) focus on fine detail, high values (50-200) capture broader structure. Default: 15'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="n-neighbors"
								type="number"
								bind:value={config.n_neighbors}
								min="2"
								max="200"
								step="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 2-200</p>
						</div>

						<div>
							<label
								for="min-dist"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Distance
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Minimum distance between points in the low dimensional space. Low values (0.0-0.1) create clumpier embeddings good for clustering. Higher values (0.5-0.99) spread points out and preserve global structure. Default: 0.1'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="min-dist"
								type="number"
								bind:value={config.min_dist}
								min="0.0"
								max="0.99"
								step="0.01"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 0.0-1.0</p>
						</div>

						<div>
							<label
								for="metric"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Metric
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Distance metric for measuring similarity. Cosine: best for text/embeddings (angle-based). Euclidean: standard distance. Manhattan: city-block distance. Default: cosine'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<select
								id="metric"
								bind:value={config.metric}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							>
								<option value="cosine">Cosine</option>
								<option value="euclidean">Euclidean</option>
								<option value="manhattan">Manhattan</option>
							</select>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Cosine recommended for embeddings
							</p>
						</div>
					</div>
				</div>

				<div
					class="mb-4 p-4 bg-purple-50 dark:bg-purple-900/10 border border-purple-200 dark:border-purple-800 rounded-lg"
				>
					<h3 class="text-sm font-semibold text-purple-900 dark:text-purple-300 mb-3">
						HDBSCAN Parameters (Clustering)
					</h3>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="min-cluster-size"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Cluster Size
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Minimum number of points required to form a cluster. Larger values create fewer, more significant clusters. Smaller values find more fine-grained clusters but may include noise. Default: 10'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="min-cluster-size"
								type="number"
								bind:value={config.min_cluster_size}
								min="2"
								max="500"
								step="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 2-500</p>
						</div>

						<div>
							<label
								for="min-samples"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Samples
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Controls how conservative the clustering is. Higher values make clusters more conservative (fewer outliers classified as cluster members). Default: 5'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="min-samples"
								type="number"
								bind:value={config.min_samples}
								min="1"
								max="500"
								step="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 1-500 (Default: 5)</p>
						</div>
					</div>
				</div>

				<div
					class="mb-4 p-4 bg-green-50 dark:bg-green-900/10 border border-green-200 dark:border-green-800 rounded-lg"
				>
					<h3 class="text-sm font-semibold text-green-900 dark:text-green-300 mb-3">
						Topic Naming Configuration
					</h3>

					<div class="grid grid-cols-1 gap-4">
						<div>
							<label
								for="topic-naming-llm"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								LLM Model
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Select which LLM to use for generating topic names. The LLM will receive document samples from each cluster and generate descriptive labels.'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<select
								id="topic-naming-llm"
								bind:value={config.topic_naming_llm_id}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							>
								<option value={null}>Select an LLM...</option>
								{#each llms as llm (llm.llm_id)}
									<option value={llm.llm_id}>
										{llm.name} ({llm.provider})
									</option>
								{/each}
							</select>
							{#if llms.length === 0}
								<p class="text-xs text-yellow-600 dark:text-yellow-400 mt-1">
									No LLMs available. Create one in the LLMs section first.
								</p>
							{/if}
						</div>
					</div>
				</div>

				<div
					class="mb-4 p-4 bg-cyan-50 dark:bg-cyan-900/10 border border-cyan-200 dark:border-cyan-800 rounded-lg"
				>
					<h3 class="text-sm font-semibold text-cyan-900 dark:text-cyan-300 mb-3">
						Visualization Settings
					</h3>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="min-fontsize"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Min Font Size
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(e, 'Minimum font size for cluster labels in points. Default: 12')}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="min-fontsize"
								type="number"
								bind:value={config.min_fontsize}
								min="8"
								max="48"
								step="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 8-48</p>
						</div>

						<div>
							<label
								for="max-fontsize"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Max Font Size
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(e, 'Maximum font size for cluster labels in points. Default: 24')}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="max-fontsize"
								type="number"
								bind:value={config.max_fontsize}
								min="8"
								max="48"
								step="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 8-48</p>
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label
								for="font-family"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Font Family
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Font family for labels (e.g., Arial, sans-serif). Default: Arial, sans-serif'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="font-family"
								type="text"
								bind:value={config.font_family}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
						</div>

						<div>
							<label
								for="noise-color"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Noise Point Color
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(e, 'Hex color for unclustered (noise) points. Default: #999999')}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="noise-color"
								type="text"
								bind:value={config.noise_color}
								placeholder="#999999"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
						</div>

						<div>
							<label
								for="label-wrap-width"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Label Wrap Width
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(e, 'Character count before wrapping labels. Default: 16')}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="label-wrap-width"
								type="number"
								bind:value={config.label_wrap_width}
								min="8"
								max="40"
								step="1"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 8-40</p>
						</div>

						<div>
							<label
								for="polygon-alpha"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Polygon Transparency
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Transparency of cluster boundary polygons. 0=invisible, 1=opaque. Default: 0.1'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="polygon-alpha"
								type="number"
								bind:value={config.polygon_alpha}
								min="0.0"
								max="1.0"
								step="0.01"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Range: 0.0-1.0</p>
						</div>

						<div class="col-span-2">
							<label class="flex items-center gap-2">
								<input
									id="darkmode"
									type="checkbox"
									bind:checked={config.darkmode}
									class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
								/>
								<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
									Dark Mode Theme
								</span>
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(e, 'Use dark background theme for the visualization')}
								>
									{@html InfoIcon()}
								</button>
							</label>
						</div>

						<div class="col-span-2">
							<label class="flex items-center gap-2">
								<input
									id="use-medoids"
									type="checkbox"
									bind:checked={config.use_medoids}
									class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
								/>
								<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
									Use Medoids (instead of centroids for cluster positions)
								</span>
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Use actual data points (medoids) instead of calculated centroids for cluster center positions'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
						</div>

						<div class="col-span-2">
							<label class="flex items-center gap-2">
								<input
									id="cluster-boundary-polygons"
									type="checkbox"
									bind:checked={config.cluster_boundary_polygons}
									class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
								/>
								<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
									Draw Cluster Boundary Polygons
								</span>
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Draw alpha-shape boundary lines around clusters to visually separate them'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
						</div>
					</div>
				</div>

				<!-- Advanced Settings (Collapsible) -->
				<details
					class="mb-4 p-4 bg-gray-50 dark:bg-gray-900/10 border border-gray-200 dark:border-gray-700 rounded-lg"
				>
					<summary
						class="text-sm font-semibold text-gray-900 dark:text-gray-300 cursor-pointer mb-3"
					>
						Advanced Visualization Settings (Click to expand)
					</summary>

					<div class="mt-4 space-y-4">
						<!-- Color & Appearance -->
						<div
							class="p-3 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded"
						>
							<h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 mb-2 uppercase">
								Color & Palette
							</h4>
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label
										for="palette-hue-shift"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Palette Hue Shift (degrees)
									</label>
									<input
										id="palette-hue-shift"
										type="number"
										bind:value={config.palette_hue_shift}
										min="-180"
										max="180"
										step="0.1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="palette-hue-radius-dep"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Hue Radius Dependence
									</label>
									<input
										id="palette-hue-radius-dep"
										type="number"
										bind:value={config.palette_hue_radius_dependence}
										min="0"
										max="10"
										step="0.1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="palette-theta-range"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Palette Theta Range (π/16 ≈ 0.196)
									</label>
									<input
										id="palette-theta-range"
										type="number"
										bind:value={config.palette_theta_range}
										min="0"
										max="6.280"
										step="any"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="background-color"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Background Color (hex)
									</label>
									<input
										id="background-color"
										type="text"
										bind:value={config.background_color}
										placeholder="Auto"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div class="col-span-2">
									<label class="flex items-center gap-2">
										<input
											type="checkbox"
											bind:checked={config.color_label_text}
											class="w-3 h-3 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500"
										/>
										<span class="text-xs font-medium text-gray-700 dark:text-gray-300"
											>Color Label Text</span
										>
									</label>
								</div>
								<div class="col-span-2">
									<label class="flex items-center gap-2">
										<input
											type="checkbox"
											bind:checked={config.cvd_safer}
											class="w-3 h-3 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500"
										/>
										<span class="text-xs font-medium text-gray-700 dark:text-gray-300"
											>CVD Safer Palette (Color Vision Deficiency)</span
										>
									</label>
								</div>
							</div>
						</div>

						<!-- Text & Typography -->
						<div
							class="p-3 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded"
						>
							<h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 mb-2 uppercase">
								Text & Typography
							</h4>
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label
										for="title"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Plot Title
									</label>
									<input
										id="title"
										type="text"
										bind:value={config.title}
										placeholder="None"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="sub-title"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Plot Subtitle
									</label>
									<input
										id="sub-title"
										type="text"
										bind:value={config.sub_title}
										placeholder="None"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="title-font-size"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Title Font Size (pt)
									</label>
									<input
										id="title-font-size"
										type="number"
										bind:value={config.title_font_size}
										min="8"
										max="100"
										step="1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="sub-title-font-size"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Subtitle Font Size (pt)
									</label>
									<input
										id="sub-title-font-size"
										type="number"
										bind:value={config.sub_title_font_size}
										min="8"
										max="100"
										step="1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="font-weight"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Font Weight (0-1000)
									</label>
									<input
										id="font-weight"
										type="number"
										bind:value={config.font_weight}
										min="100"
										max="900"
										step="100"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="line-spacing"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Line Spacing
									</label>
									<input
										id="line-spacing"
										type="number"
										bind:value={config.line_spacing}
										min="0.1"
										max="3"
										step="0.05"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="text-outline-width"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Outline Width
									</label>
									<input
										id="text-outline-width"
										type="number"
										bind:value={config.text_outline_width}
										min="0"
										max="20"
										step="1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="text-outline-color"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Outline Color
									</label>
									<input
										id="text-outline-color"
										type="text"
										bind:value={config.text_outline_color}
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="text-collision-size-scale"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Collision Size Scale
									</label>
									<input
										id="text-collision-size-scale"
										type="number"
										bind:value={config.text_collision_size_scale}
										min="0.1"
										max="10"
										step="0.1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="text-min-pixel-size"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Min Pixel Size
									</label>
									<input
										id="text-min-pixel-size"
										type="number"
										bind:value={config.text_min_pixel_size}
										min="6"
										max="100"
										step="0.5"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="text-max-pixel-size"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Max Pixel Size
									</label>
									<input
										id="text-max-pixel-size"
										type="number"
										bind:value={config.text_max_pixel_size}
										min="12"
										max="100"
										step="0.5"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
							</div>
						</div>

						<!-- Tooltips -->
						<div
							class="p-3 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded"
						>
							<h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 mb-2 uppercase">
								Tooltip Settings
							</h4>
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label
										for="tooltip-font-family"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Tooltip Font Family
									</label>
									<input
										id="tooltip-font-family"
										type="text"
										bind:value={config.tooltip_font_family}
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="tooltip-font-weight"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Tooltip Font Weight
									</label>
									<input
										id="tooltip-font-weight"
										type="number"
										bind:value={config.tooltip_font_weight}
										min="100"
										max="900"
										step="100"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
							</div>
						</div>

						<!-- Points & Markers -->
						<div
							class="p-3 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded"
						>
							<h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 mb-2 uppercase">
								Point & Marker Settings
							</h4>
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label
										for="point-hover-color"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Hover Color
									</label>
									<input
										id="point-hover-color"
										type="text"
										bind:value={config.point_hover_color}
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-size-scale"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Size Scale
									</label>
									<input
										id="point-size-scale"
										type="number"
										bind:value={config.point_size_scale}
										step="0.1"
										placeholder="Auto (leave empty for automatic)"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-radius-min-pixels"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Radius Min (px)
									</label>
									<input
										id="point-radius-min-pixels"
										type="number"
										bind:value={config.point_radius_min_pixels}
										min="0.001"
										max="100"
										step="any"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-radius-max-pixels"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Radius Max (px)
									</label>
									<input
										id="point-radius-max-pixels"
										type="number"
										bind:value={config.point_radius_max_pixels}
										min="0.1"
										max="100"
										step="0.1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-line-width"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Line Width
									</label>
									<input
										id="point-line-width"
										type="number"
										bind:value={config.point_line_width}
										min="0.0"
										max="5"
										step="any"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-line-width-min"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Line Width Min (px)
									</label>
									<input
										id="point-line-width-min"
										type="number"
										bind:value={config.point_line_width_min_pixels}
										min="0.0"
										max="5"
										step="any"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-line-width-max"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Line Width Max (px)
									</label>
									<input
										id="point-line-width-max"
										type="number"
										bind:value={config.point_line_width_max_pixels}
										min="0.0"
										max="5"
										step="any"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="cluster-boundary-line-width"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Cluster Boundary Line Width
									</label>
									<input
										id="cluster-boundary-line-width"
										type="number"
										bind:value={config.cluster_boundary_line_width}
										min="0.0"
										max="5"
										step="0.1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
							</div>
						</div>

						<!-- Layout & Display -->
						<div
							class="p-3 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded"
						>
							<h4 class="text-xs font-semibold text-gray-700 dark:text-gray-300 mb-2 uppercase">
								Layout & Display
							</h4>
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label
										for="width"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Width
									</label>
									<input
										id="width"
										type="text"
										bind:value={config.width}
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="height"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Height (px)
									</label>
									<input
										id="height"
										type="number"
										bind:value={config.height}
										min="400"
										max="5000"
										step="50"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="initial-zoom-fraction"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Initial Zoom Fraction
									</label>
									<input
										id="initial-zoom-fraction"
										type="number"
										bind:value={config.initial_zoom_fraction}
										min="0.1"
										max="3"
										step="0.05"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="noise-label"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Noise Label
									</label>
									<input
										id="noise-label"
										type="text"
										bind:value={config.noise_label}
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="logo"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Logo URL
									</label>
									<input
										id="logo"
										type="text"
										bind:value={config.logo}
										placeholder="None"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="logo-width"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Logo Width (px)
									</label>
									<input
										id="logo-width"
										type="number"
										bind:value={config.logo_width}
										min="50"
										max="1000"
										step="1"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="background-image"
										class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Background Image URL
									</label>
									<input
										id="background-image"
										type="text"
										bind:value={config.background_image}
										placeholder="None"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div class="col-span-2">
									<label class="flex items-center gap-2">
										<input
											type="checkbox"
											bind:checked={config.inline_data}
											class="w-3 h-3 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500"
										/>
										<span class="text-xs font-medium text-gray-700 dark:text-gray-300"
											>Inline Data (embed data in HTML vs separate files)</span
										>
									</label>
								</div>
								<div class="col-span-2">
									<label class="flex items-center gap-2">
										<input
											type="checkbox"
											bind:checked={config.enable_topic_tree}
											class="w-3 h-3 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500"
										/>
										<span class="text-xs font-medium text-gray-700 dark:text-gray-300"
											>Enable Topic Tree</span
										>
									</label>
								</div>
							</div>
						</div>
					</div>
				</details>

				{#if createError}
					<div
						class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg"
					>
						<p class="text-sm text-red-600 dark:text-red-400">{createError}</p>
					</div>
				{/if}

				<div class="flex gap-3">
					<button
						type="submit"
						disabled={creating}
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{creating
							? editingTransform
								? 'Updating...'
								: 'Creating...'
							: editingTransform
								? 'Update Transform'
								: 'Create Transform'}
					</button>
					<button
						type="button"
						onclick={resetForm}
						class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
			placeholder="Search visualization transforms..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading visualization transforms...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if filteredTransforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No visualization transforms found matching your search.'
					: 'No visualization transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredTransforms as transform (transform.visualization_transform_id)}
				{@const stats = statsMap.get(transform.visualization_transform_id)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">
								{transform.title}
							</h3>
							<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
								<p>
									<strong>Embedded Dataset:</strong>
									{getEmbeddedDatasetTitle(transform.embedded_dataset_id)}
								</p>
								<p><strong>Owner:</strong> {transform.owner}</p>
								<p>
									<strong>Status:</strong>
									<span
										class={transform.is_enabled
											? 'text-green-600 dark:text-green-400'
											: 'text-gray-500 dark:text-gray-400'}
									>
										{transform.is_enabled ? 'Enabled' : 'Disabled'}
									</span>
								</p>
								{#if transform.reduced_collection_name}
									<p>
										<strong>3D Points Collection:</strong>
										<code class="px-2 py-1 bg-gray-100 dark:bg-gray-700 rounded text-xs font-mono">
											{transform.reduced_collection_name}
										</code>
									</p>
								{/if}
								{#if transform.topics_collection_name}
									<p>
										<strong>Topics Collection:</strong>
										<code class="px-2 py-1 bg-gray-100 dark:bg-gray-700 rounded text-xs font-mono">
											{transform.topics_collection_name}
										</code>
									</p>
								{/if}
							</div>
						</div>
						<div class="flex flex-col gap-2">
							<button
								onclick={() => toggleEnabled(transform)}
								class="px-3 py-1 text-sm rounded-lg {transform.is_enabled
									? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400'
									: 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400'}"
							>
								{transform.is_enabled ? 'Disable' : 'Enable'}
							</button>
							<button
								onclick={() => triggerTransform(transform.visualization_transform_id)}
								class="px-3 py-1 text-sm bg-blue-100 text-blue-700 hover:bg-blue-200 rounded-lg dark:bg-blue-900/20 dark:text-blue-400"
							>
								Trigger
							</button>
							<button
								onclick={() => openEditForm(transform)}
								class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300"
							>
								Edit
							</button>
							<button
								onclick={() => requestDeleteTransform(transform)}
								disabled={deleting === transform.visualization_transform_id}
								class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{deleting === transform.visualization_transform_id ? 'Deleting...' : 'Delete'}
							</button>
						</div>
					</div>

					<div
						class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-2 gap-4"
					>
						<div>
							<p class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">UMAP Config</p>
							<div class="text-xs text-gray-600 dark:text-gray-400 space-y-1">
								<p>Neighbors: {transform.visualization_config.n_neighbors}</p>
								<p>Min Distance: {transform.visualization_config.min_dist}</p>
								<p>Metric: {transform.visualization_config.metric}</p>
							</div>
						</div>
						<div>
							<p class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
								HDBSCAN Config
							</p>
							<div class="text-xs text-gray-600 dark:text-gray-400 space-y-1">
								<p>Min Cluster Size: {transform.visualization_config.min_cluster_size}</p>
								<p>
									Min Samples: {transform.visualization_config.min_samples ?? 'Auto'}
								</p>
							</div>
						</div>
					</div>

					{#if stats}
						<div
							class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-3 gap-4"
						>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Total Points</p>
								<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
									{stats.total_points}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Clusters</p>
								<p class="text-lg font-semibold text-purple-600 dark:text-purple-400">
									{stats.total_clusters}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Noise Points</p>
								<p class="text-lg font-semibold text-gray-600 dark:text-gray-400">
									{stats.noise_points}
								</p>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete Visualization Transform"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>
