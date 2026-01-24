<script lang="ts">
	interface Props {
		rows?: number;
		variant?: 'card' | 'list' | 'text' | 'avatar';
		height?: string;
		width?: string;
		className?: string;
	}

	let { rows = 3, variant = 'card', height, width, className = '' }: Props = $props();

	function getBaseClasses(): string {
		const base = 'animate-pulse bg-gray-200 dark:bg-gray-700 rounded';
		return className ? `${base} ${className}` : base;
	}
</script>

{#if variant === 'card'}
	<div class="{getBaseClasses()} p-4">
		<div class="h-4 bg-gray-300 dark:bg-gray-600 rounded mb-3"></div>
		<div class="h-4 bg-gray-300 dark:bg-gray-600 rounded mb-2 w-3/4"></div>
		<div class="h-4 bg-gray-300 dark:bg-gray-600 rounded w-1/2"></div>
	</div>
{:else if variant === 'list'}
	<div class="space-y-2">
		{#each Array(rows) as _, i (i)}
			<div class="{getBaseClasses()} h-16 flex items-center px-4">
				<div class="h-8 w-8 bg-gray-300 dark:bg-gray-600 rounded-full mr-3"></div>
				<div class="flex-1">
					<div class="h-4 bg-gray-300 dark:bg-gray-600 rounded mb-2 w-1/3"></div>
					<div class="h-3 bg-gray-300 dark:bg-gray-600 rounded w-2/3"></div>
				</div>
			</div>
		{/each}
	</div>
{:else if variant === 'text'}
	<div class="space-y-2">
		{#each Array(rows) as _, i (i)}
			<div class="{getBaseClasses()} h-4" style="width: {width || '100%'}"></div>
		{/each}
	</div>
{:else if variant === 'avatar'}
	<div
		class="{getBaseClasses()} flex items-center justify-center"
		style="width: {width || '40px'}; height: {height || '40px'}"
	></div>
{/if}
