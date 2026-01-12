<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import type { VisualizationConfig, VisualizationTransform } from '../types/visualizations';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		// eslint-disable-next-line no-unused-vars
		onViewTransform?: (id: number) => void;
	}

	let { onViewTransform }: Props = $props();

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
	let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

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

	// Pagination and sort state
	let totalCount = $state(0);
	let currentPage = $state(1);
	let pageSize = $state(10);
	const pageSizeOptions = [10, 50, 100];
	let sortBy = $state('created_at');
	let sortDirection = $state('desc');

	let config = $state<VisualizationConfig>({ ...DEFAULT_CONFIG });
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let transformPendingDelete = $state<VisualizationTransform | null>(null);

	// Selection state
	// eslint-disable-next-line svelte/no-unnecessary-state-wrap
	let selected = $state(new SvelteSet<number>());
	let selectAll = $state(false);

	function toggleSelectAll() {
		if (selectAll) {
			selected.clear();
			for (const t of transforms) {
				selected.add(t.visualization_transform_id);
			}
		} else {
			selected.clear();
		}
	}

	function toggleSelect(id: number) {
		if (selected.has(id)) {
			selected.delete(id);
			selectAll = false;
		} else {
			selected.add(id);
		}
	}

	async function bulkToggleEnabled(_enable: boolean) {
		for (const id of selected) {
			const transform = transforms.find((t) => t.visualization_transform_id === id);
			if (transform) {
				transform.is_enabled = _enable;
				await toggleEnabled(transform, false);
			}
		}
		selected.clear();
		selectAll = false;
	}

	async function bulkTrigger() {
		for (const id of selected) {
			await triggerTransform(id);
		}
		selected.clear();
		selectAll = false;
	}

	async function bulkDelete() {
		for (const id of selected) {
			const transform = transforms.find((t) => t.visualization_transform_id === id);
			if (transform) {
				await requestDeleteTransform(transform, false);
			}
		}
		selected.clear();
		selectAll = false;
	}

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
			const offset = (currentPage - 1) * pageSize;
			const params = new SvelteURLSearchParams({
				limit: pageSize.toString(),
				offset: offset.toString(),
				sort_by: sortBy,
				sort_direction: sortDirection,
			});
			if (searchQuery.trim()) {
				params.append('search', searchQuery.trim());
			}
			const response = await fetch(`/api/visualization-transforms?${params}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch visualization transforms: ${response.statusText}`);
			}
			const data = await response.json();
			const rawTransforms = Array.isArray(data) ? data : data.items || [];
			totalCount = data.total_count ?? (Array.isArray(data) ? data.length : rawTransforms.length);

			transforms = rawTransforms.map((t: VisualizationTransform) => ({
				...t,
				visualization_config: applyDefaults(t.visualization_config),
			}));

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

	function handleSort(field: string) {
		if (sortBy === field) {
			sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
		} else {
			sortBy = field;
			sortDirection = 'desc';
		}
		currentPage = 1;
		fetchTransforms();
	}

	function handleSearchInput() {
		if (searchDebounceTimer) {
			clearTimeout(searchDebounceTimer);
		}
		searchDebounceTimer = setTimeout(() => {
			currentPage = 1;
			fetchTransforms();
		}, 300);
	}

	function handlePageChange(newPage: number) {
		currentPage = newPage;
		fetchTransforms();
	}

	function handlePageSizeChange(newSize: number) {
		pageSize = newSize;
		currentPage = 1;
		fetchTransforms();
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
				resetForm();
			} else {
				transforms = [...transforms, savedTransform];
				toastStore.success('Visualization transform created successfully');
				resetForm();
				// Redirect to the Visualizations page to monitor progress
				window.location.hash = '#/visualizations';
			}
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

	async function toggleEnabled(transform: VisualizationTransform, refresh = true) {
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
			if (refresh) {
				await fetchTransforms();
			}
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

	function requestDeleteTransform(transform: VisualizationTransform, refresh = true) {
		transformPendingDelete = transform;
		(transformPendingDelete as any)._skipRefresh = !refresh;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		const skipRefresh = (target as any)._skipRefresh;
		transformPendingDelete = null;

		try {
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
			if (!skipRefresh) {
				await fetchTransforms();
			}
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete visualization transform'));
		}
	}

	function connectSSE() {
		// Close existing connection
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}

		try {
			eventSource = new EventSource('/api/visualization-transforms/stream');

			eventSource.addEventListener('connected', () => {
				reconnectAttempts = 0;
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// Handle status update - refresh specific transform or trigger refetch
					if (statusUpdate.visualization_transform_id) {
						// Refresh stats for the specific transform
						fetchStatsForTransform(statusUpdate.visualization_transform_id);
					}
				} catch (e) {
					console.error('Failed to parse SSE status event:', e);
				}
			});

			eventSource.addEventListener('closed', () => {
				reconnectSSE();
			});

			eventSource.onerror = () => {
				eventSource?.close();
				eventSource = null;
				reconnectSSE();
			};
		} catch (e) {
			console.error('Failed to connect to SSE stream:', e);
			reconnectSSE();
		}
	}

	function reconnectSSE() {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
		}

		if (reconnectAttempts >= maxReconnectAttempts) {
			console.error('Max SSE reconnection attempts reached');
			return;
		}

		// Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 64s... up to 60s max
		const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 60000);
		reconnectAttempts++;

		reconnectTimer = setTimeout(() => {
			connectSSE();
		}, delay);
	}

	function disconnectSSE() {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}
		if (eventSource) {
			eventSource.close();
			eventSource = null;
		}
		reconnectAttempts = 0;
	}

	onMount(() => {
		fetchTransforms();
		fetchEmbeddedDatasets();
		fetchLLMs();

		// Connect to SSE stream for real-time updates
		connectSSE();

		// Check URL parameters for create action, edit action, and embedded dataset ID
		const urlParams = new SvelteURLSearchParams(window.location.hash.split('?')[1] || '');
		const shouldCreate = urlParams.get('create') === 'true';
		const embeddedDatasetId = urlParams.get('embedded_dataset_id');
		const editTransformId = urlParams.get('edit');

		if (shouldCreate) {
			showCreateForm = true;
			// Remove the URL parameters after processing
			const cleanHash = window.location.hash.split('?')[0];
			window.history.replaceState(null, '', cleanHash);
		}

		if (embeddedDatasetId) {
			newEmbeddedDatasetId = parseInt(embeddedDatasetId, 10);
		}

		// Handle edit parameter - open edit form for specified transform
		if (editTransformId) {
			const transformId = parseInt(editTransformId, 10);
			// Wait for transforms to load, then open edit form
			const checkAndOpenEdit = async () => {
				// Wait a bit for transforms to load
				await new Promise((resolve) => setTimeout(resolve, 500));
				const transform = transforms.find((t) => t.visualization_transform_id === transformId);
				if (transform) {
					openEditForm(transform);
				} else {
					// If not in current page, fetch the specific transform
					try {
						const response = await fetch(`/api/visualization-transforms/${transformId}`);
						if (response.ok) {
							const fetchedTransform = await response.json();
							openEditForm(fetchedTransform);
						}
					} catch (e) {
						console.error('Failed to fetch transform for editing:', e);
					}
				}
			};
			checkAndOpenEdit();
			// Remove the URL parameters after processing
			const cleanHash = window.location.hash.split('?')[0];
			window.history.replaceState(null, '', cleanHash);
		}
	});

	onDestroy(() => {
		disconnectSSE();
	});

	function getEmbeddedDatasetTitle(embeddedDatasetId: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === embeddedDatasetId);
		return dataset ? `${dataset.title}` : `Embedded Dataset ${embeddedDatasetId}`;
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
			oninput={handleSearchInput}
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
	{:else if transforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No visualization transforms found matching your search.'
					: 'No visualization transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		{#if selected.size > 0}
			<div
				class="mb-4 flex items-center gap-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4"
			>
				<span class="text-sm text-blue-700 dark:text-blue-300 flex-1">
					{selected.size} transform{selected.size !== 1 ? 's' : ''} selected
				</span>
				<button
					onclick={() => bulkToggleEnabled(true)}
					class="text-sm px-3 py-1 rounded bg-green-600 hover:bg-green-700 text-white transition-colors"
				>
					Enable
				</button>
				<button
					onclick={() => bulkToggleEnabled(false)}
					class="text-sm px-3 py-1 rounded bg-yellow-600 hover:bg-yellow-700 text-white transition-colors"
				>
					Disable
				</button>
				<button
					onclick={() => bulkTrigger()}
					class="text-sm px-3 py-1 rounded bg-blue-600 hover:bg-blue-700 text-white transition-colors"
				>
					Trigger
				</button>
				<button
					onclick={() => bulkDelete()}
					class="text-sm px-3 py-1 rounded bg-red-600 hover:bg-red-700 text-white transition-colors"
				>
					Delete
				</button>
				<button
					onclick={() => {
						selected = new SvelteSet();
						selectAll = false;
					}}
					class="text-sm px-3 py-1 rounded bg-gray-300 hover:bg-gray-400 dark:bg-gray-600 dark:hover:bg-gray-500 text-gray-900 dark:text-white transition-colors"
				>
					Clear
				</button>
			</div>
		{/if}
		<div class="overflow-x-auto">
			<table
				class="visualization-transforms-table w-full text-sm text-left text-gray-600 dark:text-gray-400"
			>
				<thead class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
					<tr>
						<th class="px-4 py-3 w-12">
							<input
								type="checkbox"
								checked={selectAll}
								onchange={() => toggleSelectAll()}
								class="cursor-pointer"
							/>
						</th>
						<th class="px-4 py-3">
							<button
								type="button"
								onclick={() => handleSort('title')}
								class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
							>
								Title
								{#if sortBy === 'title'}
									{sortDirection === 'asc' ? '▲' : '▼'}
								{/if}
							</button>
						</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Embedded Dataset</th>
						<th class="px-4 py-3">
							<button
								type="button"
								onclick={() => handleSort('is_enabled')}
								class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
							>
								Status
								{#if sortBy === 'is_enabled'}
									{sortDirection === 'asc' ? '▲' : '▼'}
								{/if}
							</button>
						</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Points</th>
						<th class="px-4 py-3 font-semibold text-gray-900 dark:text-white">Clusters</th>
						<th class="px-4 py-3">
							<button
								type="button"
								onclick={() => handleSort('created_at')}
								class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
							>
								Created
								{#if sortBy === 'created_at'}
									{sortDirection === 'asc' ? '▲' : '▼'}
								{/if}
							</button>
						</th>
						<th class="px-4 py-3 w-12 text-center font-semibold text-gray-900 dark:text-white"
							>Edit</th
						>
					</tr>
				</thead>
				<tbody>
					{#each transforms as transform (transform.visualization_transform_id)}
						{@const stats = statsMap.get(transform.visualization_transform_id)}
						<tr
							class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
						>
							<td class="px-4 py-3 w-12">
								<input
									type="checkbox"
									checked={selected.has(transform.visualization_transform_id)}
									onchange={() => toggleSelect(transform.visualization_transform_id)}
									class="cursor-pointer"
								/>
							</td>
							<td class="px-4 py-3 font-medium text-gray-900 dark:text-white">
								{#if onViewTransform}
									<button
										onclick={() => onViewTransform(transform.visualization_transform_id)}
										class="text-blue-600 dark:text-blue-400 hover:underline"
									>
										{transform.title}
									</button>
								{:else}
									{transform.title}
								{/if}
							</td>
							<td class="px-4 py-3 text-sm">
								<a
									href="#/embedded-datasets/{transform.embedded_dataset_id}/details"
									class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
								>
									{getEmbeddedDatasetTitle(transform.embedded_dataset_id)}
								</a>
							</td>
							<td class="px-4 py-3">
								<span
									class={transform.is_enabled
										? 'px-2 py-1 rounded-full text-xs font-semibold bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400'
										: 'px-2 py-1 rounded-full text-xs font-semibold bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-400'}
								>
									{transform.is_enabled ? 'Enabled' : 'Disabled'}
								</span>
							</td>
							<td class="px-4 py-3">{stats?.total_points ?? '-'}</td>
							<td class="px-4 py-3">{stats?.total_clusters ?? '-'}</td>
							<td class="px-4 py-3">{new Date(transform.created_at).toLocaleDateString()}</td>
							<td class="px-4 py-3 text-center">
								<button
									type="button"
									onclick={() => openEditForm(transform)}
									title="Edit"
									class="text-gray-600 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-300 transition-colors"
								>
									✎
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<div class="mt-6 flex items-center justify-between">
			<div class="flex items-center">
				<div class="text-sm text-gray-600 dark:text-gray-400">
					Showing {(currentPage - 1) * pageSize + 1} to {Math.min(
						currentPage * pageSize,
						totalCount
					)} of
					{totalCount} transforms
				</div>
				<div class="ml-4 flex items-center gap-2">
					<label for="page-size" class="text-sm font-medium text-gray-700 dark:text-gray-300">
						Per page:
					</label>
					<select
						id="page-size"
						value={pageSize}
						onchange={(e) => handlePageSizeChange(Number(e.currentTarget.value))}
						class="px-2 py-1 pr-8 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-1 focus:ring-blue-500"
					>
						{#each pageSizeOptions as size (size)}
							<option value={size}>{size}</option>
						{/each}
					</select>
				</div>
			</div>
			<div class="flex gap-2">
				<button
					type="button"
					onclick={() => handlePageChange(currentPage - 1)}
					disabled={currentPage === 1}
					class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					Previous
				</button>
				<div class="flex items-center gap-1">
					{#each Array.from({ length: Math.ceil(totalCount / pageSize) }, (_, i) => i + 1) as page (page)}
						{#if page === 1 || page === Math.ceil(totalCount / pageSize) || (page >= currentPage - 1 && page <= currentPage + 1)}
							<button
								type="button"
								onclick={() => handlePageChange(page)}
								class={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
									currentPage === page
										? 'bg-blue-600 text-white'
										: 'border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
								}`}
							>
								{page}
							</button>
						{:else if page === currentPage - 2 || page === currentPage + 2}
							<span class="px-2 py-2 text-gray-500">...</span>
						{/if}
					{/each}
				</div>
				<button
					type="button"
					onclick={() => handlePageChange(currentPage + 1)}
					disabled={currentPage >= Math.ceil(totalCount / pageSize)}
					class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					Next
				</button>
			</div>
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

<style>
	:global(.visualization-transforms-table :is(td, th)) {
		word-wrap: break-word;
		word-break: normal;
		white-space: normal;
		overflow-wrap: break-word;
	}
</style>
