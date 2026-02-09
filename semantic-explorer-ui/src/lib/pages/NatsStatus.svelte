<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import LoadingState from '../components/LoadingState.svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { createPollingInterval, type PollingController } from '../utils/polling';

	interface NatsStatusResponse {
		connection: {
			state: 'connected' | 'disconnected' | 'pending';
			server_url: string;
		};
		streams: StreamStatus[];
		consumers: ConsumerStatus[];
		dlq: {
			total_messages: number;
			total_bytes: number;
			by_subject: { subject: string; count: number }[];
		};
	}

	interface StreamStatus {
		name: string;
		messages: number;
		bytes: number;
		consumer_count: number;
		first_seq: number;
		last_seq: number;
		first_ts: string | null;
		last_ts: string | null;
		subjects: string[];
		retention: string;
	}

	interface ConsumerStatus {
		name: string;
		stream: string;
		num_pending: number;
		num_ack_pending: number;
		num_waiting: number;
		num_redelivered: number;
		num_delivered: number;
		num_ack_floor: number;
		last_delivered_seq: number;
		ack_floor_seq: number;
	}

	let status = $state<NatsStatusResponse | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let lastUpdated = $state<Date | null>(null);
	let polling: PollingController | null = null;

	async function fetchStatus() {
		try {
			const res = await fetch('/api/status/nats');
			if (!res.ok) {
				throw new Error(`Failed to fetch status: ${res.status}`);
			}
			status = await res.json();
			lastUpdated = new Date();
			error = null;
		} catch (e) {
			const err = e instanceof Error ? e : new Error(String(e));
			error = err.message;
			toastStore.error(formatError(err, 'Failed to fetch Worker status'));
		} finally {
			loading = false;
		}
	}

	onMount(async () => {
		await fetchStatus();
		polling = createPollingInterval(fetchStatus, {
			interval: 5000,
			maxErrors: 10,
			onError: (err) => console.warn('Worker status poll error:', err.message),
		});
	});

	onDestroy(() => {
		polling?.stop();
	});

	function formatBytes(bytes: number): string {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
	}

	function formatNumber(n: number): string {
		return n.toLocaleString();
	}

	function connectionDotColor(state: string): string {
		switch (state) {
			case 'connected':
				return 'bg-green-500';
			case 'disconnected':
				return 'bg-red-500';
			case 'pending':
				return 'bg-yellow-500';
			default:
				return 'bg-gray-500';
		}
	}

	function pendingBadgeColor(pending: number): string {
		if (pending === 0) return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300';
		if (pending < 100)
			return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300';
		return 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300';
	}

	function streamDisplayName(name: string): string {
		return name
			.replace(/_/g, ' ')
			.toLowerCase()
			.replace(/\b\w/g, (c) => c.toUpperCase());
	}

	function consumerDisplayName(name: string): string {
		return name
			.replace(/-/g, ' ')
			.toLowerCase()
			.replace(/\b\w/g, (c) => c.toUpperCase());
	}

	function dlqSubjectLabel(subject: string): string {
		return subject
			.replace('dlq.', '')
			.replace(/-/g, ' ')
			.replace(/\b\w/g, (c) => c.toUpperCase());
	}

	// Separate work queue streams from non-work streams
	function getWorkQueueStreams(streams: StreamStatus[]): StreamStatus[] {
		return streams.filter(
			(s) =>
				s.name === 'COLLECTION_TRANSFORMS' ||
				s.name === 'DATASET_TRANSFORMS' ||
				s.name === 'VISUALIZATION_TRANSFORMS'
		);
	}

	function getSystemStreams(streams: StreamStatus[]): StreamStatus[] {
		return streams.filter(
			(s) =>
				s.name !== 'COLLECTION_TRANSFORMS' &&
				s.name !== 'DATASET_TRANSFORMS' &&
				s.name !== 'VISUALIZATION_TRANSFORMS' &&
				s.name !== 'DLQ_TRANSFORMS'
		);
	}
</script>

