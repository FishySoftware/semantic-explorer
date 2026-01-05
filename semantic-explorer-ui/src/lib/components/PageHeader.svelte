<script lang="ts">
	import { InfoCircleSolid } from 'flowbite-svelte-icons';
	let { title, description }: { title: string; description: string } = $props();
</script>

<div
	class="mb-6 rounded-lg border border-gray-200 bg-linear-to-r from-blue-50 to-indigo-50 p-5 dark:border-gray-700 dark:from-gray-800 dark:to-gray-700"
>
	<div class="flex items-start gap-3">
		<div class="shrink-0 mt-0.5">
			<InfoCircleSolid class="w-5 h-5 text-blue-600 dark:text-blue-400" />
		</div>
		<div class="flex-1">
			<h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-1">
				{title}
			</h2>
			<p class="text-sm text-gray-700 dark:text-gray-300 leading-relaxed">
				{description}
			</p>
		</div>
	</div>
</div>
