<script lang="ts">
	import { Deck, OrbitView, OrthographicView, type Layer } from '@deck.gl/core';
	import { LineLayer, PointCloudLayer, ScatterplotLayer } from '@deck.gl/layers';
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

	interface VisualizationConfig {
		n_neighbors: number;
		n_components: number;
		min_dist: number;
		metric: string;
		min_cluster_size: number;
		min_samples: number | null;
	}

	interface VisualizationTransform {
		visualization_transform_id: number;
		title: string;
		embedded_dataset_id: number;
		owner: string;
		is_enabled: boolean;
		reduced_collection_name: string | null;
		topics_collection_name: string | null;
		visualization_config: VisualizationConfig;
		last_run_status: string | null;
		last_run_at: string | null;
		last_error: string | null;
		last_run_stats: {
			n_points?: number;
			n_clusters?: number;
			processing_duration_ms?: number;
		} | null;
		created_at: string;
		updated_at: string;
	}

	// Internal types
	interface VisualizationPoint {
		id: string;
		position: [number, number] | [number, number, number];
		cluster_id: number;
		topic_label: string | null;
		text: string;
	}

	interface Topic {
		cluster_id: number;
		label: string;
		centroid: [number, number] | [number, number, number];
		size: number;
		visible: boolean;
	}

	interface Props {
		transformId: number;
		onBack: () => void;
	}

	let { transformId, onBack }: Props = $props();

	let transform = $state<VisualizationTransform | null>(null);
	let points = $state<VisualizationPoint[]>([]);
	let topics = $state<Topic[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let deckCanvas = $state<HTMLCanvasElement | undefined>(undefined);
	let deck = $state<any>(null);

	let selectedClusters = $state<Set<number>>(new Set());
	let hoveredPoint = $state<VisualizationPoint | null>(null);

	// Container dimensions
	let containerWidth = $state<number>(0);
	let containerHeight = $state<number>(0);

	// Derived state
	let is2D = $derived(transform?.visualization_config.n_components === 2);

	// Bounding box state
	interface BoundingBox {
		minX: number;
		maxX: number;
		minY: number;
		maxY: number;
		minZ: number;
		maxZ: number;
		centerX: number;
		centerY: number;
		centerZ: number;
		sizeX: number;
		sizeY: number;
		sizeZ: number;
		maxSize: number;
	}

	let boundingBox = $state<BoundingBox | null>(null);

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


	function calculateBoundingBox(pts: VisualizationPoint[], mode2D: boolean): BoundingBox {
		if (pts.length === 0) {
			return {
				minX: -1,
				maxX: 1,
				minY: -1,
				maxY: 1,
				minZ: -1,
				maxZ: 1,
				centerX: 0,
				centerY: 0,
				centerZ: 0,
				sizeX: 2,
				sizeY: 2,
				sizeZ: 2,
				maxSize: 2,
			};
		}

		const xValues = pts.map((p) => p.position[0]);
		const yValues = pts.map((p) => p.position[1]);
		const zValues = mode2D ? [0] : pts.map((p) => (p.position as [number, number, number])[2]);

		const minX = Math.min(...xValues);
		const maxX = Math.max(...xValues);
		const minY = Math.min(...yValues);
		const maxY = Math.max(...yValues);
		const minZ = Math.min(...zValues);
		const maxZ = Math.max(...zValues);

		const sizeX = maxX - minX;
		const sizeY = maxY - minY;
		const sizeZ = maxZ - minZ;

		const centerX = (minX + maxX) / 2;
		const centerY = (minY + maxY) / 2;
		const centerZ = (minZ + maxZ) / 2;

		// Add padding to the max size (10% on each side = 20% total)
		const maxSize = Math.max(sizeX, sizeY, sizeZ) * 1.2;

		console.log('Bounding box calculated:', {
			minX,
			maxX,
			minY,
			maxY,
			minZ,
			maxZ,
			sizeX,
			sizeY,
			sizeZ,
			maxSize,
		});

		return {
			minX,
			maxX,
			minY,
			maxY,
			minZ,
			maxZ,
			centerX,
			centerY,
			centerZ,
			sizeX,
			sizeY,
			sizeZ,
			maxSize,
		};
	}

	function calculateOptimalZoom(
		bbox: BoundingBox,
		mode2D: boolean,
		canvasWidth: number,
		canvasHeight: number
	): number {
		// For 2D orthographic projection and 3D orbit view, calculate appropriate zoom
		if (mode2D) {
			// For orthographic view, zoom is a scale factor
			// We want to fit the bounding box with padding
			// Use aspect ratio to determine which dimension is constraining
			const xRange = bbox.maxX - bbox.minX;
			const yRange = bbox.maxY - bbox.minY;
			const canvasAspect = canvasWidth / canvasHeight;
			const dataAspect = xRange / yRange;

			// Calculate how much we need to scale to fit with 10% padding
			let zoomScale: number;
			if (dataAspect > canvasAspect) {
				// Width is constraining
				zoomScale = (canvasWidth / xRange) * 0.9;
			} else {
				// Height is constraining
				zoomScale = (canvasHeight / yRange) * 0.9;
			}

			// Normalize to Deck.gl's zoom scale (zoom ~1 is typical default)
			// The empirical factor 100 comes from testing - adjust if needed
			return zoomScale / 100;
		} else {
			// For 3D orbit view, calculate zoom to frame the bounding box
			// The viewport distance needed is roughly maxSize / (2 * tan(FOV/2))
			// Deck.gl's default FOV is about 50 degrees
			// A zoom of 1 is approximately 1.5x the max dimension away
			// Empirically, zoom of 0 to 3 works well, where 0 is far and 3 is close
			const zoomLevel = Math.log2(50 / bbox.maxSize);
			return Math.max(-10, Math.min(10, zoomLevel));
		}
	}

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
			// Respect the grid height constraint of 600px
			const constrainedHeight = Math.min(containerHeight, 600);
			console.log('Resizing deck to:', containerWidth, constrainedHeight);
			deck.setProps({
				width: containerWidth,
				height: constrainedHeight,
			});
		}
	});

	async function loadTransform() {
		try {
			const response = await fetch(`/api/visualization-transforms/${transformId}`);
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
			// Load all points with pagination
			let allPoints: ApiVisualizationPoint[] = [];
			let nextOffset: string | null = null;
			let pageCount = 0;
			
			do {
				pageCount++;
				const url = nextOffset 
					? `/api/visualizations/${transformId}/points?offset=${encodeURIComponent(nextOffset)}`
					: `/api/visualizations/${transformId}/points`;
				
				const pointsResponse = await fetch(url);
				if (!pointsResponse.ok) {
					throw new Error(`Failed to fetch points: ${pointsResponse.statusText}`);
				}
				const pointsData: ApiPointsResponse = await pointsResponse.json();
				
				allPoints = [...allPoints, ...pointsData.points];
				nextOffset = pointsData.next_offset;
				
				console.log(`Loaded page ${pageCount}: ${pointsData.points.length} points (total: ${allPoints.length})`);
			} while (nextOffset);

			console.log(`Total points loaded: ${allPoints.length} across ${pageCount} pages`);

			// Check dimensionality
			const nComponents = transform?.visualization_config.n_components || 3;
			console.log('Transform config:', transform?.visualization_config);
			console.log('nComponents detected:', nComponents);
			console.log('First few points from API:', allPoints.slice(0, 3));

			// Calculate center of the point cloud from raw data
			const rawXValues = allPoints.map((p) => p.x);
			const rawYValues = allPoints.map((p) => p.y);
			const rawZValues = allPoints.map((p) => p.z);

			const centerX = (Math.min(...rawXValues) + Math.max(...rawXValues)) / 2;
			const centerY = (Math.min(...rawYValues) + Math.max(...rawYValues)) / 2;
			const centerZ = (Math.min(...rawZValues) + Math.max(...rawZValues)) / 2;

			console.log('Raw Z values range:', [Math.min(...rawZValues), Math.max(...rawZValues)]);
			console.log('Point cloud center:', [centerX, centerY, centerZ]);

			// Scale factor: UMAP outputs are in [0,1] range but tightly clustered
			// Scale up by 10000x to make the small variations visible
			const SCALE = 10000;

			// Build topic map from points
			const clusterMap = new Map<
				number,
				{ label: string | null; count: number; sumX: number; sumY: number; sumZ: number }
			>();

			points = allPoints.map((p) => {
				const clusterId = p.cluster_id ?? -1;

				// Accumulate cluster info
				if (!clusterMap.has(clusterId)) {
					clusterMap.set(clusterId, { label: p.topic_label, count: 0, sumX: 0, sumY: 0, sumZ: 0 });
				}
				const cluster = clusterMap.get(clusterId)!;
				cluster.count++;
				cluster.sumX += p.x;
				cluster.sumY += p.y;
				cluster.sumZ += p.z; // Always accumulate Z regardless of nComponents

				// Create point with appropriate dimensionality
				if (nComponents === 2) {
					return {
						id: p.id,
						position: [(p.x - centerX) * SCALE, (p.y - centerY) * SCALE] as [number, number],
						cluster_id: clusterId,
						topic_label: p.topic_label,
						text: p.text ?? '',
					};
				} else {
					return {
						id: p.id,
						position: [
							(p.x - centerX) * SCALE,
							(p.y - centerY) * SCALE,
							(p.z - centerZ) * SCALE,
						] as [number, number, number],
						cluster_id: clusterId,
						topic_label: p.topic_label,
						text: p.text ?? '',
					};
				}
			});

			// Build topics from cluster map
			topics = Array.from(clusterMap.entries())
				.filter(([clusterId]) => clusterId !== -1) // Exclude noise
				.map(([clusterId, data]) => ({
					cluster_id: clusterId,
					label: data.label || `Cluster ${clusterId}`,
					centroid:
						nComponents === 2
							? ([
									(data.sumX / data.count - centerX) * SCALE,
									(data.sumY / data.count - centerY) * SCALE,
								] as [number, number])
							: ([
									(data.sumX / data.count - centerX) * SCALE,
									(data.sumY / data.count - centerY) * SCALE,
									(data.sumZ / data.count - centerZ) * SCALE,
								] as [number, number, number]),
					size: data.count,
					visible: true,
				}))
				.sort((a, b) => b.size - a.size); // Sort by size descending

			console.log(`Loaded ${points.length} points`);
			console.log(`Generated ${topics.length} topics from cluster data`);
			console.log('nComponents:', nComponents, '- is 3D?', nComponents === 3);
			if (points.length > 0) {
				console.log('Sample point:', points[0]);
				console.log('Sample position:', points[0].position);
				console.log('Sample position length:', (points[0].position as any).length);

				// Check coordinate ranges after scaling
				const xValues = points.map((p) => p.position[0]);
				const yValues = points.map((p) => p.position[1]);
				console.log('Scaled X range:', [Math.min(...xValues), Math.max(...xValues)]);
				console.log('Scaled Y range:', [Math.min(...yValues), Math.max(...yValues)]);
				if (nComponents === 3) {
					const zValues = points.map((p) => (p.position as [number, number, number])[2]);
					console.log('Scaled Z range:', [Math.min(...zValues), Math.max(...zValues)]);
					console.log('Z values sample:', zValues.slice(0, 5));
				} else {
					console.log('Detected as 2D - Z values not extracted');
				}

				// Calculate bounding box
				boundingBox = calculateBoundingBox(points, nComponents === 2);
			}
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

		// Set canvas dimensions - but use the actual constrained dimensions from the container
		// The container is constrained by the grid, so use those dimensions
		const width = Math.min(containerWidth, window.innerWidth);
		const height = Math.min(containerHeight, 600); // Grid height constraint

		deckCanvas.width = width;
		deckCanvas.height = height;

		console.log('Initializing Deck.GL with canvas:', {
			width,
			height,
			containerWidth,
			containerHeight,
		});
		console.log('Visualization mode:', is2D ? '2D' : '3D');

		// Calculate initial view state based on bounding box
		let initialViewState: any;

		if (boundingBox) {
			const constrainedHeight = Math.min(containerHeight, 600);
			const optimalZoom = calculateOptimalZoom(
				boundingBox,
				is2D,
				containerWidth,
				constrainedHeight
			);

			if (is2D) {
				initialViewState = {
					target: [boundingBox.centerX, boundingBox.centerY, 0],
					zoom: optimalZoom,
				};
			} else {
				// For 3D, center on the bounding box centroid with calculated zoom
				initialViewState = {
					target: [boundingBox.centerX, boundingBox.centerY, boundingBox.centerZ],
					rotationX: 30,
					rotationOrbit: 45,
					zoom: optimalZoom,
					minZoom: -10,
					maxZoom: 10,
				};
			}

			console.log('Initial view state from bounding box:', initialViewState);
		} else {
			// Fallback if bounding box not available
			initialViewState = is2D
				? {
						target: [0, 0, 0],
						zoom: 1,
					}
				: {
						...viewState,
						minZoom: -10,
						maxZoom: 10,
					};
		}

		// Choose view based on dimensionality
		const view = is2D
			? new OrthographicView({ controller: true })
			: new OrbitView({ orbitAxis: 'Y' });

		deck = new Deck({
			canvas: deckCanvas,
			views: view,
			initialViewState: initialViewState,
			width: containerWidth,
			height: containerHeight,
			controller: {
				// Enable both rotation and panning
				dragRotate: true,
				dragPan: true,
				doubleClickZoom: true,
				touchRotate: true,
				touchZoom: true,
				touchPan: true,
				keyboard: true,
				inertia: true,
				scrollZoom: true,
				minZoom: -10,
				maxZoom: 10,
			},
			layers: [],
			useDevicePixels: false,
			onViewStateChange: ({ viewState: newViewState }: { viewState: any }) => {
				if (is2D) {
					viewState = {
						target: newViewState.target as [number, number, number],
						rotationX: 0,
						rotationOrbit: 0,
						zoom: newViewState.zoom,
					};
				} else {
					viewState = {
						target: newViewState.target as [number, number, number],
						rotationX: newViewState.rotationX,
						rotationOrbit: newViewState.rotationOrbit,
						zoom: newViewState.zoom,
					};
				}
			},
		} as any);

		console.log(
			'Deck.GL initialized successfully with size:',
			containerWidth,
			'x',
			containerHeight
		);
	}

	// Update deck layers when points, topics, or selectedClusters changes
	$effect(() => {
		// Track dependencies
		const currentPoints = points;
		const currentTopics = topics;
		const currentSelectedClusters = selectedClusters;
		const gridVisible = showGrid;
		const mode2D = is2D;

		if (!deck) {
			console.log('Deck not initialized yet');
			return;
		}

		// Build visibility map from topics
		const visibleClusters = new Set(
			currentTopics.filter((t) => t.visible).map((t) => t.cluster_id)
		);

		// Filter points based on topic visibility
		const filteredPoints = currentPoints.filter((p) => {
			// Always show noise points (-1)
			if (p.cluster_id === -1) return true;
			// Show if topic is visible
			return visibleClusters.has(p.cluster_id);
		});

		console.log(
			`Updating deck with ${filteredPoints.length}/${currentPoints.length} visible points, ${currentTopics.filter((t) => t.visible).length}/${currentTopics.length} visible topics`
		);

		const layers: Layer[] = mode2D
			? [
					// 2D: Use ScatterplotLayer (billboards work fine for 2D)
					new ScatterplotLayer({
						id: 'points-layer',
						data: filteredPoints,
						getPosition: (d: VisualizationPoint) => d.position,
						getFillColor: (d: VisualizationPoint) => {
							if (currentSelectedClusters.size > 0 && !currentSelectedClusters.has(d.cluster_id)) {
								const color = getClusterColor(d.cluster_id);
								return [...color, 50];
							}
							return getClusterColor(d.cluster_id);
						},
						getRadius: 15,
						radiusMinPixels: 5,
						radiusMaxPixels: 25,
						pickable: true,
						onHover: (info: any) => {
							if (info.object) {
								hoveredPoint = info.object;
							} else {
								hoveredPoint = null;
							}
						},
					}),
				]
			: [
					// 3D: Use PointCloudLayer for proper 3D point rendering with depth
					new PointCloudLayer({
						id: 'points-layer',
						data: filteredPoints,
						getPosition: (d: VisualizationPoint) => d.position,
						getColor: (d: VisualizationPoint) => {
							if (currentSelectedClusters.size > 0 && !currentSelectedClusters.has(d.cluster_id)) {
								const color = getClusterColor(d.cluster_id);
								return [...color, 50];
							}
							return getClusterColor(d.cluster_id);
						},
						getRadius: 50,
						radiusPixels: 5,
						pickable: true,
						onHover: (info: any) => {
							if (info.object) {
								hoveredPoint = info.object;
							} else {
								hoveredPoint = null;
							}
						},
					}),
				];

		if (gridVisible && !mode2D) {
			// Add axis lines (3D only)
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

			// Add grid lines (3D only)
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

	function toggleTopic(clusterId: number) {
		topics = topics.map((t) => (t.cluster_id === clusterId ? { ...t, visible: !t.visible } : t));
	}

	function toggleAllTopics(visible: boolean) {
		topics = topics.map((t) => ({ ...t, visible }));
	}

	function selectCluster(clusterId: number | null) {
		if (clusterId === null) {
			selectedClusters = new Set();
		} else {
			if (selectedClusters.has(clusterId)) {
				selectedClusters.delete(clusterId);
			} else {
				selectedClusters.add(clusterId);
			}
			selectedClusters = new Set(selectedClusters); // Trigger reactivity
		}
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
		if (boundingBox) {
			const optimalZoom = calculateOptimalZoom(boundingBox, is2D, containerWidth, containerHeight);

			if (is2D) {
				viewState = {
					target: [boundingBox.centerX, boundingBox.centerY, 0],
					rotationX: 0,
					rotationOrbit: 0,
					zoom: optimalZoom,
				};
			} else {
				viewState = {
					target: [boundingBox.centerX, boundingBox.centerY, boundingBox.centerZ],
					rotationX: 30,
					rotationOrbit: 45,
					zoom: optimalZoom,
				};
			}
		} else {
			viewState = {
				target: [0, 0, 0],
				rotationX: 30,
				rotationOrbit: 45,
				zoom: 1,
			};
		}

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
				<span class="text-lg text-gray-500 dark:text-gray-400">
					({is2D ? '2D' : '3D'} Visualization)
				</span>
			</h1>

			<!-- Status Banner -->
			{#if transform.last_run_status}
				<div class="mt-4">
					{#if transform.last_run_status === 'completed'}
						<div
							class="bg-green-50 dark:bg-green-900/20 border-l-4 border-green-400 p-4 rounded-lg"
						>
							<div class="flex items-center justify-between">
								<div>
									<p class="font-semibold text-green-800 dark:text-green-400">
										✓ Processing Complete
									</p>
									{#if transform.last_run_stats}
										<p class="text-sm text-green-700 dark:text-green-300 mt-1">
											Generated {transform.last_run_stats.n_points?.toLocaleString()} points in {transform
												.last_run_stats.n_clusters} clusters
											{#if transform.last_run_stats.processing_duration_ms}
												in {(transform.last_run_stats.processing_duration_ms / 1000).toFixed(1)}s
											{/if}
										</p>
									{/if}
								</div>
								{#if transform.last_run_at}
									<p class="text-xs text-green-600 dark:text-green-400">
										{new Date(transform.last_run_at).toLocaleString()}
									</p>
								{/if}
							</div>
						</div>
					{:else if transform.last_run_status === 'failed'}
						<div class="bg-red-50 dark:bg-red-900/20 border-l-4 border-red-400 p-4 rounded-lg">
							<div class="flex items-center justify-between">
								<div class="flex-1">
									<p class="font-semibold text-red-800 dark:text-red-400">✗ Processing Failed</p>
									{#if transform.last_error}
										<p class="text-sm text-red-700 dark:text-red-300 mt-1">
											{transform.last_error}
										</p>
									{/if}
								</div>
								{#if transform.last_run_at}
									<p class="text-xs text-red-600 dark:text-red-400 ml-4">
										{new Date(transform.last_run_at).toLocaleString()}
									</p>
								{/if}
							</div>
						</div>
					{:else if transform.last_run_status === 'processing'}
						<div class="bg-blue-50 dark:bg-blue-900/20 border-l-4 border-blue-400 p-4 rounded-lg">
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-3">
									<svg
										class="animate-spin h-5 w-5 text-blue-600 dark:text-blue-400"
										xmlns="http://www.w3.org/2000/svg"
										fill="none"
										viewBox="0 0 24 24"
									>
										<circle
											class="opacity-25"
											cx="12"
											cy="12"
											r="10"
											stroke="currentColor"
											stroke-width="4"
										></circle>
										<path
											class="opacity-75"
											fill="currentColor"
											d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
										></path>
									</svg>
									<p class="font-semibold text-blue-800 dark:text-blue-400">Processing...</p>
								</div>
								{#if transform.last_run_at}
									<p class="text-xs text-blue-600 dark:text-blue-400">
										Started {new Date(transform.last_run_at).toLocaleString()}
									</p>
								{/if}
							</div>
						</div>
					{/if}
				</div>
			{/if}
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
			<div class="lg:col-span-1 overflow-hidden">
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow p-6 h-full flex flex-col overflow-y-auto"
				>
					<div class="flex items-center justify-between mb-4">
						<h2 class="text-lg font-semibold text-gray-900 dark:text-white">
							Topics ({topics.filter((t) => t.visible).length}/{topics.length})
						</h2>
						<div class="flex gap-1">
							<button
								onclick={() => toggleAllTopics(true)}
								class="text-xs px-2 py-1 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 rounded"
								title="Show All"
							>
								All
							</button>
							<button
								onclick={() => toggleAllTopics(false)}
								class="text-xs px-2 py-1 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 rounded"
								title="Hide All"
							>
								None
							</button>
						</div>
					</div>

					<div class="space-y-2 overflow-y-auto flex-1">
						{#each topics as topic (topic.cluster_id)}
							{@const color = getClusterColor(topic.cluster_id)}
							{@const isHighlighted = selectedClusters.has(topic.cluster_id)}
							<div class="flex items-center gap-2">
								<input
									type="checkbox"
									checked={topic.visible}
									onchange={() => toggleTopic(topic.cluster_id)}
									class="w-4 h-4 text-blue-600 rounded focus:ring-2 focus:ring-blue-500"
								/>
								<button
									onclick={() => selectCluster(topic.cluster_id)}
									class="flex-1 text-left px-3 py-2 rounded transition-colors {isHighlighted
										? 'bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100'
										: 'hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300'}"
								>
									<div class="flex items-center gap-2">
										<div
											class="w-3 h-3 rounded-full shrink-0"
											style="background-color: rgb({color[0]}, {color[1]}, {color[2]}); opacity: {topic.visible
												? 1
												: 0.3}"
										></div>
										<div class="flex-1 min-w-0">
											<div class="font-medium text-sm truncate">{topic.label}</div>
											<div class="text-xs text-gray-500 dark:text-gray-400">
												{topic.size} points
											</div>
										</div>
									</div>
								</button>
							</div>
						{/each}
					</div>
				</div>
			</div>

			<!-- Main content: 3D visualization -->
			<div class="lg:col-span-3 overflow-hidden">
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow relative overflow-hidden w-full h-full"
					bind:clientWidth={containerWidth}
					bind:clientHeight={containerHeight}
					style="display: flex; flex-direction: column;"
				>
					<div class="absolute inset-0 rounded-lg flex" style="pointer-events: auto;">
						<div id="deckgl-wrapper" class="w-full h-full relative flex-1" style="flex-grow: 1;">
							<canvas
								bind:this={deckCanvas}
								class="w-full h-full block"
								style="touch-action: none; display: block; width: 100%; height: 100%;"
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
					<div
						class="absolute bottom-4 left-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg p-4 border border-gray-200 dark:border-gray-700 space-y-3"
						style="z-index: 10; pointer-events: auto;"
					>
						<!-- Zoom Control -->
						<div>
							<label
								for="zoom-slider"
								class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2"
							>
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
							{#if !is2D}
								<button
									onclick={() => (showGrid = !showGrid)}
									class="flex-1 px-3 py-2 text-xs font-medium {showGrid
										? 'text-white bg-blue-600 dark:bg-blue-500'
										: 'text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700'} hover:bg-blue-700 dark:hover:bg-blue-600 rounded transition-colors"
									title="Toggle Grid"
								>
									Grid
								</button>
							{/if}
						</div>

						<!-- View Info -->
						<div class="text-xs text-gray-500 dark:text-gray-400 space-y-1">
							<div>Mode: {is2D ? '2D' : '3D'}</div>
							{#if !is2D}
								<div>Rotation: {viewState.rotationOrbit.toFixed(0)}°</div>
								<div class="text-[10px] text-gray-400 dark:text-gray-500">
									<span class="text-red-500">Red</span>=X
									<span class="text-green-500">Green</span>=Y
									<span class="text-blue-500">Blue</span>=Z
								</div>
							{/if}
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