<div class="max-w-full xl: mx-auto">
	<!-- Header -->
	<div class="mb-3 lg:mb-6">
		<h1 class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mb-1 lg:mb-2">
			Worker Status
		</h1>
		<p class="text-sm lg:text-base text-gray-600 dark:text-gray-400">
			Message queue monitoring and worker health
			{#if lastUpdated}
				<span class="text-xs text-gray-400 dark:text-gray-500 ml-2">
					&middot; Updated {lastUpdated.toLocaleTimeString()}
				</span>
			{/if}
		</p>
	</div>

	{#if loading && !status}
		<LoadingState message="Loading Worker status..." />
	{:else if error && !status}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4 lg:p-6"
		>
			<p class="text-red-700 dark:text-red-400">{error}</p>
			<button
				onclick={fetchStatus}
				class="mt-3 text-sm text-red-600 hover:text-red-800 dark:text-red-400 underline"
			>
				Retry
			</button>
		</div>
	{:else if status}
		<!-- Connection + DLQ summary bar -->
		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 lg:gap-4 mb-4 lg:mb-6">
			<!-- Connection State -->
			<div
				class="rounded-lg border p-4 lg:p-5 {status.connection.state === 'connected'
					? 'bg-green-50 border-green-200 dark:bg-green-900/20 dark:border-green-800'
					: status.connection.state === 'disconnected'
						? 'bg-red-50 border-red-200 dark:bg-red-900/20 dark:border-red-800'
						: 'bg-yellow-50 border-yellow-200 dark:bg-yellow-900/20 dark:border-yellow-800'}"
			>
				<div
					class="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2"
				>
					Connection
				</div>
				<div class="flex items-center gap-2">
					<span
						class="inline-block w-2.5 h-2.5 rounded-full {connectionDotColor(
							status.connection.state
						)} {status.connection.state === 'connected' ? '' : 'animate-pulse'}"
					></span>
					<span
						class="text-lg font-semibold {status.connection.state === 'connected'
							? 'text-green-700 dark:text-green-300'
							: status.connection.state === 'disconnected'
								? 'text-red-700 dark:text-red-300'
								: 'text-yellow-700 dark:text-yellow-300'}"
					>
						{status.connection.state.charAt(0).toUpperCase() + status.connection.state.slice(1)}
					</span>
				</div>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1.5 font-mono truncate">
					{status.connection.server_url}
				</div>
			</div>

			<!-- Total Streams -->
			<div
				class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4 lg:p-5"
			>
				<div
					class="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2"
				>
					Streams
				</div>
				<div class="text-2xl font-bold text-gray-900 dark:text-white">
					{status.streams.length}
				</div>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1.5">
					{status.streams.reduce((sum, s) => sum + s.messages, 0).toLocaleString()} total messages
				</div>
			</div>

			<!-- Active Consumers -->
			<div
				class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4 lg:p-5"
			>
				<div
					class="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2"
				>
					Consumers
				</div>
				<div class="text-2xl font-bold text-gray-900 dark:text-white">
					{status.consumers.length}
				</div>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1.5">
					{status.consumers
						.reduce((sum: number, c: any) => sum + (c.num_ack_floor ?? 0), 0)
						.toLocaleString()} processed
				</div>
			</div>

			<!-- DLQ Status -->
			<div
				class="rounded-lg border p-4 lg:p-5 {status.dlq.total_messages > 0
					? 'bg-red-50 border-red-300 dark:bg-red-900/20 dark:border-red-800'
					: 'bg-white border-gray-200 dark:bg-gray-800 dark:border-gray-700'}"
			>
				<div
					class="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2"
				>
					Dead Letter Queue
				</div>
				<div
					class="text-2xl font-bold {status.dlq.total_messages > 0
						? 'text-red-600 dark:text-red-400'
						: 'text-gray-900 dark:text-white'}"
				>
					{formatNumber(status.dlq.total_messages)}
				</div>
				<div
					class="text-xs {status.dlq.total_messages > 0
						? 'text-red-500 dark:text-red-400'
						: 'text-gray-500 dark:text-gray-400'} mt-1.5"
				>
					{status.dlq.total_messages > 0
						? formatBytes(status.dlq.total_bytes) + ' of failed messages'
						: 'No failed messages'}
				</div>
			</div>
		</div>

		<!-- DLQ Alert Banner (only when messages present) -->
		{#if status.dlq.total_messages > 0}
			<div
				class="rounded-lg border border-red-300 bg-red-50 dark:bg-red-900/20 dark:border-red-800 p-4 lg:p-5 mb-4 lg:mb-6"
			>
				<div class="flex items-center gap-2 mb-4">
					<svg
						class="w-5 h-5 text-red-600 dark:text-red-400 shrink-0"
						fill="currentColor"
						viewBox="0 0 20 20"
					>
						<path
							fill-rule="evenodd"
							d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
							clip-rule="evenodd"
						/>
					</svg>
					<h2 class="text-base lg:text-lg font-semibold text-red-800 dark:text-red-300">
						Dead Letter Queue Alert
					</h2>
				</div>
				<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
					{#each status.dlq.by_subject as entry (entry.subject)}
						<div
							class="bg-white dark:bg-gray-800 rounded-lg px-4 py-3 border border-red-200 dark:border-red-700"
						>
							<div class="text-xs text-gray-500 dark:text-gray-400 mb-1">
								{dlqSubjectLabel(entry.subject)}
							</div>
							<div
								class="text-xl font-bold {entry.count > 0
									? 'text-red-600 dark:text-red-400'
									: 'text-gray-300 dark:text-gray-600'}"
							>
								{formatNumber(entry.count)}
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Worker Consumers — one card per consumer -->
		<div class="mb-4 lg:mb-6">
			<h2 class="text-lg lg:text-xl font-semibold text-gray-900 dark:text-white mb-3 lg:mb-4">
				Worker Consumers
			</h2>
			<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3 lg:gap-4">
				{#each status.consumers as consumer (consumer.name)}
					{@const isActive = consumer.num_pending > 0 || consumer.num_ack_pending > 0}
					<div
						class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4 lg:p-5"
					>
						<!-- Consumer header -->
						<div class="flex items-start justify-between mb-4">
							<div>
								<h3 class="font-semibold text-gray-900 dark:text-white text-sm lg:text-base">
									{consumerDisplayName(consumer.name)}
								</h3>
								<div class="text-xs text-gray-400 dark:text-gray-500 mt-0.5 font-mono">
									{consumer.stream}
								</div>
							</div>
							<span
								class="text-xs font-medium px-2.5 py-1 rounded-full {pendingBadgeColor(
									consumer.num_pending
								)}"
							>
								{consumer.num_pending === 0 && consumer.num_ack_pending === 0
									? 'Idle'
									: consumer.num_ack_pending > 0 && consumer.num_pending === 0
										? 'Processing'
										: `${formatNumber(consumer.num_pending)} queued`}
							</span>
						</div>

						<!-- Processed count (hero stat) -->
						<div class="bg-green-50 dark:bg-green-900/20 rounded-lg p-3 mb-3">
							<div class="flex items-center justify-between">
								<div>
									<div
										class="text-[11px] font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide"
									>
										Processed
									</div>
									<div class="text-2xl font-bold mt-1 text-green-700 dark:text-green-300">
										{formatNumber(consumer.num_ack_floor)}
									</div>
									<div class="text-[10px] text-gray-400 dark:text-gray-500 mt-0.5">
										Successfully completed
									</div>
								</div>
								<div class="text-right">
									<div
										class="text-[11px] font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide"
									>
										Delivered
									</div>
									<div class="text-lg font-semibold mt-1 text-gray-700 dark:text-gray-300">
										{formatNumber(consumer.num_delivered)}
									</div>
									<div class="text-[10px] text-gray-400 dark:text-gray-500 mt-0.5">
										Total attempts
									</div>
								</div>
							</div>
						</div>

						<!-- Current queue stats -->
						<div class="grid grid-cols-3 gap-3 mb-4">
							<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-3">
								<div
									class="text-[11px] font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide"
								>
									Pending
								</div>
								<div
									class="text-xl font-bold mt-1 {consumer.num_pending > 0
										? 'text-amber-600 dark:text-amber-400'
										: 'text-gray-900 dark:text-white'}"
								>
									{formatNumber(consumer.num_pending)}
								</div>
								<div class="text-[10px] text-gray-400 dark:text-gray-500 mt-0.5">
									Waiting in queue
								</div>
							</div>
							<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-3">
								<div
									class="text-[11px] font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide"
								>
									In-Flight
								</div>
								<div
									class="text-xl font-bold mt-1 {consumer.num_ack_pending > 0
										? 'text-blue-600 dark:text-blue-400'
										: 'text-gray-900 dark:text-white'}"
								>
									{formatNumber(consumer.num_ack_pending)}
								</div>
								<div class="text-[10px] text-gray-400 dark:text-gray-500 mt-0.5">
									Processing now
								</div>
							</div>
							<div class="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-3">
								<div
									class="text-[11px] font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide"
								>
									Redelivered
								</div>
								<div
									class="text-xl font-bold mt-1 {consumer.num_redelivered > 0
										? 'text-orange-600 dark:text-orange-400'
										: 'text-gray-900 dark:text-white'}"
								>
									{formatNumber(consumer.num_redelivered)}
								</div>
								<div class="text-[10px] text-gray-400 dark:text-gray-500 mt-0.5">
									Retried after timeout
								</div>
							</div>
						</div>

						<!-- Progress bar -->
						{#if isActive}
							{@const total = consumer.num_pending + consumer.num_ack_pending}
							{@const ackPct = total > 0 ? (consumer.num_ack_pending / total) * 100 : 0}
							<div>
								<div class="flex justify-between text-[10px] text-gray-400 dark:text-gray-500 mb-1">
									<span>Processing progress</span>
									<span>{Math.round(ackPct)}% in-flight</span>
								</div>
								<div
									class="w-full bg-gray-200 rounded-full h-2 dark:bg-gray-700 overflow-hidden flex"
								>
									<div
										class="bg-blue-500 h-2 transition-all duration-500"
										style="width: {ackPct}%"
									></div>
									<div
										class="bg-amber-400 h-2 transition-all duration-500"
										style="width: {100 - ackPct}%"
									></div>
								</div>
								<div class="flex gap-4 mt-1.5 text-[10px] text-gray-400 dark:text-gray-500">
									<span class="flex items-center gap-1.5">
										<span class="w-2 h-2 rounded-full bg-blue-500 inline-block"></span>
										In-flight ({formatNumber(consumer.num_ack_pending)})
									</span>
									<span class="flex items-center gap-1.5">
										<span class="w-2 h-2 rounded-full bg-amber-400 inline-block"></span>
										Pending ({formatNumber(consumer.num_pending)})
									</span>
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
			{#if status.consumers.length === 0}
				<div
					class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-8 text-center"
				>
					<p class="text-gray-400 dark:text-gray-500">No consumers found</p>
				</div>
			{/if}
		</div>

		<!-- Streams -->
		<div class="grid grid-cols-1 lg:grid-cols-2 gap-3 lg:gap-4">
			<!-- Work Queue Streams -->
			<div
				class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 shadow-sm"
			>
				<div class="px-4 lg:px-5 py-3 lg:py-4 border-b border-gray-200 dark:border-gray-700">
					<h2 class="text-base lg:text-lg font-semibold text-gray-900 dark:text-white">
						Work Queue Streams
					</h2>
					<p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">Transform job queues</p>
				</div>
				<div class="divide-y divide-gray-100 dark:divide-gray-700">
					{#each getWorkQueueStreams(status.streams) as stream (stream.name)}
						<div class="px-4 lg:px-5 py-3 lg:py-4">
							<div class="flex items-center justify-between mb-2">
								<span class="font-medium text-sm text-gray-900 dark:text-white">
									{streamDisplayName(stream.name)}
								</span>
								<span
									class="text-xs px-2 py-0.5 rounded-full bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300"
								>
									{stream.retention}
								</span>
							</div>
							<div class="grid grid-cols-3 gap-3 text-xs">
								<div>
									<span class="text-gray-500 dark:text-gray-400">Messages</span>
									<div
										class="font-semibold font-mono mt-0.5 {stream.messages > 0
											? 'text-amber-600 dark:text-amber-400'
											: 'text-gray-700 dark:text-gray-300'}"
									>
										{formatNumber(stream.messages)}
									</div>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Size</span>
									<div class="font-semibold font-mono mt-0.5 text-gray-700 dark:text-gray-300">
										{formatBytes(stream.bytes)}
									</div>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Consumers</span>
									<div class="font-semibold font-mono mt-0.5 text-gray-700 dark:text-gray-300">
										{stream.consumer_count}
									</div>
								</div>
							</div>
							<div class="flex flex-wrap gap-1 mt-2">
								{#each stream.subjects as subj (subj)}
									<code
										class="text-[10px] bg-gray-100 dark:bg-gray-700 px-1.5 py-0.5 rounded text-gray-500 dark:text-gray-400"
									>
										{subj}
									</code>
								{/each}
							</div>
						</div>
					{/each}
				</div>
			</div>

			<!-- System Streams -->
			<div
				class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 shadow-sm"
			>
				<div class="px-4 lg:px-5 py-3 lg:py-4 border-b border-gray-200 dark:border-gray-700">
					<h2 class="text-base lg:text-lg font-semibold text-gray-900 dark:text-white">
						System Streams
					</h2>
					<p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
						Triggers, status updates, audit &amp; DLQ
					</p>
				</div>
				<div class="divide-y divide-gray-100 dark:divide-gray-700">
					{#each getSystemStreams(status.streams) as stream (stream.name)}
						<div class="px-4 lg:px-5 py-3 lg:py-4">
							<div class="flex items-center justify-between mb-2">
								<span class="font-medium text-sm text-gray-900 dark:text-white">
									{streamDisplayName(stream.name)}
									{#if stream.name === 'DLQ_TRANSFORMS' && status.dlq.total_messages > 0}
										<span class="ml-1.5 inline-block w-2 h-2 rounded-full bg-red-500 animate-pulse"
										></span>
									{/if}
								</span>
								<span
									class="text-xs px-2 py-0.5 rounded-full {stream.retention === 'WorkQueue'
										? 'bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300'
										: 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400'}"
								>
									{stream.retention}
								</span>
							</div>
							<div class="grid grid-cols-3 gap-3 text-xs">
								<div>
									<span class="text-gray-500 dark:text-gray-400">Messages</span>
									<div
										class="font-semibold font-mono mt-0.5 {stream.name === 'DLQ_TRANSFORMS' &&
										stream.messages > 0
											? 'text-red-600 dark:text-red-400'
											: stream.messages > 0
												? 'text-gray-700 dark:text-gray-300'
												: 'text-gray-400 dark:text-gray-500'}"
									>
										{formatNumber(stream.messages)}
									</div>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Size</span>
									<div class="font-semibold font-mono mt-0.5 text-gray-700 dark:text-gray-300">
										{formatBytes(stream.bytes)}
									</div>
								</div>
								<div>
									<span class="text-gray-500 dark:text-gray-400">Consumers</span>
									<div class="font-semibold font-mono mt-0.5 text-gray-700 dark:text-gray-300">
										{stream.consumer_count}
									</div>
								</div>
							</div>
							<div class="flex flex-wrap gap-1 mt-2">
								{#each stream.subjects as subj (subj)}
									<code
										class="text-[10px] bg-gray-100 dark:bg-gray-700 px-1.5 py-0.5 rounded text-gray-500 dark:text-gray-400"
									>
										{subj}
									</code>
								{/each}
							</div>
						</div>
					{/each}
				</div>
			</div>
		</div>

		<!-- DLQ empty indicator -->
		{#if status.dlq.total_messages === 0}
			<div
				class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 px-4 lg:px-5 py-3 lg:py-4 flex items-center gap-2.5 mt-4"
			>
				<svg class="w-4 h-4 text-green-500 shrink-0" fill="currentColor" viewBox="0 0 20 20">
					<path
						fill-rule="evenodd"
						d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
						clip-rule="evenodd"
					/>
				</svg>
				<span class="text-sm text-gray-600 dark:text-gray-400">
					Dead Letter Queue is empty — no failed messages
				</span>
			</div>
		{/if}
	{/if}
</div>
