<script lang="ts">
	interface Props {
		value: string;
		placeholder?: string;
		onchange?: (_newValue: string) => void;
	}

	let { value = $bindable(''), placeholder = 'Search...', onchange }: Props = $props();

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		value = target.value;
		onchange?.(target.value);
	}
</script>

<div class="mb-6">
	<input
		type="text"
		{value}
		oninput={handleInput}
		{placeholder}
		class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
	/>
</div>
