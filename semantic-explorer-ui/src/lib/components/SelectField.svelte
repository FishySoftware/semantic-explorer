<script lang="ts">
	interface Option {
		value: number | string;
		label: string;
	}

	interface Props {
		id: string;
		label: string;
		value: number | string | null;
		options: Option[];
		placeholder?: string;
		hint?: string;
		required?: boolean;
		disabled?: boolean;
		onchange?: (_newValue: number | string | null) => void;
	}

	let {
		id,
		label,
		value = $bindable(),
		options,
		placeholder = 'Select an option...',
		hint,
		required = false,
		disabled = false,
		onchange,
	}: Props = $props();

	function handleChange(e: Event) {
		const target = e.target as HTMLSelectElement;
		const selectedValue =
			target.value === ''
				? null
				: isNaN(Number(target.value))
					? target.value
					: Number(target.value);
		value = selectedValue;
		onchange?.(selectedValue);
	}
</script>

<div class="mb-4">
	<label for={id} class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
		{label}
		{#if required}
			<span class="text-red-500" aria-label="required">*</span>
		{/if}
	</label>
	<select
		{id}
		value={value ?? ''}
		{required}
		{disabled}
		onchange={handleChange}
		aria-label={label}
		aria-describedby={hint ? `${id}-hint` : undefined}
		class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed outline-none transition-colors"
	>
		<option value="">{placeholder}</option>
		{#each options as option (option.value)}
			<option value={option.value}>{option.label}</option>
		{/each}
	</select>
	{#if hint}
		<p id={`${id}-hint`} class="text-sm text-gray-500 dark:text-gray-400 mt-1">{hint}</p>
	{/if}
</div>
