<script lang="ts">
	interface StatItem {
		label: string;
		value: number | string;
		color?: 'default' | 'green' | 'red' | 'blue' | 'purple' | 'yellow';
	}

	interface Props {
		stats: StatItem[];
		columns?: 3 | 4 | 5 | 6;
	}

	let { stats, columns = 4 }: Props = $props();

	const colorClasses: Record<string, string> = {
		default: 'text-gray-900 dark:text-white',
		green: 'text-green-600 dark:text-green-400',
		red: 'text-red-600 dark:text-red-400',
		blue: 'text-blue-600 dark:text-blue-400',
		purple: 'text-purple-600 dark:text-purple-400',
		yellow: 'text-yellow-600 dark:text-yellow-400',
	};

	const gridColsClass: Record<number, string> = {
		3: 'grid-cols-3',
		4: 'grid-cols-4',
		5: 'grid-cols-5',
		6: 'grid-cols-6',
	};
</script>

<div class="grid {gridColsClass[columns]} gap-4">
	{#each stats as stat (stat.label)}
		<div>
			<p class="text-sm text-gray-600 dark:text-gray-400">{stat.label}</p>
			<p class="text-lg font-semibold {colorClasses[stat.color || 'default']}">
				{stat.value}
			</p>
		</div>
	{/each}
</div>
