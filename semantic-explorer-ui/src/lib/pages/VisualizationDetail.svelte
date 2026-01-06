<script lang="ts">
	import { Deck, OrbitView } from '@deck.gl/core';
	import { ScatterplotLayer } from '@deck.gl/layers';
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

	interface ApiTopic {
		id: string;
		x: number;
		y: number;
		z: number;
		cluster_id: number;
		label: string;
		size: number | null;
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

	let deckContainer = $state<HTMLDivElement | undefined>(undefined);
	let deck: any = null;

	let selectedCluster = $state<number | null>(null);
	let hoveredPoint = $state<VisualizationPoint | null>(null);

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
		initializeDeck();
	});

	onDestroy(() => {
		if (deck) {
			deck.finalize();
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
			points = pointsData.points.map((p) => ({
				id: p.id,
				position: [p.x, p.y, p.z] as [number, number, number],
				cluster_id: p.cluster_id ?? -1,
				topic_label: p.topic_label,
				text: p.text ?? '',
			}));

			// Load topics
			const topicsResponse = await fetch(`/api/visualizations/${transformId}/topics`);
			if (!topicsResponse.ok) {
				throw new Error(`Failed to fetch topics: ${topicsResponse.statusText}`);
			}
			const topicsData: ApiTopic[] = await topicsResponse.json();
			topics = topicsData.map((t) => ({
				cluster_id: t.cluster_id,
				label: t.label,
				centroid: [t.x, t.y, t.z] as [number, number, number],
				size: t.size ?? 0,
			}));
		} catch (err) {
			error = formatError(err);
			toastStore.error(error);
		} finally {
			loading = false;
		}
	}

	function initializeDeck() {
		if (!deckContainer) return;

		deck = new Deck({
			container: deckContainer,
			views: new OrbitView({ orbitAxis: 'Y' }),
			initialViewState: {
				target: [0, 0, 0],
				rotationX: 15,
				rotationOrbit: 30,
				zoom: 0,
				minZoom: -5,
				maxZoom: 5,
			},
			controller: true,
			layers: [
				new ScatterplotLayer({
					id: 'points-layer',
					data: points,
					getPosition: (d: VisualizationPoint) => d.position,
					getColor: (d: VisualizationPoint) => {
						if (selectedCluster !== null && d.cluster_id !== selectedCluster) {
							const color = getClusterColor(d.cluster_id);
							return [...color, 50]; // Fade out non-selected
						}
						return getClusterColor(d.cluster_id);
					},
					getRadius: 0.02,
					radiusMinPixels: 2,
					radiusMaxPixels: 8,
					pickable: true,
					onHover: (info) => {
						if (info.object) {
							hoveredPoint = info.object;
						} else {
							hoveredPoint = null;
						}
					},
				}),
			],
		} as any);
	}

	$effect(() => {
		if (deck && points.length > 0) {
			deck.setProps({
				layers: [
					new ScatterplotLayer({
						id: 'points-layer',
						data: points,
						getPosition: (d: VisualizationPoint) => d.position,
						getColor: (d: VisualizationPoint) => {
							if (selectedCluster !== null && d.cluster_id !== selectedCluster) {
								const color = getClusterColor(d.cluster_id);
								return [...color, 50];
							}
							return getClusterColor(d.cluster_id);
						},
						getRadius: 0.02,
						radiusMinPixels: 2,
						radiusMaxPixels: 8,
						pickable: true,
						onHover: (info) => {
							if (info.object) {
								hoveredPoint = info.object;
							} else {
								hoveredPoint = null;
							}
						},
					}),
				],
			});
		}
	});

	function selectCluster(clusterId: number | null) {
		selectedCluster = clusterId;
	}
</script>

<div class="min-h-screen bg-gray-50 dark:bg-gray-900">
	<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
		<!-- Header -->
		<div class="mb-8">
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
				<p class="text-gray-600 dark:text-gray-400">3D Embedding Space Visualization</p>
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
			<div class="grid grid-cols-1 lg:grid-cols-4 gap-6">
				<!-- Left sidebar: Topics -->
				<div class="lg:col-span-1">
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
						<h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
							Topics ({topics.length})
						</h2>

						<div class="space-y-2 max-h-150 overflow-y-auto">
							<button
								onclick={() => selectCluster(null)}
								class="w-full text-left px-3 py-2 rounded transition-colors {selectedCluster ===
								null
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
											class="w-3 h-3 rounded-full"
											style="background-color: rgb({color[0]}, {color[1]}, {color[2]})"
										></div>
										<div class="flex-1">
											<div class="font-medium text-sm">{topic.label}</div>
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
				<div class="lg:col-span-3">
					<div class="bg-white dark:bg-gray-800 rounded-lg shadow">
						<div bind:this={deckContainer} class="w-full h-150 rounded-lg relative"></div>

						{#if hoveredPoint}
							{@const color = getClusterColor(hoveredPoint.cluster_id)}
							<div
								class="absolute top-4 right-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg p-4 max-w-md border border-gray-200 dark:border-gray-700"
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
					</div>

					<!-- Statistics -->
					<div class="mt-4 grid grid-cols-3 gap-4">
						<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
							<div class="text-sm text-gray-600 dark:text-gray-400">Total Points</div>
							<div class="text-2xl font-bold text-gray-900 dark:text-white">
								{points.length.toLocaleString()}
							</div>
						</div>
						<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
							<div class="text-sm text-gray-600 dark:text-gray-400">Topics</div>
							<div class="text-2xl font-bold text-gray-900 dark:text-white">
								{topics.length}
							</div>
						</div>
						<div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
							<div class="text-sm text-gray-600 dark:text-gray-400">Noise Points</div>
							<div class="text-2xl font-bold text-gray-900 dark:text-white">
								{points.filter((p) => p.cluster_id === -1).length.toLocaleString()}
							</div>
						</div>
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>
