<script lang="ts">
	interface TransformProgress {
		transformId: number;
		title: string;
		totalFiles: number;
		processedFiles: number;
		successfulFiles: number;
		failedFiles: number;
	}

	interface Props {
		transforms: TransformProgress[];
		onDismiss?: () => void;
	}

	let { transforms, onDismiss }: Props = $props();

	let totalFilesInCollection = $derived(
		transforms.length > 0 ? Math.max(...transforms.map((t) => t.totalFiles)) : 0
	);

	let overallProcessed = $derived(
		transforms.length > 0 ? Math.max(...transforms.map((t) => t.processedFiles)) : 0
	);

	let overallProgress = $derived(
		totalFilesInCollection > 0
			? Math.min(100, Math.round((overallProcessed / totalFilesInCollection) * 100))
			: 0
	);

	let isComplete = $derived(
		totalFilesInCollection > 0 && overallProcessed >= totalFilesInCollection
	);

	let totalFailed = $derived(transforms.reduce((sum, t) => sum + t.failedFiles, 0));
</script>

<div
	class="mb-4 rounded-lg border overflow-hidden transition-all duration-300 {isComplete
		? 'bg-green-50 dark:bg-green-900/10 border-green-200 dark:border-green-800'
		: 'bg-purple-50 dark:bg-purple-900/10 border-purple-200 dark:border-purple-800'}"
>
	<!-- Progress bar -->
	<div class="h-1.5 bg-gray-200 dark:bg-gray-700">
		<div
			class="h-full transition-all duration-500 ease-out {isComplete
				? 'bg-green-500'
				: 'bg-purple-500'}"
			style="width: {overallProgress}%"
		>
			{#if !isComplete}
				<div class="h-full w-full bg-purple-400/30 animate-pulse"></div>
			{/if}
		</div>
	</div>

	<div class="p-4">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				{#if isComplete}
					<div
						class="flex items-center justify-center w-8 h-8 rounded-full bg-green-100 dark:bg-green-900/30"
					>
						<svg
							class="w-5 h-5 text-green-600 dark:text-green-400"
							fill="none"
							stroke="currentColor"
							viewBox="0 0 24 24"
						>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M5 13l4 4L19 7"
							></path>
						</svg>
					</div>
				{:else}
					<div
						class="flex items-center justify-center w-8 h-8 rounded-full bg-purple-100 dark:bg-purple-900/30"
					>
						<svg
							class="animate-spin w-5 h-5 text-purple-600 dark:text-purple-400"
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
						>
							<circle
								class="opacity-25"
								cx="12"
								cy="12"
								r="10"
								stroke="currentColor"
								stroke-width="4"
							></circle>
							<path
								class="opacity-75"
								fill="currentColor"
								d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
							></path>
						</svg>
					</div>
				{/if}
				<div>
					<h3
						class="text-sm font-semibold {isComplete
							? 'text-green-900 dark:text-green-100'
							: 'text-purple-900 dark:text-purple-100'}"
					>
						{#if isComplete}
							Transform Processing Complete
						{:else}
							Processing Files...
						{/if}
					</h3>
					<p
						class="text-xs {isComplete
							? 'text-green-700 dark:text-green-300'
							: 'text-purple-700 dark:text-purple-300'}"
					>
						{#if isComplete}
							All {totalFilesInCollection} file{totalFilesInCollection !== 1 ? 's' : ''} processed
							{#if totalFailed > 0}
								({totalFailed} failed)
							{/if}
						{:else}
							{overallProcessed} of {totalFilesInCollection} file{totalFilesInCollection !== 1
								? 's'
								: ''} processed ({overallProgress}%)
							{#if totalFailed > 0}
								<span class="text-red-600 dark:text-red-400 ml-1">· {totalFailed} failed</span>
							{/if}
						{/if}
					</p>
				</div>
			</div>

			<div class="flex items-center gap-3">
				{#if !isComplete}
					<span
						class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-full bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300"
					>
						<span class="w-1.5 h-1.5 rounded-full bg-purple-500 animate-pulse"></span>
						Processing
					</span>
				{/if}

				{#if isComplete && onDismiss}
					<button
						onclick={onDismiss}
						class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-1 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
						title="Dismiss"
					>
						<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M6 18L18 6M6 6l12 12"
							></path>
						</svg>
					</button>
				{/if}
			</div>
		</div>

		<!-- Per-transform breakdown (only show if multiple transforms) -->
		{#if transforms.length > 1}
			<div class="mt-3 space-y-2">
				{#each transforms as transform (transform.transformId)}
					{@const progress =
						transform.totalFiles > 0
							? Math.min(100, Math.round((transform.processedFiles / transform.totalFiles) * 100))
							: 0}
					{@const transformComplete =
						transform.totalFiles > 0 && transform.processedFiles >= transform.totalFiles}
					<div class="flex items-center gap-3">
						<span
							class="text-xs font-medium text-gray-600 dark:text-gray-400 w-32 truncate"
							title={transform.title}
						>
							{transform.title}
						</span>
						<div class="flex-1 h-1.5 bg-gray-200 dark:bg-gray-600 rounded-full overflow-hidden">
							<div
								class="h-full rounded-full transition-all duration-500 {transformComplete
									? 'bg-green-500'
									: 'bg-purple-400'}"
								style="width: {progress}%"
							></div>
						</div>
						<span class="text-xs text-gray-500 dark:text-gray-400 w-16 text-right">
							{transform.processedFiles}/{transform.totalFiles}
						</span>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
