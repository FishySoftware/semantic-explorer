<script lang="ts">
	interface Props {
		value: string;
		disabled: boolean;
		onSend: () => void;
		onKeyDown?: (_e: KeyboardEvent) => void;
		placeholder?: string;
	}

	let { value = $bindable(), disabled, onSend, onKeyDown, placeholder }: Props = $props();

	function handleKeyDown(e: KeyboardEvent) {
		if (onKeyDown) {
			onKeyDown(e);
		}
		// Handle Ctrl+Enter to send
		if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
			e.preventDefault();
			onSend();
		}
	}

	function handleSubmit() {
		if (!value.trim() || disabled) return;
		onSend();
	}
</script>

<div class="flex gap-3">
	<textarea
		bind:value
		onkeydown={handleKeyDown}
		placeholder={placeholder || 'Type your message... (Enter to send, Ctrl+Enter to send)'}
		{disabled}
		class="flex-1 px-4 py-3 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 resize-none disabled:opacity-50"
		rows="1"
	></textarea>
	<button
		onclick={handleSubmit}
		disabled={!value.trim() || disabled}
		class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
	>
		Send
	</button>
</div>

<style>
	/* Auto-resize textarea */
	:global(textarea[rows='1']) {
		overflow: hidden;
	}
</style>
