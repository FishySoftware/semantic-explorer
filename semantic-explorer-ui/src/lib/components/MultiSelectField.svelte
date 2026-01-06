<script lang="ts">
	interface Option {
		value: number | string;
		label: string;
		description?: string;
	}

	interface Props {
		id: string;
		label: string;
		selectedValues: (number | string)[];
		options: Option[];
		hint?: string;
		required?: boolean;
		disabled?: boolean;
		onchange?: (_newValues: (number | string)[]) => void;
	}

	let {
		id,
		label,
		selectedValues = $bindable([]),
		options,
		hint,
		required = false,
		disabled = false,
		onchange,
	}: Props = $props();

	function toggleValue(optionValue: number | string) {
		if (disabled) return;

		if (selectedValues.includes(optionValue)) {
			selectedValues = selectedValues.filter((v) => v !== optionValue);
		} else {
			selectedValues = [...selectedValues, optionValue];
		}
		onchange?.(selectedValues);
	}

	function isSelected(optionValue: number | string): boolean {
		return selectedValues.includes(optionValue);
	}
</script>

<div class="mb-4">
	<label for={id} class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
		{label}
		{#if required}
			<span class="text-red-500">*</span>
		{/if}
		{#if selectedValues.length > 0}
			<span class="ml-2 text-xs text-gray-500 dark:text-gray-400">
				({selectedValues.length} selected)
			</span>
		{/if}
	</label>
	<div
		{id}
		class="border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 max-h-48 overflow-y-auto"
		class:opacity-50={disabled}
		class:cursor-not-allowed={disabled}
	>
		{#each options as option (option.value)}
			<button
				type="button"
				onclick={() => toggleValue(option.value)}
				{disabled}
				class="w-full px-3 py-2 text-left hover:bg-gray-100 dark:hover:bg-gray-600 transition-colors flex items-center justify-between border-b border-gray-200 dark:border-gray-600 last:border-b-0 disabled:cursor-not-allowed"
				class:bg-blue-50={isSelected(option.value)}
				class:dark:bg-blue-900={isSelected(option.value)}
			>
				<div>
					<span class="text-gray-900 dark:text-white">{option.label}</span>
					{#if option.description}
						<span class="ml-2 text-xs text-gray-500 dark:text-gray-400">
							{option.description}
						</span>
					{/if}
				</div>
				{#if isSelected(option.value)}
					<span class="text-blue-600 dark:text-blue-400">✓</span>
				{/if}
			</button>
		{/each}
		{#if options.length === 0}
			<p class="px-3 py-2 text-gray-500 dark:text-gray-400 text-sm">No options available</p>
		{/if}
	</div>
	{#if hint}
		<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{hint}</p>
	{/if}
</div>
