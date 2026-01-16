<script lang="ts">
	interface Props {
		message: string;
		searchMessage?: string;
		hasSearch?: boolean;
	}

	let { message, searchMessage, hasSearch = false }: Props = $props();
</script>

<div
	class="bg-gray-50 dark:bg-gray-800/50 rounded-lg p-12 text-center border-2 border-dashed border-gray-200 dark:border-gray-700"
>
	<div class="text-gray-400 dark:text-gray-500 mb-3" aria-hidden="true">
		<svg
			class="mx-auto h-12 w-12"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			aria-hidden="true"
		>
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				stroke-width="2"
				d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"
			/>
		</svg>
	</div>
	<p class="text-gray-600 dark:text-gray-400 font-medium">
		{hasSearch && searchMessage ? searchMessage : message}
	</p>
</div>
