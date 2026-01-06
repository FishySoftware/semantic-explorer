<script lang="ts">
	import {
		ArrowsRepeatOutline,
		BookOpenSolid,
		BrainSolid,
		ChartPieSolid,
		CubeSolid,
		DatabaseSolid,
		FolderSolid,
		GridSolid,
		LayersSolid,
		SearchOutline,
	} from 'flowbite-svelte-icons';
	import type { Props } from 'flowbite-svelte-icons/types';
	import type { Component } from 'svelte';

	let { activeUrl = $bindable('/dashboard') } = $props();

	const menuItems: Array<{ name: string; icon: Component<Props, {}, ''>; url: string }> = [
		{ name: 'Dashboard', icon: GridSolid, url: '/dashboard' },
		{ name: 'Documentation', icon: BookOpenSolid, url: '/documentation' },
		{ name: 'Collections', icon: FolderSolid, url: '/collections' },
		{ name: 'Datasets', icon: DatabaseSolid, url: '/datasets' },
		{ name: 'Embedders', icon: BrainSolid, url: '/embedders' },
		{ name: 'Collection Transforms', icon: ArrowsRepeatOutline, url: '/collection-transforms' },
		{ name: 'Dataset Transforms', icon: LayersSolid, url: '/dataset-transforms' },
		{ name: 'Embedded Datasets', icon: CubeSolid, url: '/embedded-datasets' },
		{ name: 'Visualization Transforms', icon: ChartPieSolid, url: '/visualization-transforms' },
		{ name: 'Search', icon: SearchOutline, url: '/search' },
		{ name: 'Visualizations', icon: ChartPieSolid, url: '/visualizations' },
	];
</script>

<aside
	class="shrink-0 w-64 bg-white border-r border-gray-200 dark:bg-gray-800 dark:border-gray-700 overflow-y-auto"
>
	<div class="h-full px-3 py-4 overflow-y-auto">
		<ul class="space-y-2 font-medium">
			{#each menuItems as item (item.url)}
				{@const Icon = item.icon}
				<li>
					<a
						href={`#${item.url}`}
						class="flex items-center p-2 rounded-lg transition-colors duration-200
							{activeUrl === item.url
							? 'bg-gray-100 text-gray-900 dark:bg-gray-700 dark:text-white'
							: 'text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700'}"
						onclick={(e) => {
							e.preventDefault();
							window.location.hash = item.url;
							activeUrl = item.url;
						}}
					>
						<Icon class="w-5 h-5" />
						<span class="ml-3">{item.name}</span>
					</a>
				</li>
			{/each}
		</ul>
	</div>
</aside>
