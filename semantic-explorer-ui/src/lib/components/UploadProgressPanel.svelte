<script lang="ts">
	interface FileStatus {
		name: string;
		status: 'pending' | 'uploading' | 'completed' | 'failed';
		progress: number; // 0-100
		error?: string;
	}

	interface Props {
		uploadProgress: {
			completed: number;
			total: number;
			currentBatch: number;
			totalBatches: number;
		} | null;
		fileStatuses?: FileStatus[];
		isUploading: boolean;
		onDismiss?: () => void;
	}

	let { uploadProgress, fileStatuses = [], isUploading, onDismiss }: Props = $props();

	// Calculate overall metrics
	const overallProgress = $derived(
		uploadProgress ? Math.round((uploadProgress.completed / uploadProgress.total) * 100) : 0
	);

	const completedFiles = $derived(fileStatuses.filter((f) => f.status === 'completed').length);
	const failedFiles = $derived(fileStatuses.filter((f) => f.status === 'failed').length);
	const uploadingFiles = $derived(fileStatuses.filter((f) => f.status === 'uploading').length);
	const pendingFiles = $derived(fileStatuses.filter((f) => f.status === 'pending').length);
</script>

<div
	class="mt-4 p-4 bg-linear-to-br from-blue-50 to-blue-25 dark:from-blue-900/20 dark:to-blue-900/10 border border-blue-200 dark:border-blue-800 rounded-lg"
>
	<!-- Header with title and overall progress -->
	<div class="flex items-center justify-between mb-4">
		<div class="flex items-center gap-3">
			<div class="shrink-0">
				{#if isUploading}
					<div class="relative inline-flex">
						<svg
							class="animate-spin h-6 w-6 text-blue-600"
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
				{:else}
					<svg class="h-6 w-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"
						></path>
					</svg>
				{/if}
			</div>
			<div>
				<h3 class="text-sm font-semibold text-blue-900 dark:text-blue-100">
					{isUploading
						? 'Uploading Files'
						: failedFiles > 0
							? 'Upload Completed with Errors'
							: 'Upload Complete'}
				</h3>
				{#if uploadProgress}
					<p class="text-xs text-blue-700 dark:text-blue-300">
						Batch {uploadProgress.currentBatch} of {uploadProgress.totalBatches}
					</p>
				{/if}
			</div>
		</div>

		<!-- Dismiss button (shown after upload with failures) -->
		{#if !isUploading && failedFiles > 0 && onDismiss}
			<button
				onclick={onDismiss}
				class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
				title="Dismiss"
			>
				<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M6 18L18 6M6 6l12 12"
					></path>
				</svg>
			</button>
		{/if}

		<!-- Summary stats -->
		<div class="text-right">
			{#if uploadProgress}
				<p class="text-sm font-medium text-blue-900 dark:text-blue-100">
					{uploadProgress.completed} / {uploadProgress.total}
				</p>
				<p class="text-xs text-blue-700 dark:text-blue-300">{overallProgress}%</p>
			{/if}
		</div>
	</div>

	<!-- Main progress bar -->
	<div class="mb-4">
		<div class="w-full bg-blue-200 dark:bg-blue-900 rounded-full h-3 overflow-hidden">
			<div
				class="bg-linear-to-r from-blue-500 to-blue-600 h-3 rounded-full transition-all duration-300 ease-out shadow-lg"
				style="width: {overallProgress}%"
			></div>
		</div>
		<div class="mt-1 flex justify-between text-xs text-blue-700 dark:text-blue-300">
			<span>{overallProgress}% complete</span>
			<span>{completedFiles} completed, {failedFiles > 0 ? failedFiles + ' failed' : ''}</span>
		</div>
	</div>

	<!-- File status cards (scrollable) -->
	{#if fileStatuses.length > 0}
		<div
			class="mb-3 max-h-48 overflow-y-auto border border-blue-200 dark:border-blue-800 rounded bg-white dark:bg-gray-800/50"
		>
			<div class="divide-y divide-blue-100 dark:divide-blue-900">
				{#each fileStatuses as file (file.name)}
					<div
						class="px-3 py-2 flex items-center gap-3 text-xs {file.status === 'completed'
							? 'bg-green-50 dark:bg-green-900/10'
							: file.status === 'failed'
								? 'bg-red-50 dark:bg-red-900/10'
								: ''}"
					>
						<!-- Status icon -->
						<div class="shrink-0">
							{#if file.status === 'completed'}
								<svg
									class="h-4 w-4 text-green-600"
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
							{:else if file.status === 'failed'}
								<svg
									class="h-4 w-4 text-red-600"
									fill="none"
									stroke="currentColor"
									viewBox="0 0 24 24"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M6 18L18 6M6 6l12 12"
									></path>
								</svg>
							{:else if file.status === 'uploading'}
								<svg
									class="h-4 w-4 text-blue-600 animate-spin"
									fill="none"
									stroke="currentColor"
									viewBox="0 0 24 24"
								>
									<circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" fill="none"
									></circle>
									<path stroke-linecap="round" d="M12 2a10 10 0 010 20"></path>
								</svg>
							{:else}
								<svg
									class="h-4 w-4 text-gray-400"
									fill="none"
									stroke="currentColor"
									viewBox="0 0 24 24"
								>
									<circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" fill="none"
									></circle>
								</svg>
							{/if}
						</div>

						<!-- File details -->
						<div class="flex-1 min-w-0">
							<p class="text-gray-900 dark:text-white truncate font-medium">{file.name}</p>
							{#if file.error}
								<p class="text-red-600 dark:text-red-400 text-wrap wrap-break-word">{file.error}</p>
							{:else if file.status === 'uploading'}
								<p class="text-blue-600 dark:text-blue-400">{file.progress}%</p>
							{:else if file.status === 'pending'}
								<p class="text-gray-500 dark:text-gray-400">Waiting...</p>
							{:else}
								<p class="text-green-600 dark:text-green-400">Done</p>
							{/if}
						</div>

						<!-- Progress bar for individual files -->
						{#if file.status === 'uploading'}
							<div class="w-16 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
								<div
									class="h-full bg-blue-600 rounded-full transition-all duration-200"
									style="width: {file.progress}%"
								></div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		</div>

		<!-- Status summary -->
		<div class="grid grid-cols-4 gap-2">
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Completed</p>
				<p class="text-lg font-semibold text-green-600 dark:text-green-400">{completedFiles}</p>
			</div>
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Uploading</p>
				<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">{uploadingFiles}</p>
			</div>
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Pending</p>
				<p class="text-lg font-semibold text-gray-600 dark:text-gray-400">{pendingFiles}</p>
			</div>
			<div
				class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
			>
				<p class="text-xs text-gray-500 dark:text-gray-400">Failed</p>
				<p class="text-lg font-semibold text-red-600 dark:text-red-400">{failedFiles}</p>
			</div>
		</div>
	{/if}
</div>

<style>
	/* Smooth animations */
	:global(.animate-fadeIn) {
		animation: fadeIn 0.3s ease-in;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateY(-10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
