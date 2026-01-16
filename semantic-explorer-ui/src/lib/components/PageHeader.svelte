<script lang="ts">
	import { InfoCircleSolid } from 'flowbite-svelte-icons';
	let { title, description }: { title: string; description: string } = $props();
</script>

<div
	class="mb-6 rounded-lg border border-blue-200 bg-linear-to-r from-blue-50 to-indigo-50 p-5 dark:border-blue-900/50 dark:from-blue-900/20 dark:to-indigo-900/20 animate-fadeIn"
	role="region"
	aria-label="Page information"
>
	<div class="flex items-start gap-3">
		<div class="shrink-0 mt-0.5">
			<InfoCircleSolid class="w-5 h-5 text-blue-600 dark:text-blue-400" aria-hidden="true" />
		</div>
		<div class="flex-1">
			<h1 class="text-lg font-semibold text-gray-900 dark:text-white mb-1">
				{title}
			</h1>
			<p class="text-sm text-gray-700 dark:text-gray-300 leading-relaxed">
				{description}
			</p>
		</div>
	</div>
</div>
