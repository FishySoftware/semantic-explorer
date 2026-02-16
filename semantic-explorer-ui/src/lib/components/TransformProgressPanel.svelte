<script lang="ts">
	import { onMount } from 'svelte';

	type ProgressStatus =
		| 'idle'
		| 'pending'
		| 'processing'
		| 'completed'
		| 'completed_with_errors'
		| 'failed';

	interface SubProgress {
		id: number | string;
		name: string;
		status: 'pending' | 'processing' | 'completed' | 'failed';
		itemsProcessed: number;
		totalItems: number;
		detail?: string;
		error?: string;
	}

	interface Props {
		status: ProgressStatus;
		title: string;
		subtitle?: string;
		totalItems?: number;
		processedItems?: number;
		failedItems?: number;
		startedAt?: string;
		subProgresses?: SubProgress[];
		onDismiss?: () => void;
	}

	let {
		status,
		title,
		subtitle,
		totalItems = 0,
		processedItems = 0,
		failedItems = 0,
		startedAt,
		subProgresses = [],
		onDismiss,
	}: Props = $props();

	// Reactive time for elapsed/estimated calculations
	let now = $state(Date.now());

	onMount(() => {
		const interval = setInterval(() => {
			now = Date.now();
		}, 1000);
		return () => clearInterval(interval);
	});

	const isActive = $derived(status === 'processing' || status === 'pending');
	const isComplete = $derived(
		status === 'completed' || status === 'completed_with_errors' || status === 'failed'
	);

	const overallProgress = $derived(
		totalItems > 0 ? Math.min(100, Math.round((processedItems / totalItems) * 100)) : 0
	);

	const elapsedTime = $derived.by(() => {
		if (!startedAt) return 0;
		const start = new Date(startedAt).getTime();
		return Math.floor((now - start) / 1000);
	});

	const estimatedTimeRemaining = $derived.by(() => {
		if (processedItems === 0 || totalItems === 0 || !isActive) return 0;
		const progressRate = processedItems / (elapsedTime || 1);
		const remainingItems = totalItems - processedItems;
		return Math.max(0, Math.floor(remainingItems / progressRate));
	});

	const formatTime = (seconds: number): string => {
		if (seconds < 60) return `${seconds}s`;
		if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
		const hours = Math.floor(seconds / 3600);
		const minutes = Math.floor((seconds % 3600) / 60);
		return `${hours}h ${minutes}m`;
	};

	// Color theming based on status
	const statusConfig = $derived.by(() => {
		switch (status) {
			case 'processing':
				return {
					bg: 'bg-purple-50 dark:bg-purple-900/10',
					border: 'border-purple-200 dark:border-purple-800',
					text: 'text-purple-900 dark:text-purple-100',
					subtext: 'text-purple-700 dark:text-purple-300',
					bar: 'bg-purple-500',
					barBg: 'bg-purple-200 dark:bg-purple-900',
					icon: 'text-purple-600 dark:text-purple-400',
					iconBg: 'bg-purple-100 dark:bg-purple-900/30',
					badge: 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300',
					badgeDot: 'bg-purple-500',
				};
			case 'pending':
				return {
					bg: 'bg-blue-50 dark:bg-blue-900/10',
					border: 'border-blue-200 dark:border-blue-800',
					text: 'text-blue-900 dark:text-blue-100',
					subtext: 'text-blue-700 dark:text-blue-300',
					bar: 'bg-blue-500',
					barBg: 'bg-blue-200 dark:bg-blue-900',
					icon: 'text-blue-600 dark:text-blue-400',
					iconBg: 'bg-blue-100 dark:bg-blue-900/30',
					badge: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
					badgeDot: 'bg-blue-500',
				};
			case 'completed':
				return {
					bg: 'bg-green-50 dark:bg-green-900/10',
					border: 'border-green-200 dark:border-green-800',
					text: 'text-green-900 dark:text-green-100',
					subtext: 'text-green-700 dark:text-green-300',
					bar: 'bg-green-500',
					barBg: 'bg-green-200 dark:bg-green-900',
					icon: 'text-green-600 dark:text-green-400',
					iconBg: 'bg-green-100 dark:bg-green-900/30',
					badge: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
					badgeDot: 'bg-green-500',
				};
			case 'completed_with_errors':
				return {
					bg: 'bg-yellow-50 dark:bg-yellow-900/10',
					border: 'border-yellow-200 dark:border-yellow-800',
					text: 'text-yellow-900 dark:text-yellow-100',
					subtext: 'text-yellow-700 dark:text-yellow-300',
					bar: 'bg-yellow-500',
					barBg: 'bg-yellow-200 dark:bg-yellow-900',
					icon: 'text-yellow-600 dark:text-yellow-400',
					iconBg: 'bg-yellow-100 dark:bg-yellow-900/30',
					badge: 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300',
					badgeDot: 'bg-yellow-500',
				};
			case 'failed':
				return {
					bg: 'bg-red-50 dark:bg-red-900/10',
					border: 'border-red-200 dark:border-red-800',
					text: 'text-red-900 dark:text-red-100',
					subtext: 'text-red-700 dark:text-red-300',
					bar: 'bg-red-500',
					barBg: 'bg-red-200 dark:bg-red-900',
					icon: 'text-red-600 dark:text-red-400',
					iconBg: 'bg-red-100 dark:bg-red-900/30',
					badge: 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
					badgeDot: 'bg-red-500',
				};
			default:
				return {
					bg: 'bg-gray-50 dark:bg-gray-900/10',
					border: 'border-gray-200 dark:border-gray-800',
					text: 'text-gray-900 dark:text-gray-100',
					subtext: 'text-gray-700 dark:text-gray-300',
					bar: 'bg-gray-500',
					barBg: 'bg-gray-200 dark:bg-gray-900',
					icon: 'text-gray-600 dark:text-gray-400',
					iconBg: 'bg-gray-100 dark:bg-gray-900/30',
					badge: 'bg-gray-100 dark:bg-gray-900/30 text-gray-700 dark:text-gray-300',
					badgeDot: 'bg-gray-500',
				};
		}
	});

	const statusLabel = $derived.by(() => {
		switch (status) {
			case 'processing':
				return 'Processing...';
			case 'pending':
				return 'Pending';
			case 'completed':
				return 'Complete';
			case 'completed_with_errors':
				return 'Completed with Errors';
			case 'failed':
				return 'Failed';
			default:
				return 'Idle';
		}
	});

	const completedSubs = $derived(subProgresses.filter((s) => s.status === 'completed').length);
	const processingSubs = $derived(subProgresses.filter((s) => s.status === 'processing').length);
	const failedSubs = $derived(subProgresses.filter((s) => s.status === 'failed').length);
