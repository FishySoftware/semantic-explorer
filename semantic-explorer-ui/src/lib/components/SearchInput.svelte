<script lang="ts">
	interface Props {
		value: string;
		placeholder?: string;
		label?: string;
		onchange?: (_newValue: string) => void;
	}

	let { value = $bindable(''), placeholder = 'Search...', label, onchange }: Props = $props();

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		value = target.value;
		onchange?.(target.value);
	}
</script>

<div class="mb-6">
	{#if label}
		<label
			for="search-input"
			class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
		>
			{label}
		</label>
	{/if}
	<input
		id="search-input"
		type="text"
		{value}
		oninput={handleInput}
		{placeholder}
		aria-label={label || 'Search'}
		class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white outline-none transition-colors"
	/>
</div>
