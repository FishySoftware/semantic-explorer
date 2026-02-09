<script lang="ts">
	import {
		ArrowsRepeatOutline,
		BookOpenSolid,
		BrainSolid,
		CartSolid,
		ChartPieSolid,
		ChevronDownOutline,
		ChevronRightOutline,
		DatabaseSolid,
		FolderSolid,
		GridSolid,
		LayersSolid,
		MessageDotsOutline,
		PaperClipOutline,
		SearchOutline,
		ServerOutline,
	} from 'flowbite-svelte-icons';
	import type { Props } from 'flowbite-svelte-icons/types';
	import type { Component } from 'svelte';

	let { activeUrl = $bindable('/datasets') } = $props();

	type MenuItem = {
		name: string;
		icon: Component<Props, {}, ''>;
		url?: string;
		children?: MenuItem[];
		isDivider?: boolean;
	};

	const menuItems: MenuItem[] = [
		{ name: 'Dashboard', icon: GridSolid, url: '/dashboard' },
		{ name: 'Documentation', icon: BookOpenSolid, url: '/documentation' },
		{ name: 'Marketplace', icon: CartSolid, url: '/marketplace' },

		{ name: 'divider-1', icon: GridSolid, isDivider: true },

		{ name: 'Collections', icon: FolderSolid, url: '/collections' },
		{ name: 'Datasets', icon: DatabaseSolid, url: '/datasets' },
		{ name: 'Embedded Datasets', icon: LayersSolid, url: '/embedded-datasets' },

		{ name: 'divider-2', icon: GridSolid, isDivider: true },

		{ name: 'Search', icon: SearchOutline, url: '/search' },
		{ name: 'Chat', icon: PaperClipOutline, url: '/chat' },
		{ name: 'Visualizations', icon: ChartPieSolid, url: '/visualizations' },

		{ name: 'divider-3', icon: GridSolid, isDivider: true },

		{ name: 'Embedders', icon: BrainSolid, url: '/embedders' },
		{ name: 'LLMs', icon: MessageDotsOutline, url: '/llms' },

		{ name: 'divider-4', icon: GridSolid, isDivider: true },

		{
			name: 'Transforms',
			icon: ArrowsRepeatOutline,
			children: [
				{
					name: 'Collection Transforms',
					icon: FolderSolid,
					url: '/collection-transforms',
				},
				{
					name: 'Dataset Transforms',
					icon: DatabaseSolid,
					url: '/dataset-transforms',
				},
				{
					name: 'Visualization Transforms',
					icon: ChartPieSolid,
					url: '/visualization-transforms',
				},
			],
		},
		{ name: 'divider-5', icon: GridSolid, isDivider: true },

		{ name: 'Worker Status', icon: ServerOutline, url: '/status/nats' },
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
	class="shrink-0 w-48 lg:w-56 xl:w-64 bg-white border-r border-gray-200 dark:bg-gray-800 dark:border-gray-700 overflow-y-auto"
	role="navigation"
	aria-label="Main navigation"
>
	<div class="h-full px-2 lg:px-3 py-3 lg:py-4 overflow-y-auto">
		<ul
			class="space-y-1 lg:space-y-2 font-medium text-sm lg:text-base"
			role="menubar"
			aria-label="Navigation menu"
		>
			{#each menuItems as item (item.url || item.name)}
				{@const Icon = item.icon}
				{#if item.isDivider}
					<!-- Divider -->
					<li class="py-2">
						<hr class="border-gray-200 dark:border-gray-700" />
					</li>
				{:else if item.children}
					<!-- Folder item with children -->
					<li role="none">
						<button
							class="flex items-center w-full p-1.5 lg:p-2 rounded-lg transition-colors duration-200 text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700"
							onclick={() => toggleFolder(item.name)}
							onkeydown={(e) => {
								if (e.key === 'Enter' || e.key === ' ') {
									e.preventDefault();
									toggleFolder(item.name);
								}
							}}
							aria-expanded={expandedFolders.includes(item.name)}
							aria-label={`${item.name} submenu, ${expandedFolders.includes(item.name) ? 'expanded' : 'collapsed'}`}
						>
							<Icon class="w-4 h-4 lg:w-5 lg:h-5 shrink-0" />
							<span class="ml-2 lg:ml-3 flex-1 text-left">{item.name}</span>
							{#if expandedFolders.includes(item.name)}
								<ChevronDownOutline class="w-3 h-3 lg:w-4 lg:h-4 shrink-0" />
							{:else}
								<ChevronRightOutline class="w-3 h-3 lg:w-4 lg:h-4 shrink-0" />
							{/if}
						</button>

						{#if expandedFolders.includes(item.name)}
							<ul
								class="ml-4 lg:ml-6 mt-1 lg:mt-2 space-y-1 lg:space-y-2"
								role="menu"
								aria-label={`${item.name} submenu`}
							>
								{#each item.children as child (child.url)}
									{@const ChildIcon = child.icon}
									<li role="none">
										<a
											href={`#${child.url}`}
											role="menuitem"
											class="flex items-center p-1.5 lg:p-2 rounded-lg transition-colors duration-200
												{activeUrl === child.url
												? 'bg-gray-100 text-gray-900 dark:bg-gray-700 dark:text-white'
												: 'text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700'}"
											aria-current={activeUrl === child.url ? 'page' : undefined}
											onclick={(e) => {
												e.preventDefault();
												window.location.hash = child.url || '';
												activeUrl = child.url || '';
											}}
										>
											<ChildIcon class="w-3 h-3 lg:w-4 lg:h-4 shrink-0" />
											<span class="ml-2 lg:ml-3 text-xs lg:text-sm">{child.name}</span>
										</a>
									</li>
								{/each}
							</ul>
						{/if}
					</li>
				{:else}
					<!-- Regular menu item -->
					<li role="none">
						<a
							href={`#${item.url}`}
							role="menuitem"
							class="flex items-center p-1.5 lg:p-2 rounded-lg transition-colors duration-200
								{activeUrl === item.url
								? 'bg-gray-100 text-gray-900 dark:bg-gray-700 dark:text-white'
								: 'text-gray-900 hover:bg-gray-100 dark:text-white dark:hover:bg-gray-700'}"
							aria-current={activeUrl === item.url ? 'page' : undefined}
							onclick={(e) => {
								e.preventDefault();
								window.location.hash = item.url || '';
								activeUrl = item.url || '';
							}}
						>
							<Icon class="w-4 h-4 lg:w-5 lg:h-5 shrink-0" />
							<span class="ml-2 lg:ml-3">{item.name}</span>
						</a>
					</li>
				{/if}
			{/each}
		</ul>
	</div>
</aside>
