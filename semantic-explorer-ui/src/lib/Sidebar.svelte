<script lang="ts">
	import {
		ArrowsRepeatOutline,
		BookOpenSolid,
		BrainSolid,
		CartSolid,
		ChartPieSolid,
		ChevronDownOutline,
		ChevronRightOutline,
		CubeSolid,
		DatabaseSolid,
		FolderSolid,
		GridSolid,
		LayersSolid,
		PaperClipOutline,
		SearchOutline,
	} from 'flowbite-svelte-icons';
	import type { Props } from 'flowbite-svelte-icons/types';
	import type { Component } from 'svelte';

	let { activeUrl = $bindable('/dashboard') } = $props();

	type MenuItem = {
		name: string;
		icon: Component<Props, {}, ''>;
		url?: string;
		children?: MenuItem[];
	};

	const menuItems: MenuItem[] = [
		{ name: 'Dashboard', icon: GridSolid, url: '/dashboard' },
		{ name: 'Documentation', icon: BookOpenSolid, url: '/documentation' },
		{ name: 'Marketplace', icon: CartSolid, url: '/marketplace' },
		{ name: 'Collections', icon: FolderSolid, url: '/collections' },
		{ name: 'Datasets', icon: DatabaseSolid, url: '/datasets' },
		{ name: 'Embedders', icon: BrainSolid, url: '/embedders' },
		{ name: 'Embedded Datasets', icon: CubeSolid, url: '/embedded-datasets' },
		{
			name: 'Transforms',
			icon: ArrowsRepeatOutline,
			children: [
				{
					name: 'Collection Transforms',
					icon: ArrowsRepeatOutline,
					url: '/collection-transforms',
				},
				{ name: 'Dataset Transforms', icon: LayersSolid, url: '/dataset-transforms' },
				{
					name: 'Visualization Transforms',
					icon: ChartPieSolid,
					url: '/visualization-transforms',
				},
			],
		},
		{ name: 'Search', icon: SearchOutline, url: '/search' },
		{ name: 'Visualizations', icon: ChartPieSolid, url: '/visualizations' },
		{ name: 'Chat', icon: PaperClipOutline, url: '/chat' },
	];
	let expandedFolders = $state<string[]>(['Transforms']);

	function toggleFolder(folderName: string) {
		if (expandedFolders.includes(folderName)) {
			expandedFolders = expandedFolders.filter((f) => f !== folderName);
		} else {
			expandedFolders = [...expandedFolders, folderName];
		}
	}
</script>

<aside
	class="shrink-0 w-64 bg-white border-r border-gray-200 dark:bg-gray-800 dark:border-gray-700 overflow-y-auto"
>
	<div class="h-full px-3 py-4 overflow-y-auto">
		<ul class="space-y-2 font-medium">
			{#each menuItems as item (item.url || item.name)}
				{@const Icon = item.icon}
				<li>
					{#if item.children}
						<!-- Folder item with children -->
						<button
							class="flex items-center w-full p-2 rounded-lg transition-colors duration-200 text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700"
							onclick={() => toggleFolder(item.name)}
						>
							<Icon class="w-5 h-5" />
							<span class="ml-3 flex-1 text-left">{item.name}</span>
							{#if expandedFolders.includes(item.name)}
								<ChevronDownOutline class="w-4 h-4" />
							{:else}
								<ChevronRightOutline class="w-4 h-4" />
							{/if}
						</button>

						{#if expandedFolders.includes(item.name)}
							<ul class="ml-6 mt-2 space-y-2">
								{#each item.children as child (child.url)}
									{@const ChildIcon = child.icon}
									<li>
										<a
											href={`#${child.url}`}
											class="flex items-center p-2 rounded-lg transition-colors duration-200
												{activeUrl === child.url
												? 'bg-gray-100 text-gray-900 dark:bg-gray-700 dark:text-white'
												: 'text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700'}"
											onclick={(e) => {
												e.preventDefault();
												window.location.hash = child.url || '';
												activeUrl = child.url || '';
											}}
										>
											<ChildIcon class="w-4 h-4" />
											<span class="ml-3 text-sm">{child.name}</span>
										</a>
									</li>
								{/each}
							</ul>
						{/if}
					{:else}
						<!-- Regular menu item -->
						<a
							href={`#${item.url}`}
							class="flex items-center p-2 rounded-lg transition-colors duration-200
								{activeUrl === item.url
								? 'bg-gray-100 text-gray-900 dark:bg-gray-700 dark:text-white'
								: 'text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700'}"
							onclick={(e) => {
								e.preventDefault();
								window.location.hash = item.url || '';
								activeUrl = item.url || '';
							}}
						>
							<Icon class="w-5 h-5" />
							<span class="ml-3">{item.name}</span>
						</a>
					{/if}
				</li>
			{/each}
		</ul>
	</div>
</aside>
