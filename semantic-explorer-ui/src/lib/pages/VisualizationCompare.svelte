<script lang="ts">
	import { Spinner } from 'flowbite-svelte';
	import { ArrowLeftOutline } from 'flowbite-svelte-icons';
	import { onMount } from 'svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import type { Visualization, VisualizationTransform } from '../types/models';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';

	interface Props {
		ids: string; // comma-separated "transformId-vizId" pairs
		onBack?: () => void;
	}

	let { ids, onBack }: Props = $props();

	interface CompareItem {
		transformId: number;
		vizId: number;
		transform?: VisualizationTransform;
		visualization?: Visualization;
		htmlContent?: string;
		loading: boolean;
		error?: string;
	}

	let items = $state<CompareItem[]>([]);
	let columns = $state(2);

	onMount(() => {
		const pairs = ids.split(',').filter(Boolean);
		items = pairs.map((pair) => {
			const [transformId, vizId] = pair.split('-').map(Number);
			return { transformId, vizId, loading: true };
		});

		// Load all items in parallel
		for (const item of items) {
			loadItem(item);
		}
	});

	async function loadItem(item: CompareItem) {
		const downloadUrl = `/api/visualization-transforms/${item.transformId}/visualizations/${item.vizId}/download`;
		console.log('[VizCompare] loadItem: fetching', {
			transformId: item.transformId,
			vizId: item.vizId,
			downloadUrl,
		});
		try {
			// Fetch transform details and visualization HTML in parallel
			const [transformRes, downloadRes] = await Promise.all([
				fetch(`/api/visualization-transforms/${item.transformId}`),
				fetch(downloadUrl),
			]);

			if (transformRes.ok) {
				item.transform = await transformRes.json();
			}

			// Also fetch the visualization metadata
			const vizRes = await fetch(
				`/api/visualization-transforms/${item.transformId}/visualizations?limit=50`
			);
			if (vizRes.ok) {
				const vizList: Visualization[] = await vizRes.json();
				item.visualization = vizList.find((v) => v.visualization_id === item.vizId);
			}

			if (!downloadRes.ok) {
				throw new Error(`Failed to load HTML: ${downloadRes.statusText}`);
			}

			item.htmlContent = await downloadRes.text();
		} catch (err) {
			item.error = formatError(err, 'Failed to load visualization');
			toastStore.error(item.error);
		} finally {
			item.loading = false;
		}
	}

	function removeItem(index: number) {
		items = items.filter((_, i) => i !== index);
	}

	let gridClass = $derived(
		columns === 1
			? 'grid-cols-1'
			: columns === 2
				? 'grid-cols-2'
				: columns === 3
					? 'grid-cols-3'
					: 'grid-cols-4'
	);
</script>

<div class="max-w-full mx-auto">
	<div class="flex items-center gap-4 mb-6">
		{#if onBack}
			<button
				onclick={onBack}
				class="flex items-center gap-1 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
			>
				<ArrowLeftOutline class="w-4 h-4" />
				<span class="text-sm">Back</span>
			</button>
		{/if}
		<div class="flex-1">
			<PageHeader
				title="Compare Visualizations"
				description="Side-by-side comparison of {items.length} visualization{items.length !== 1
					? 's'
					: ''}."
			/>
		</div>
		<div class="flex items-center gap-2">
			<span class="text-sm text-gray-600 dark:text-gray-400">Grid:</span>
			{#each [1, 2, 3, 4] as cols (cols)}
				<button
					onclick={() => (columns = cols)}
					class="px-2.5 py-1 text-sm rounded transition-colors {columns === cols
						? 'bg-blue-600 text-white'
						: 'bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600'}"
				>
					{cols}
				</button>
			{/each}
		</div>
	</div>

	{#if items.length === 0}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-8 text-center">
			<p class="text-gray-500 dark:text-gray-400">No visualizations selected for comparison.</p>
			{#if onBack}
				<button onclick={onBack} class="mt-4 text-blue-600 dark:text-blue-400 hover:underline">
					Go back to select visualizations
				</button>
			{/if}
		</div>
	{:else}
		<div class="grid {gridClass} gap-4">
			{#each items as item, index (item.transformId + '-' + item.vizId)}
				<div
					class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden border border-gray-200 dark:border-gray-700 flex flex-col"
				>
					<!-- Header -->
					<div
						class="px-3 py-2 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 flex items-start justify-between gap-2"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 flex-wrap">
								<a
									href="#/visualizations/{item.vizId}/details"
									class="text-sm font-medium text-blue-600 dark:text-blue-400 hover:underline truncate"
								>
									{item.transform?.title ?? `Transform #${item.transformId}`}
								</a>
								<span
									class="text-xs shrink-0 px-2 py-0.5 rounded bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 font-medium"
								>
									Run #{item.vizId}
								</span>
							</div>
							{#if item.visualization}
								<div
									class="text-xs text-gray-500 dark:text-gray-500 mt-1 flex items-center gap-2 flex-wrap"
								>
									{#if item.visualization.point_count != null}
										<span>{item.visualization.point_count.toLocaleString()} points</span>
									{/if}
									{#if item.visualization.cluster_count != null}
										<span>· {item.visualization.cluster_count} clusters</span>
									{/if}
									{#if item.visualization.completed_at}
										<span>· {formatDate(item.visualization.completed_at)}</span>
									{/if}
								</div>
							{/if}
						</div>
						<button
							onclick={() => removeItem(index)}
							class="text-gray-400 hover:text-red-500 dark:hover:text-red-400 transition-colors shrink-0 p-0.5"
							title="Remove from comparison"
						>
							<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M6 18L18 6M6 6l12 12"
								/>
							</svg>
						</button>
					</div>
					<!-- Content -->
					<div class="flex-1 relative" style="min-height: 500px;">
						{#if item.loading}
							<div class="absolute inset-0 flex items-center justify-center">
								<Spinner size="8" />
							</div>
						{:else if item.error}
							<div class="absolute inset-0 flex items-center justify-center p-4">
								<p class="text-red-600 dark:text-red-400 text-sm text-center">{item.error}</p>
							</div>
						{:else if item.htmlContent}
							<iframe
								title="Visualization {item.vizId}"
								srcdoc={item.htmlContent}
								class="w-full h-full border-0"
								style="min-height: 500px; height: 100%;"
								sandbox="allow-scripts allow-same-origin allow-popups allow-forms allow-modals allow-pointer-lock allow-presentation"
							></iframe>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
