<script lang="ts">
	interface Props {
		message: string;
		searchMessage?: string;
		hasSearch?: boolean;
	}

	let { message, searchMessage, hasSearch = false }: Props = $props();
</script>

<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
	<p class="text-gray-600 dark:text-gray-400">
		{hasSearch && searchMessage ? searchMessage : message}
	</p>
</div>