</script>

<div
	class="mb-6 rounded-lg border overflow-hidden transition-all duration-300 {statusConfig.bg} {statusConfig.border}"
>
	<!-- Progress bar -->
	<div class="h-1.5 {statusConfig.barBg}">
		{#if status === 'pending'}
			<div class="h-full {statusConfig.bar} animate-pulse" style="width: 100%"></div>
		{:else if totalItems > 0}
			<div
				class="h-full {statusConfig.bar} transition-all duration-500 ease-out"
				style="width: {overallProgress}%"
			>
				{#if isActive}
					<div class="h-full w-full opacity-30 animate-pulse"></div>
				{/if}
			</div>
		{:else if isActive}
			<div class="h-full {statusConfig.bar} animate-pulse" style="width: 100%"></div>
		{/if}
	</div>

	<div class="p-4">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<!-- Status icon -->
				<div class="flex items-center justify-center w-8 h-8 rounded-full {statusConfig.iconBg}">
					{#if isActive}
						<svg
							class="animate-spin w-5 h-5 {statusConfig.icon}"
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
					{:else if status === 'completed'}
						<svg
							class="w-5 h-5 {statusConfig.icon}"
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
					{:else if status === 'completed_with_errors'}
						<svg
							class="w-5 h-5 {statusConfig.icon}"
							fill="none"
							stroke="currentColor"
							viewBox="0 0 24 24"
						>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
							></path>
						</svg>
					{:else if status === 'failed'}
						<svg
							class="w-5 h-5 {statusConfig.icon}"
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
					{/if}
				</div>
				<div>
					<h3 class="text-sm font-semibold {statusConfig.text}">
						{title}
						{#if !isActive && !isComplete}
							— {statusLabel}
						{:else}
							— {statusLabel}
						{/if}
					</h3>
					<p class="text-xs {statusConfig.subtext}">
						{#if subtitle}
							{subtitle}
						{/if}
						{#if totalItems > 0}
							{#if subtitle}&middot;{/if}
							{processedItems} of {totalItems} items ({overallProgress}%)
							{#if failedItems > 0}
								<span class="text-red-600 dark:text-red-400 ml-1"
									>&middot; {failedItems} failed</span
								>
							{/if}
						{/if}
						{#if isActive && startedAt}
							&middot; Elapsed: {formatTime(elapsedTime)}
							{#if estimatedTimeRemaining > 0}
								&middot; ~{formatTime(estimatedTimeRemaining)} remaining
							{/if}
						{/if}
					</p>
				</div>
			</div>

			<div class="flex items-center gap-3">
				{#if isActive}
					<span
						class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-full {statusConfig.badge}"
					>
						<span class="w-1.5 h-1.5 rounded-full {statusConfig.badgeDot} animate-pulse"></span>
						{status === 'pending' ? 'Pending' : 'Processing'}
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

		<!-- Main progress bar (if we have items to track) -->
		{#if totalItems > 0 && (isActive || isComplete)}
			<div class="mt-3">
				<div class="w-full {statusConfig.barBg} rounded-full h-2.5 overflow-hidden">
					<div
						class="bg-linear-to-r from-purple-500 to-purple-600 h-2.5 rounded-full transition-all duration-300 ease-out {statusConfig.bar}"
						style="width: {overallProgress}%"
					></div>
				</div>
			</div>
		{/if}

		<!-- Sub-progresses (per-embedder, per-file, per-run) -->
		{#if subProgresses.length > 0}
			<div
				class="mt-3 max-h-64 overflow-y-auto border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800/50"
			>
				<div class="divide-y divide-gray-100 dark:divide-gray-700">
					{#each subProgresses as sub (sub.id)}
						{@const subProgress =
							sub.totalItems > 0
								? Math.min(100, Math.round((sub.itemsProcessed / sub.totalItems) * 100))
								: 0}
						<div class="px-4 py-3 text-xs">
							<div class="flex items-center justify-between mb-2">
								<div class="flex items-center gap-2">
									{#if sub.status === 'processing'}
										<svg
											class="h-4 w-4 text-purple-600 animate-spin"
											fill="none"
											viewBox="0 0 24 24"
										>
											<circle
												cx="12"
												cy="12"
												r="10"
												stroke="currentColor"
												stroke-width="2"
												fill="none"
											></circle>
											<path stroke-linecap="round" d="M12 2a10 10 0 010 20"></path>
										</svg>
									{:else if sub.status === 'completed'}
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
									{:else if sub.status === 'failed'}
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
									{:else}
										<svg
											class="h-4 w-4 text-gray-400"
											fill="none"
											stroke="currentColor"
											viewBox="0 0 24 24"
										>
											<circle
												cx="12"
												cy="12"
												r="10"
												stroke="currentColor"
												stroke-width="2"
												fill="none"
											></circle>
										</svg>
									{/if}
									<span class="font-medium text-gray-900 dark:text-white">{sub.name}</span>
								</div>
								<span
									class="px-2 py-0.5 rounded text-xs font-medium {sub.status === 'processing'
										? 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300'
										: sub.status === 'completed'
											? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
											: sub.status === 'failed'
												? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
												: 'bg-gray-100 dark:bg-gray-900/30 text-gray-700 dark:text-gray-300'}"
								>
									{sub.status}
								</span>
							</div>
							<div
								class="flex items-center justify-between text-xs mb-1 text-gray-600 dark:text-gray-400"
							>
								<span>{sub.itemsProcessed} / {sub.totalItems} items</span>
								{#if sub.detail}
									<span>{sub.detail}</span>
								{/if}
							</div>
							<div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5 overflow-hidden">
								<div
									class="bg-linear-to-r from-purple-500 to-purple-600 h-1.5 rounded-full transition-all duration-200"
									style="width: {subProgress}%"
								></div>
							</div>
							{#if sub.error}
								<p class="text-red-600 dark:text-red-400 mt-1">{sub.error}</p>
							{/if}
						</div>
					{/each}
				</div>
			</div>

			<!-- Sub-progress summary -->
			<div class="mt-3 grid grid-cols-3 gap-2">
				<div
					class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
				>
					<p class="text-xs text-gray-500 dark:text-gray-400">Processing</p>
					<p class="text-lg font-semibold text-purple-600 dark:text-purple-400">
						{processingSubs}
					</p>
				</div>
				<div
					class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
				>
					<p class="text-xs text-gray-500 dark:text-gray-400">Completed</p>
					<p class="text-lg font-semibold text-green-600 dark:text-green-400">{completedSubs}</p>
				</div>
				<div
					class="text-center p-2 bg-white dark:bg-gray-800/50 rounded border border-gray-200 dark:border-gray-700"
				>
					<p class="text-xs text-gray-500 dark:text-gray-400">Failed</p>
					<p class="text-lg font-semibold text-red-600 dark:text-red-400">{failedSubs}</p>
				</div>
			</div>
		{/if}
	</div>
</div>
