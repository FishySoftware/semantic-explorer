<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { InfoCircleSolid } from 'flowbite-svelte-icons';
	import { onDestroy, onMount } from 'svelte';
	import { SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import type {
		EmbeddedDataset,
		LLM,
		PaginatedEmbeddedDatasetList,
		PaginatedResponse,
		VisualizationStats as Stats,
		VisualizationConfig,
		VisualizationTransform,
	} from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate, showTooltip } from '../utils/ui-helpers';

	interface Props {
		// eslint-disable-next-line no-unused-vars
		onViewTransform?: (id: number) => void;
	}

	let { onViewTransform }: Props = $props();

	let transforms = $state<VisualizationTransform[]>([]);
	let embeddedDatasets = $state<EmbeddedDataset[]>([]);
	let llms = $state<LLM[]>([]);
	let statsMap = $state<Record<number, Stats>>({});
	let loading = $state(true);
	let error = $state<string | null>(null);

	let searchQuery = $state('');
	let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

	// SSE connection state
	let eventSource: EventSource | null = null;
	let reconnectAttempts = 0;
	let maxReconnectAttempts = 10;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let isMounted = false; // Track if component is still mounted

	let showCreateForm = $state(false);
	let editingTransform = $state<VisualizationTransform | null>(null);
	let newTitle = $state('');
	let selectedEmbeddedDatasetIds = new SvelteSet<number>();

	// Default configuration values - must match backend defaults in crates/core/src/models.rs
	const DEFAULT_CONFIG: VisualizationConfig = {
		n_neighbors: 15,
		min_dist: 0.1,
		metric: 'cosine',
		min_cluster_size: 15,
		min_samples: 5,
		topic_naming_llm_id: null,
		topic_naming_prompt:
			'These are representative texts from a document cluster:\n\n{{samples}}\n\nProvide a short, concise topic name (2-4 words) that captures the main theme. Respond with ONLY the topic name, nothing else.',
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
		polygon_alpha: 0.5,
		cvd_safer: false,
		// Datamapplot render_html parameters
		title: null,
		sub_title: null,
		title_font_size: 36,
		sub_title_font_size: 18,
		text_collision_size_scale: 3.0,
		text_min_pixel_size: 12.0,
		text_max_pixel_size: 36.0,
		font_family: 'Playfair Display SC',
		font_weight: 600,
		tooltip_font_family: 'Playfair Display SC',
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

	// Round a number to avoid f32 precision issues (e.g., 0.10000000149011612 -> 0.1)
	// Uses 6 decimal places which is sufficient for f32 precision
	function roundFloat(value: number | null | undefined): number | null {
		if (value === null || value === undefined) return null;
		// Round to 6 decimal places to clean up f32 precision artifacts
		return Math.round(value * 1000000) / 1000000;
	}

	// Merge loaded config with defaults to handle missing fields from older records
	// Also normalizes float values to avoid f32 precision issues from backend
	function applyDefaults(loadedConfig: Partial<VisualizationConfig>): VisualizationConfig {
		const merged = { ...DEFAULT_CONFIG, ...loadedConfig };

		// Normalize float fields to clean up f32 precision artifacts
		merged.min_dist = roundFloat(merged.min_dist) ?? DEFAULT_CONFIG.min_dist;
		merged.palette_hue_shift =
			roundFloat(merged.palette_hue_shift) ?? DEFAULT_CONFIG.palette_hue_shift;
		merged.palette_hue_radius_dependence =
			roundFloat(merged.palette_hue_radius_dependence) ??
			DEFAULT_CONFIG.palette_hue_radius_dependence;
		merged.palette_theta_range =
			roundFloat(merged.palette_theta_range) ?? DEFAULT_CONFIG.palette_theta_range;
		merged.polygon_alpha = roundFloat(merged.polygon_alpha) ?? DEFAULT_CONFIG.polygon_alpha;
		merged.text_collision_size_scale =
			roundFloat(merged.text_collision_size_scale) ?? DEFAULT_CONFIG.text_collision_size_scale;
		merged.text_min_pixel_size =
			roundFloat(merged.text_min_pixel_size) ?? DEFAULT_CONFIG.text_min_pixel_size;
		merged.text_max_pixel_size =
			roundFloat(merged.text_max_pixel_size) ?? DEFAULT_CONFIG.text_max_pixel_size;
		merged.line_spacing = roundFloat(merged.line_spacing) ?? DEFAULT_CONFIG.line_spacing;
		merged.point_radius_min_pixels =
			roundFloat(merged.point_radius_min_pixels) ?? DEFAULT_CONFIG.point_radius_min_pixels;
		merged.point_radius_max_pixels =
			roundFloat(merged.point_radius_max_pixels) ?? DEFAULT_CONFIG.point_radius_max_pixels;
		merged.point_line_width_min_pixels =
			roundFloat(merged.point_line_width_min_pixels) ?? DEFAULT_CONFIG.point_line_width_min_pixels;
		merged.point_line_width_max_pixels =
			roundFloat(merged.point_line_width_max_pixels) ?? DEFAULT_CONFIG.point_line_width_max_pixels;
		merged.point_line_width =
			roundFloat(merged.point_line_width) ?? DEFAULT_CONFIG.point_line_width;
		merged.cluster_boundary_line_width =
			roundFloat(merged.cluster_boundary_line_width) ?? DEFAULT_CONFIG.cluster_boundary_line_width;
		merged.initial_zoom_fraction =
			roundFloat(merged.initial_zoom_fraction) ?? DEFAULT_CONFIG.initial_zoom_fraction;
		merged.point_size_scale = roundFloat(merged.point_size_scale);

		return merged;
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
	let transformsPendingBulkDelete = $state<VisualizationTransform[]>([]);

	// Selection state
	// eslint-disable-next-line svelte/no-unnecessary-state-wrap
	let selected = $state(new SvelteSet<number>());
	let selectAll = $state(false);

	function toggleSelectAll() {
		selectAll = !selectAll;
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
				await toggleEnabled(transform, _enable, false);
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
		const toDelete: VisualizationTransform[] = [];
		for (const id of selected) {
			const transform = transforms.find((t) => t.visualization_transform_id === id);
			if (transform) {
				toDelete.push(transform);
			}
		}
		if (toDelete.length > 0) {
			transformsPendingBulkDelete = toDelete;
		}
	}

	async function confirmBulkDeleteTransforms() {
		const toDelete = transformsPendingBulkDelete;
		transformsPendingBulkDelete = [];

		for (const transform of toDelete) {
			try {
				const response = await fetch(
					`/api/visualization-transforms/${transform.visualization_transform_id}`,
					{
						method: 'DELETE',
					}
				);

				if (!response.ok) {
					throw new Error(`Failed to delete visualization transform: ${response.statusText}`);
				}

				transforms = transforms.filter(
					(t) => t.visualization_transform_id !== transform.visualization_transform_id
				);
			} catch (e) {
				toastStore.error(formatError(e, `Failed to delete transform "${transform.title}"`));
			}
		}

		selected.clear();
		selectAll = false;
		toastStore.success(`Deleted ${toDelete.length} transform${toDelete.length !== 1 ? 's' : ''}`);
		await fetchTransforms();
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
			const data = (await response.json()) as PaginatedResponse<VisualizationTransform>;
			const rawTransforms = data.items;
			totalCount = data.total_count;

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
				statsMap = { ...statsMap, [transformId]: stats };
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function fetchEmbeddedDatasets() {
		try {
			const response = await fetch('/api/embedded-datasets?limit=100&offset=0');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedded datasets: ${response.statusText}`);
			}
			const data: PaginatedEmbeddedDatasetList = await response.json();
			embeddedDatasets = data.embedded_datasets;
		} catch (e) {
			console.error('Failed to fetch embedded datasets:', e);
		}
	}

	async function fetchLLMs() {
		try {
			const response = await fetch('/api/llms?limit=10&offset=0');
			if (!response.ok) {
				throw new Error(`Failed to fetch LLMs: ${response.statusText}`);
			}
			const data: PaginatedResponse<LLM> = await response.json();
			llms = data.items;
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

		if (editingTransform) {
			// Update existing transform
			try {
				creating = true;
				createError = null;

				const response = await fetch(
					`/api/visualization-transforms/${editingTransform.visualization_transform_id}`,
					{
						method: 'PATCH',
						headers: { 'Content-Type': 'application/json' },
						body: JSON.stringify({
							title: newTitle,
							visualization_config: config,
						}),
					}
				);

				if (!response.ok) {
					throw new Error(`Failed to update visualization transform: ${response.statusText}`);
				}

				const savedTransform = await response.json();
				transforms = transforms.map((t) =>
					t.visualization_transform_id === savedTransform.visualization_transform_id
						? savedTransform
						: t
				);
				toastStore.success('Visualization transform updated successfully');
				resetForm();
			} catch (e) {
				const message = formatError(e, 'Failed to update visualization transform');
				createError = message;
				toastStore.error(message);
			} finally {
				creating = false;
			}
			return;
		}

		// Create new transforms - one per selected embedded dataset
		if (selectedEmbeddedDatasetIds.size === 0) {
			createError = 'At least one Embedded Dataset must be selected';
			return;
		}

		try {
			creating = true;
			createError = null;

			const datasetIds = Array.from(selectedEmbeddedDatasetIds);
			const useSuffix = datasetIds.length > 1;
			let createdCount = 0;
			let lastError: string | null = null;

			for (const embeddedDatasetId of datasetIds) {
				const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === embeddedDatasetId);
				const suffix = useSuffix && dataset ? ` (${dataset.title})` : '';

				const body = {
					title: `${newTitle}${suffix}`,
					embedded_dataset_id: embeddedDatasetId,
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

				try {
					const response = await fetch('/api/visualization-transforms', {
						method: 'POST',
						headers: { 'Content-Type': 'application/json' },
						body: JSON.stringify(body),
					});

					if (!response.ok) {
						throw new Error(`Failed to create visualization transform: ${response.statusText}`);
					}

					const savedTransform = await response.json();
					transforms = [...transforms, savedTransform];
					createdCount++;
				} catch (e) {
					lastError = formatError(e, `Failed to create transform for dataset ${embeddedDatasetId}`);
					toastStore.error(lastError);
				}
			}

			if (createdCount > 0) {
				toastStore.success(
					`Created ${createdCount} visualization transform${createdCount !== 1 ? 's' : ''} successfully`
				);
				resetForm();
				window.location.hash = '#/visualizations';
			} else if (lastError) {
				createError = lastError;
			}
		} catch (e) {
			const message = formatError(e, 'Failed to create visualization transforms');
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	async function toggleEnabled(
		transform: VisualizationTransform,
		targetState: boolean,
		refresh = true
	) {
		try {
			const response = await fetch(
				`/api/visualization-transforms/${transform.visualization_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: targetState,
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
		selectedEmbeddedDatasetIds.clear();
		selectedEmbeddedDatasetIds.add(transform.embedded_dataset_id);
		// Apply defaults to handle missing fields from older database records
		config = applyDefaults(transform.visualization_config);
		showCreateForm = true;
	}

	function resetForm() {
		newTitle = '';
		selectedEmbeddedDatasetIds.clear();
		config = { ...DEFAULT_CONFIG };
		showCreateForm = false;
		editingTransform = null;
		createError = null;
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
		disconnectSSE();

		try {
			eventSource = new EventSource('/api/visualization-transforms/stream');

			eventSource.addEventListener('connected', () => {
				reconnectAttempts = 0;
			});

			eventSource.addEventListener('status', (event) => {
				try {
					const statusUpdate = JSON.parse(event.data);
					// Handle status update - refresh specific transform or trigger refetch
					// API sends transform_id (generic) not visualization_transform_id
					if (statusUpdate.transform_id) {
						// Refresh stats for the specific transform
						fetchStatsForTransform(statusUpdate.transform_id);
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
		if (!isMounted) {
			// Component has been unmounted, don't attempt reconnection
			return;
		}

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
			if (isMounted) {
				connectSSE();
			}
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
		isMounted = true;
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
			selectedEmbeddedDatasetIds.add(parseInt(embeddedDatasetId, 10));
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
		isMounted = false;
		disconnectSSE();
	});

	function getEmbeddedDatasetTitle(embeddedDatasetId: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === embeddedDatasetId);
		return dataset ? `${dataset.title}` : `Embedded Dataset ${embeddedDatasetId}`;
	}
</script>

<div class="mx-auto">
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
					<fieldset class="mb-4">
						<legend class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
							Embedded Datasets
							{#if selectedEmbeddedDatasetIds.size > 0}
								<span class="ml-2 text-xs font-normal text-blue-600 dark:text-blue-400">
									{selectedEmbeddedDatasetIds.size} selected — will create {selectedEmbeddedDatasetIds.size}
									visualization{selectedEmbeddedDatasetIds.size !== 1 ? 's' : ''}
								</span>
							{/if}
						</legend>
						<div
							class="max-h-48 overflow-y-auto border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 p-2 space-y-1"
						>
							{#if embeddedDatasets.length === 0}
								<p class="text-sm text-gray-500 dark:text-gray-400 px-2 py-1">
									No embedded datasets available
								</p>
							{:else}
								<div
									class="flex items-center gap-2 px-2 py-1 border-b border-gray-200 dark:border-gray-600 mb-1"
								>
									<button
										type="button"
										class="text-xs text-blue-600 dark:text-blue-400 hover:underline"
										onclick={() => {
											if (selectedEmbeddedDatasetIds.size === embeddedDatasets.length) {
												selectedEmbeddedDatasetIds.clear();
											} else {
												for (const d of embeddedDatasets) {
													selectedEmbeddedDatasetIds.add(d.embedded_dataset_id);
												}
											}
										}}
									>
										{selectedEmbeddedDatasetIds.size === embeddedDatasets.length
											? 'Deselect All'
											: 'Select All'}
									</button>
								</div>
								{#each embeddedDatasets as dataset (dataset.embedded_dataset_id)}
									<label
										class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-gray-100 dark:hover:bg-gray-600 cursor-pointer"
									>
										<input
											type="checkbox"
											checked={selectedEmbeddedDatasetIds.has(dataset.embedded_dataset_id)}
											onchange={() => {
												if (selectedEmbeddedDatasetIds.has(dataset.embedded_dataset_id)) {
													selectedEmbeddedDatasetIds.delete(dataset.embedded_dataset_id);
												} else {
													selectedEmbeddedDatasetIds.add(dataset.embedded_dataset_id);
												}
											}}
											class="rounded border-gray-300 dark:border-gray-500 text-blue-600 focus:ring-blue-500"
										/>
										<span class="text-sm text-gray-900 dark:text-white">{dataset.title}</span>
										{#if dataset.source_dataset_title}
											<span class="text-xs text-gray-500 dark:text-gray-400"
												>({dataset.source_dataset_title})</span
											>
										{/if}
									</label>
								{/each}
							{/if}
						</div>
					</fieldset>
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
											'Minimum number of points required to form a cluster. Larger values create fewer, more significant clusters. Smaller values find more fine-grained clusters but may include noise. Default: 15'
										)}
								>
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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

						<div class="col-span-1">
							<label
								for="topic-naming-prompt"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Topic Naming Prompt
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Custom prompt template for LLM topic naming. Use {{samples}} as a placeholder where the representative document texts will be inserted. The LLM should respond with just the topic name.'
										)}
								>
									<InfoCircleSolid class="w-4 h-4" />
								</button>
							</label>
							<textarea
								id="topic-naming-prompt"
								bind:value={config.topic_naming_prompt}
								rows="5"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white font-mono text-sm"
								placeholder={'These are representative texts from a document cluster:\n\n{{samples}}\n\nProvide a short, concise topic name...'}
							></textarea>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Use <code class="bg-gray-200 dark:bg-gray-600 px-1 rounded">{'{{samples}}'}</code> as
								placeholder for sample texts
							</p>
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
											'Font family for labels (e.g., Arial, sans-serif). Default: Playfair Display SC'
										)}
								>
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
								Polygon Alpha
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Alpha-shape parameter controlling how tightly cluster boundary polygons wrap around points. Lower values = tighter boundaries (may fail if too low). Higher values = looser, more convex-hull-like boundaries. Increase this if you get "polygon_alpha was too low" errors. Default: 0.5'
										)}
								>
									<InfoCircleSolid class="w-4 h-4" />
								</button>
							</label>
							<input
								id="polygon-alpha"
								type="number"
								bind:value={config.polygon_alpha}
								min="0.01"
								max="10.0"
								step="0.01"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
							/>
							<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
								Range: 0.01-10.0 (increase if boundaries fail to form)
							</p>
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
									<InfoCircleSolid class="w-4 h-4" />
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Palette Hue Shift (degrees)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Rotate the entire color palette around the color wheel. Use to change the overall color scheme. Default: 0'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Hue Radius Dependence
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'How much the hue changes based on distance from center. Higher values create more color variation across the map. Default: 1.0'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Palette Theta Range
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Angular range for the color palette in radians. Controls how much of the color wheel is used. π/16 ≈ 0.196 (default) uses a narrow range for subtle variation.'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Background Color (hex)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Custom background color in hex format (e.g., #1a1a2e). Leave empty to use automatic color based on dark/light mode.'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Color cluster label text to match the cluster color. When disabled, labels use a neutral color.'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Use a color palette optimized for users with color vision deficiency (colorblindness). Improves accessibility.'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Plot Title
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'Main title displayed at the top of the visualization.')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Plot Subtitle
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'Secondary title displayed below the main title.')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Title Font Size (pt)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'Font size for the main title in points. Default: 36')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Subtitle Font Size (pt)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'Font size for the subtitle in points. Default: 18')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Font Weight
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Font weight for labels. 100=thin, 400=normal, 700=bold, 900=black. Default: 600'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Line Spacing
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Multiplier for line height in wrapped text. 1.0 = normal, <1 = tighter, >1 = looser. Default: 0.95'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Outline Width
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Width of the outline/halo around label text for better readability. Default: 8'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Outline Color
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Color of the text outline/halo in hex format with optional alpha (e.g., #eeeeeedd). Default: #eeeeeedd'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Collision Scale
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Scale factor for label collision detection. Higher values create more space between labels. Default: 3.0'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Min Pixel Size
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Minimum text size in pixels when zoomed out. Labels smaller than this will be hidden. Default: 12'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Text Max Pixel Size
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Maximum text size in pixels when zoomed in. Prevents labels from becoming too large. Default: 36'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Tooltip Font Family
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Font family used for point hover tooltips. Default: Playfair Display SC'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Tooltip Font Weight
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Font weight for tooltips. 100=thin, 400=normal, 700=bold. Default: 400'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Hover Color
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Color shown when hovering over a point. Use hex with alpha (e.g., #aa0000bb). Default: #aa0000bb'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Size Scale
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Multiplier for point sizes. Leave empty for automatic sizing based on data density.'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
									</label>
									<input
										id="point-size-scale"
										type="number"
										bind:value={config.point_size_scale}
										step="0.1"
										placeholder="Auto"
										class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:ring-1 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
									/>
								</div>
								<div>
									<label
										for="point-radius-min-pixels"
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Radius Min (px)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Minimum point radius in pixels when zoomed out. Default: 0.01'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Radius Max (px)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Maximum point radius in pixels when zoomed in. Default: 24'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Line Width
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Stroke/outline width around each point. 0 = no outline. Default: 0.001'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Line Width Min
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Minimum point outline width in pixels when zoomed out. Default: 0.001'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Point Line Width Max
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Maximum point outline width in pixels when zoomed in. Default: 3'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Cluster Boundary Width
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Line width for cluster boundary polygons when enabled. Default: 1.0'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Width
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Width of the visualization. Can be a percentage (e.g., "100%") or pixels. Default: 100%'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Height (px)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'Height of the visualization in pixels. Default: 800')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Initial Zoom
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Initial zoom level. 1.0 = fit all points. <1 = zoomed in, >1 = zoomed out. Default: 1.0'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Noise Label
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Label displayed for points not assigned to any cluster. Default: Unlabelled'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Logo URL
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'URL of a logo image to display on the visualization.')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Logo Width (px)
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'Width of the logo in pixels. Default: 256')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
										class="flex items-center gap-1 text-xs font-medium text-gray-600 dark:text-gray-400 mb-1"
									>
										Background Image URL
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(e, 'URL of an image to use as the visualization background.')}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
											>Inline Data</span
										>
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Embed all data directly in the HTML file vs. loading from separate files. Recommended for portability.'
												)}
										>
											<InfoCircleSolid class="w-4 h-4" />
										</button>
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
								: `Creating ${selectedEmbeddedDatasetIds.size > 1 ? `${selectedEmbeddedDatasetIds.size} Transforms` : ''}...`
							: editingTransform
								? 'Update Transform'
								: selectedEmbeddedDatasetIds.size > 1
									? `Create ${selectedEmbeddedDatasetIds.size} Transforms`
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
						{@const stats = statsMap[transform.visualization_transform_id]}
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
							<td class="px-4 py-3">{stats?.latest_visualization?.point_count ?? '-'}</td>
							<td class="px-4 py-3">{stats?.latest_visualization?.cluster_count ?? '-'}</td>
							<td class="px-4 py-3">{formatDate(transform.created_at, false)}</td>
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
	onConfirm={confirmDeleteTransform}
	onCancel={() => (transformPendingDelete = null)}
/>

<ConfirmDialog
	open={transformsPendingBulkDelete.length > 0}
	title="Delete Visualization Transforms"
	message={`Are you sure you want to delete ${transformsPendingBulkDelete.length} transform${transformsPendingBulkDelete.length !== 1 ? 's' : ''}? This action cannot be undone.`}
	confirmLabel="Delete All"
	variant="danger"
	onConfirm={confirmBulkDeleteTransforms}
	onCancel={() => (transformsPendingBulkDelete = [])}
/>

<style>
	:global(.visualization-transforms-table :is(td, th)) {
		word-wrap: break-word;
		word-break: normal;
		white-space: normal;
		overflow-wrap: break-word;
	}
</style>
