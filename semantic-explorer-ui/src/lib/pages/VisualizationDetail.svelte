<script lang="ts">
	import { Deck, OrbitView, type Layer } from '@deck.gl/core';
	import { LineLayer, ScatterplotLayer } from '@deck.gl/layers';
	import { onDestroy, onMount } from 'svelte';
	import { formatError, toastStore } from '../utils/notifications';

	// API response types
	interface ApiVisualizationPoint {
		id: string;
		x: number;
		y: number;
		z: number;
		cluster_id: number | null;
		topic_label: string | null;
		text: string | null;
	}

	interface ApiPointsResponse {
		points: ApiVisualizationPoint[];
		next_offset: string | null;
	}

	// Internal types
	interface VisualizationPoint {
		id: string;
		position: [number, number, number];
		cluster_id: number;
		topic_label: string | null;
		text: string;
	}

	interface Topic {
		cluster_id: number;
		label: string;
		centroid: [number, number, number];
		size: number;
	}

	interface Transform {
		transform_id: number;
		title: string;
		job_type: string;
		dataset_id: number;
		source_transform_id: number | null;
		embedder_ids?: number[] | null;
		job_config: any;
		updated_at: string;
	}

	interface Props {
		transformId: number;
		onBack: () => void;
	}

	let { transformId, onBack }: Props = $props();

	let transform = $state<Transform | null>(null);
	let points = $state<VisualizationPoint[]>([]);
	let topics = $state<Topic[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let deckCanvas = $state<HTMLCanvasElement | undefined>(undefined);
	let deck = $state<any>(null);

	let selectedCluster = $state<number | null>(null);
	let hoveredPoint = $state<VisualizationPoint | null>(null);

	// Container dimensions
	let containerWidth = $state<number>(0);
	let containerHeight = $state<number>(0);

	// View controls
	let viewState = $state({
		target: [0, 0, 0] as [number, number, number],
		rotationX: 30,
		rotationOrbit: 45,
		zoom: 1,
	});
	let showGrid = $state(true);

	// Color palette for clusters (up to 20 distinct colors)
	const CLUSTER_COLORS: [number, number, number][] = [
		[255, 99, 71], // Tomato
		[135, 206, 250], // Sky Blue
		[144, 238, 144], // Light Green
		[255, 215, 0], // Gold
		[216, 191, 216], // Thistle
		[255, 160, 122], // Light Salmon
		[173, 216, 230], // Light Blue
		[255, 182, 193], // Light Pink
		[255, 218, 185], // Peach
		[176, 224, 230], // Powder Blue
		[255, 228, 181], // Moccasin
		[221, 160, 221], // Plum
		[250, 128, 114], // Salmon
		[152, 251, 152], // Pale Green
		[255, 239, 213], // Papaya Whip
		[175, 238, 238], // Pale Turquoise
		[240, 230, 140], // Khaki
		[255, 192, 203], // Pink
		[230, 230, 250], // Lavender
		[211, 211, 211], // Light Gray
	];

	const getClusterColor = (clusterId: number): [number, number, number] => {
		if (clusterId === -1) return [128, 128, 128]; // Gray for noise
		return CLUSTER_COLORS[clusterId % CLUSTER_COLORS.length];
	};

	onMount(async () => {
		await loadTransform();
		await loadVisualizationData();
	});

	onDestroy(() => {
		if (deck) {
			deck.finalize();
		}
	});

	// Initialize deck when container dimensions are available
	$effect(() => {
		if (!deck && deckCanvas && containerWidth > 0 && containerHeight > 0) {
			console.log('Initializing Deck.GL with dimensions:', containerWidth, containerHeight);
			initializeDeck();
		}
	});

	// Resize deck when container dimensions change
	$effect(() => {
		if (deck && containerWidth > 0 && containerHeight > 0) {
			console.log('Resizing deck to:', containerWidth, containerHeight);
			deck.setProps({
				width: containerWidth,
				height: containerHeight,
			});
		}
	});

	async function loadTransform() {
		try {
			const response = await fetch(`/api/transforms/${transformId}`);
			if (!response.ok) {
				throw new Error(`Failed to fetch transform: ${response.statusText}`);
			}
			transform = await response.json();
		} catch (err) {
			error = formatError(err);
			toastStore.error(error);
		}
	}

	async function loadVisualizationData() {
		loading = true;
		error = null;

		try {
			// Load points
			const pointsResponse = await fetch(`/api/visualizations/${transformId}/points`);
			if (!pointsResponse.ok) {
				throw new Error(`Failed to fetch points: ${pointsResponse.statusText}`);
			}
			const pointsData: ApiPointsResponse = await pointsResponse.json();
			// Calculate center of the point cloud from raw data
			const rawXValues = pointsData.points.map((p) => p.x);
			const rawYValues = pointsData.points.map((p) => p.y);
			const rawZValues = pointsData.points.map((p) => p.z);

			const centerX = (Math.min(...rawXValues) + Math.max(...rawXValues)) / 2;
			const centerY = (Math.min(...rawYValues) + Math.max(...rawYValues)) / 2;
			const centerZ = (Math.min(...rawZValues) + Math.max(...rawZValues)) / 2;

			console.log('Point cloud center:', [centerX, centerY, centerZ]);

			// Scale factor: UMAP outputs are in [0,1] range but tightly clustered
			// Scale up by 10000x to make the small variations visible
			const SCALE = 10000;

			points = pointsData.points.map((p) => ({
				id: p.id,
				// Center and scale coordinates to make tightly clustered UMAP output visible
				position: [(p.x - centerX) * SCALE, (p.y - centerY) * SCALE, (p.z - centerZ) * SCALE] as [
					number,
					number,
					number,
				],
				cluster_id: p.cluster_id ?? -1,
				topic_label: p.topic_label,
				text: p.text ?? '',
			}));

			console.log(`Loaded ${points.length} points`);
			if (points.length > 0) {
				console.log('Sample point:', points[0]);
				console.log('Sample position [x,y,z]:', points[0].position);

				// Check coordinate ranges after scaling
				const xValues = points.map((p) => p.position[0]);
				const yValues = points.map((p) => p.position[1]);
				const zValues = points.map((p) => p.position[2]);
				console.log('Scaled X range:', [Math.min(...xValues), Math.max(...xValues)]);
				console.log('Scaled Y range:', [Math.min(...yValues), Math.max(...yValues)]);
				console.log('Scaled Z range:', [Math.min(...zValues), Math.max(...zValues)]);
			}

			// Load topics - temporarily disabled since topics collection isn't dimensionally reduced yet
			// const topicsResponse = await fetch(`/api/visualizations/${transformId}/topics`);
			// if (!topicsResponse.ok) {
			// 	throw new Error(`Failed to fetch topics: ${topicsResponse.statusText}`);
			// }
			// const topicsData: ApiTopic[] = await topicsResponse.json();
			// topics = topicsData.map((t) => ({
			// 	cluster_id: t.cluster_id,
			// 	label: t.label,
			// 	centroid: [t.x, t.y, t.z] as [number, number, number],
			// 	size: t.size ?? 0,
			// }));

			console.log(`Loaded ${topics.length} topics (topics disabled for now)`);
		} catch (err) {
			error = formatError(err);
			toastStore.error(error);
			console.error('Failed to load visualization data:', err);
		} finally {
			loading = false;
		}
	}

	function initializeDeck() {
		if (!deckCanvas) {
			console.warn('Deck canvas not ready');
			return;
		}

		if (deck) {
			console.log('Deck already initialized');
			return;
		}

		if (containerWidth === 0 || containerHeight === 0) {
			console.warn('Container dimensions not available yet');
			return;
		}

		// Set canvas dimensions before initializing Deck
		deckCanvas.width = containerWidth;
		deckCanvas.height = containerHeight;

		console.log('Initializing Deck.GL with canvas:', deckCanvas, containerWidth, containerHeight);

		deck = new Deck({
			canvas: deckCanvas,
			views: new OrbitView({ orbitAxis: 'Y' }),
			initialViewState: {
				...viewState,
				minZoom: -10,
				maxZoom: 10,
			},
			width: containerWidth,
			height: containerHeight,
			controller: true,
			layers: [],
			useDevicePixels: false,
			onViewStateChange: ({ viewState: newViewState }: { viewState: any }) => {
				viewState = {
					target: newViewState.target as [number, number, number],
					rotationX: newViewState.rotationX,
					rotationOrbit: newViewState.rotationOrbit,
					zoom: newViewState.zoom,
				};
			},
		} as any);

		console.log('Deck.GL initialized successfully with size:', containerWidth, 'x', containerHeight);
	}

	// Update deck layers when points or selectedCluster changes
	$effect(() => {
		// Track dependencies
		const currentPoints = points;
		const currentCluster = selectedCluster;
		const gridVisible = showGrid;

		if (!deck) {
			console.log('Deck not initialized yet');
			return;
		}

		console.log(
			`Updating deck with ${currentPoints.length} points, selectedCluster: ${currentCluster}`
		);

		const layers: Layer[] = [
			new ScatterplotLayer({
				id: 'points-layer',
				data: currentPoints,
				getPosition: (d: VisualizationPoint) => d.position,
				getFillColor: (d: VisualizationPoint) => {
					if (currentCluster !== null && d.cluster_id !== currentCluster) {
						const color = getClusterColor(d.cluster_id);
						return [...color, 50];
					}
					return getClusterColor(d.cluster_id);
				},
				getRadius: 0.5,
				radiusMinPixels: 4,
				radiusMaxPixels: 20,
				pickable: true,
				onHover: (info) => {
					if (info.object) {
						hoveredPoint = info.object;
					} else {
						hoveredPoint = null;
					}
				},
			}),
		];

		if (gridVisible) {
			// Add axis lines
			layers.push(
				new LineLayer({
					id: 'axis-layer',
					data: createAxisLines(100),
					getSourcePosition: (d) => d.sourcePosition,
					getTargetPosition: (d) => d.targetPosition,
					getColor: (d) => d.color,
					getWidth: 3,
					widthMinPixels: 2,
				})
			);

			// Add grid lines
			layers.push(
				new LineLayer({
					id: 'grid-layer',
					data: createGridLines(100),
					getSourcePosition: (d) => d.sourcePosition,
					getTargetPosition: (d) => d.targetPosition,
					getColor: (d) => d.color,
					getWidth: 1,
					widthMinPixels: 1,
				})
			);
		}

		deck.setProps({ layers });
	});

	function selectCluster(clusterId: number | null) {
		selectedCluster = clusterId;
	}

	function createGridLines(size: number = 100): any[] {
		const lines = [];
		const step = size / 10;
		const color = [128, 128, 128, 100]; // Semi-transparent gray

		// Grid lines on XY plane (Z=0)
		for (let i = -5; i <= 5; i++) {
			// Lines parallel to X axis
			lines.push({
				sourcePosition: [-size / 2, i * step, 0],
				targetPosition: [size / 2, i * step, 0],
				color,
			});
			// Lines parallel to Y axis
			lines.push({
				sourcePosition: [i * step, -size / 2, 0],
				targetPosition: [i * step, size / 2, 0],
				color,
			});
		}

		return lines;
	}

	function createAxisLines(size: number = 100): any[] {
		return [
			// X axis - red
			{
				sourcePosition: [0, 0, 0],
				targetPosition: [size / 2, 0, 0],
				color: [255, 0, 0, 200],
			},
			// Y axis - green
			{
				sourcePosition: [0, 0, 0],
				targetPosition: [0, size / 2, 0],
				color: [0, 255, 0, 200],
			},
			// Z axis - blue
			{
				sourcePosition: [0, 0, 0],
				targetPosition: [0, 0, size / 2],
				color: [0, 0, 255, 200],
			},
		];
	}

	function resetView() {
		viewState = {
			target: [0, 0, 0],
			rotationX: 30,
			rotationOrbit: 45,
			zoom: 1,
		};
		if (deck) {
			deck.setProps({
				initialViewState: {
					...viewState,
					minZoom: -10,
					maxZoom: 10,
				},
			});
		}
	}

	function updateZoom(newZoom: number) {
		viewState = { ...viewState, zoom: newZoom };
		if (deck) {
			deck.setProps({
				initialViewState: {
					...viewState,
					minZoom: -10,
					maxZoom: 10,
				},
			});
		}
	}
</script>

<div>
	<!-- Header -->
	<div class="mb-6">
		<button
			onclick={onBack}
			class="mb-4 text-blue-600 dark:text-blue-400 hover:text-blue-800 dark:hover:text-blue-300 flex items-center gap-2"
		>
			← Back
		</button>

		{#if transform}
			<h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
				{transform.title}
			</h1>
		{/if}
	</div>

	{#if loading && points.length === 0}
		<div class="flex items-center justify-center h-96">
			<div class="text-center">
				<svg
					class="animate-spin h-12 w-12 text-blue-600 dark:text-blue-400 mx-auto mb-4"
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
				>
					<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
					></circle>
					<path
						class="opacity-75"
						fill="currentColor"
						d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
					></path>
				</svg>
				<p class="text-gray-600 dark:text-gray-400">Loading visualization...</p>
			</div>
		</div>
	{:else if error}
		<div class="bg-red-50 dark:bg-red-900/20 border-l-4 border-red-400 p-4 rounded-lg">
			<p class="text-red-700 dark:text-red-400">{error}</p>
		</div>
	{:else}
		<!-- Statistics Bar -->
		<div class="mb-2 grid grid-cols-3 gap-4">
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
				<div class="text-sm text-gray-600 dark:text-gray-400">Total Points</div>
				<div class="text-2xl font-bold text-gray-900 dark:text-white">
					{points.length.toLocaleString()}
				</div>
			</div>
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
				<div class="text-sm text-gray-600 dark:text-gray-400">Topics</div>
				<div class="text-2xl font-bold text-gray-900 dark:text-white">{topics.length}</div>
			</div>
			<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
				<div class="text-sm text-gray-600 dark:text-gray-400">Noise Points</div>
				<div class="text-2xl font-bold text-gray-900 dark:text-white">
					{points.filter((p) => p.cluster_id === -1).length.toLocaleString()}
				</div>
			</div>
		</div>

		<div class="grid grid-cols-1 lg:grid-cols-4 gap-6 overflow-hidden" style="height: 600px;">
			<!-- Left sidebar: Topics -->
			<div class="lg:col-span-1">
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6 h-full flex flex-col">
					<h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
						Topics ({topics.length})
					</h2>

					<div class="space-y-2 overflow-y-auto flex-1">
						<button
							onclick={() => selectCluster(null)}
							class="w-full text-left px-3 py-2 rounded transition-colors {selectedCluster === null
								? 'bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100'
								: 'hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300'}"
						>
							<div class="font-medium">All Topics</div>
							<div class="text-sm text-gray-500 dark:text-gray-400">
								{points.length} points
							</div>
						</button>

						{#each topics as topic (topic.cluster_id)}
							{@const color = getClusterColor(topic.cluster_id)}
							<button
								onclick={() => selectCluster(topic.cluster_id)}
								class="w-full text-left px-3 py-2 rounded transition-colors {selectedCluster ===
								topic.cluster_id
									? 'bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100'
									: 'hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300'}"
							>
								<div class="flex items-center gap-2">
									<div
										class="w-3 h-3 rounded-full shrink-0"
										style="background-color: rgb({color[0]}, {color[1]}, {color[2]})"
									></div>
									<div class="flex-1 min-w-0">
										<div class="font-medium text-sm truncate">{topic.label}</div>
										<div class="text-xs text-gray-500 dark:text-gray-400">
											{topic.size} points
										</div>
									</div>
								</div>
							</button>
						{/each}
					</div>
				</div>
			</div>

			<!-- Main content: 3D visualization -->
			<div class="lg:col-span-3 relative">
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow h-full relative overflow-hidden"
					bind:clientWidth={containerWidth}
					bind:clientHeight={containerHeight}
				>
					<div class="absolute inset-0 rounded-lg">
						<div
							id="deckgl-wrapper"
							class="w-full h-full relative"
						>
							<canvas
								bind:this={deckCanvas}
								class="w-full h-full block"
								style="touch-action: none;"
							></canvas>
						</div>
					</div>

					{#if hoveredPoint}
						{@const color = getClusterColor(hoveredPoint.cluster_id)}
						<div
							class="absolute top-4 right-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg p-4 max-w-md border border-gray-200 dark:border-gray-700 pointer-events-none"
							style="z-index: 10;"
						>
							<div class="flex items-start gap-2">
								<div
									class="w-3 h-3 rounded-full mt-1 shrink-0"
									style="background-color: rgb({color[0]}, {color[1]}, {color[2]})"
								></div>
								<div class="flex-1 min-w-0">
									{#if hoveredPoint.topic_label}
										<div class="text-sm font-semibold text-gray-900 dark:text-white mb-1">
											{hoveredPoint.topic_label}
										</div>
									{/if}
									<div class="text-sm text-gray-700 dark:text-gray-300 line-clamp-4">
										{hoveredPoint.text}
									</div>
								</div>
							</div>
						</div>
					{/if}

					<!-- Controls Panel -->
					<div class="absolute bottom-4 left-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg p-4 border border-gray-200 dark:border-gray-700 space-y-3" style="z-index: 10; pointer-events: auto;">
						<!-- Zoom Control -->
						<div>
							<label for="zoom-slider" class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
								Zoom: {viewState.zoom.toFixed(1)}
							</label>
							<input
								id="zoom-slider"
								type="range"
								min="-10"
								max="10"
								step="0.1"
								value={viewState.zoom}
								oninput={(e) => updateZoom(parseFloat(e.currentTarget.value))}
								class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-blue-600"
							/>
						</div>

						<!-- Control Buttons -->
						<div class="flex gap-2">
							<button
								onclick={resetView}
								class="flex-1 px-3 py-2 text-xs font-medium text-white bg-blue-600 hover:bg-blue-700 dark:bg-blue-500 dark:hover:bg-blue-600 rounded transition-colors"
								title="Reset View"
							>
								Reset
							</button>
							<button
								onclick={() => (showGrid = !showGrid)}
								class="flex-1 px-3 py-2 text-xs font-medium {showGrid
									? 'text-white bg-blue-600 dark:bg-blue-500'
									: 'text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700'} hover:bg-blue-700 dark:hover:bg-blue-600 rounded transition-colors"
								title="Toggle Grid"
							>
								Grid
							</button>
						</div>

						<!-- View Info -->
						<div class="text-xs text-gray-500 dark:text-gray-400 space-y-1">
							<div>Rotation: {viewState.rotationOrbit.toFixed(0)}°</div>
							<div class="text-[10px] text-gray-400 dark:text-gray-500">
								<span class="text-red-500">Red</span>=X
								<span class="text-green-500">Green</span>=Y
								<span class="text-blue-500">Blue</span>=Z
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	/* Constrain Deck.GL canvas within container */
	#deckgl-wrapper {
		overflow: hidden;
	}

	#deckgl-wrapper canvas {
		display: block;
		outline: none;
	}
</style>
