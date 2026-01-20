<script lang="ts">
	interface StatItem {
		label: string;
		value: number | string;
		color?: 'default' | 'green' | 'red' | 'blue' | 'purple' | 'yellow';
		icon?: string;
		trend?: 'up' | 'down' | 'neutral';
	}

	interface Props {
		stats: StatItem[];
		columns?: 3 | 4 | 5 | 6;
	}

	let { stats, columns = 4 }: Props = $props();

	const colorClasses: Record<string, string> = {
		default:
			'text-gray-900 dark:text-white bg-gray-50 dark:bg-gray-800/50 border-gray-200 dark:border-gray-700',
		green:
			'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/10 border-green-200 dark:border-green-800/50',
		red: 'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/10 border-red-200 dark:border-red-800/50',
		blue: 'text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/10 border-blue-200 dark:border-blue-800/50',
		purple:
			'text-purple-600 dark:text-purple-400 bg-purple-50 dark:bg-purple-900/10 border-purple-200 dark:border-purple-800/50',
		yellow:
			'text-yellow-600 dark:text-yellow-400 bg-yellow-50 dark:bg-yellow-900/10 border-yellow-200 dark:border-yellow-800/50',
	};

	const gridColsClass: Record<number, string> = {
		3: 'md:grid-cols-3',
		4: 'md:grid-cols-4',
		5: 'md:grid-cols-5',
		6: 'md:grid-cols-6',
	};
</script>

<div class="grid grid-cols-1 sm:grid-cols-2 {gridColsClass[columns]} gap-4">
	{#each stats as stat (stat.label)}
		<div
			class="border rounded-lg p-4 transition-all duration-200 hover:shadow-md card-hover {colorClasses[
				stat.color || 'default'
			]}"
		>
			<div class="flex items-start justify-between">
				<div class="flex-1">
					<p class="text-xs font-medium text-gray-600 dark:text-gray-400 uppercase tracking-wider">
						{stat.label}
					</p>
					<p
						class="text-2xl font-bold mt-2 {stat.color === 'default'
							? 'text-gray-900 dark:text-white'
							: ''}"
					>
						{stat.value}
					</p>
				</div>
				{#if stat.icon}
					<div class="text-2xl opacity-20">{stat.icon}</div>
				{/if}
			</div>
			{#if stat.trend}
				<div class="mt-2 text-xs font-medium flex items-center gap-1">
					{#if stat.trend === 'up'}
						<span class="text-green-600 dark:text-green-400">↑ Increasing</span>
					{:else if stat.trend === 'down'}
						<span class="text-red-600 dark:text-red-400">↓ Decreasing</span>
					{:else}
						<span class="text-gray-600 dark:text-gray-400">→ Neutral</span>
					{/if}
				</div>
			{/if}
		</div>
	{/each}
</div>
