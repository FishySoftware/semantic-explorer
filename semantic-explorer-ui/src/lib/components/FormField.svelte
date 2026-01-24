<script lang="ts">
	interface Props {
		id: string;
		label: string;
		type?: 'text' | 'number' | 'email' | 'password';
		value: string | number | null;
		placeholder?: string;
		hint?: string;
		min?: number;
		max?: number;
		required?: boolean;
		disabled?: boolean;
		error?: string;
		onchange?: (_newValue: string | number | null) => void;
	}

	let {
		id,
		label,
		type = 'text',
		value = $bindable(),
		placeholder = '',
		hint,
		min,
		max,
		required = false,
		disabled = false,
		error,
		onchange,
	}: Props = $props();

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		if (type === 'number') {
			const numValue = target.value === '' ? null : Number(target.value);
			value = numValue;
			onchange?.(numValue);
		} else {
			value = target.value;
			onchange?.(target.value);
		}
	}

	function isInvalid(): boolean {
		if (error) return true;
		if (required && (value === '' || value === null)) return true;
		return false;
	}
</script>

<div class="mb-4">
	<label for={id} class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
		{label}
		{#if required}
			<span class="text-red-500" aria-label="required">*</span>
		{/if}
	</label>
	<input
		{id}
		{type}
		value={value ?? ''}
		{placeholder}
		{min}
		{max}
		{required}
		{disabled}
		oninput={handleInput}
		aria-label={label}
		aria-invalid={isInvalid()}
		aria-describedby={hint ? `${id}-hint` : undefined}
		aria-errormessage={error ? `${id}-error` : undefined}
		class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed outline-none transition-colors {isInvalid()
			? 'border-red-500 focus:ring-red-500'
			: ''}"
	/>
	{#if hint}
		<p id={`${id}-hint`} class="text-sm text-gray-500 dark:text-gray-400 mt-1">{hint}</p>
	{/if}
	{#if error}
		<p id={`${id}-error`} class="text-sm text-red-600 dark:text-red-400 mt-1">{error}</p>
	{/if}
</div>
