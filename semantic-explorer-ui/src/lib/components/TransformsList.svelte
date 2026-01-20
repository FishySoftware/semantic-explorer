<script lang="ts">
	import { Table, TableBody, TableBodyCell, TableHead, TableHeadCell } from 'flowbite-svelte';
	import ActionMenu from './ActionMenu.svelte';
	import { formatDate } from '$lib/utils/ui-helpers';

	interface Props {
		transforms: any[];
		type?: 'dataset' | 'collection' | 'visualization'; // Transform type
		loading?: boolean;
		onEdit?: (_transform: any) => void;
		onTrigger?: (_transform: any) => void;
		onDelete?: (_transform: any) => void;
		onView?: (_transform: any) => void;
	}

	let {
		transforms = [],
		type = 'dataset',
		loading = false,
		onEdit,
		onTrigger,
		onDelete,
		onView,
	}: Props = $props();

	function getStatusBadge(transform: any): { label: string; color: string } {
		// For dataset transforms, derive status from stats
		if (type === 'dataset') {
			const stats = transform.last_run_stats;
			if (!stats || stats.total_batches_processed === 0) {
				return {
					label: 'Never run',
					color: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
				};
			}

			// Check if currently processing
			if (stats.is_processing || stats.processing_batches > 0) {
				return {
					label: 'Processing',
					color: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200 animate-pulse',
				};
			}

			// If there are failed batches or failed chunks, show partial success or error
			if (stats.failed_batches > 0 || stats.total_chunks_failed > 0) {
				if (stats.successful_batches === 0) {
					return {
						label: 'Failed',
						color: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
					};
				} else {
					return {
						label: 'Partial Success',
						color: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
					};
				}
			}

			return {
				label: 'Success',
				color: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
			};
		}

		// For collection transforms, derive status from stats
		if (type === 'collection') {
			const stats = transform.last_run_stats;
			if (!stats || stats.total_files_processed === 0) {
				return {
					label: 'Never run',
					color: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
				};
			}

			// Check if currently processing (has processing files)
			if (stats.processing_files > 0) {
				return {
					label: 'Processing',
					color: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200 animate-pulse',
				};
			}

			// If there are failed files, show partial success or error
			if (stats.failed_files > 0) {
				if (stats.successful_files === 0) {
					return {
						label: 'Failed',
						color: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
					};
				} else {
					return {
						label: 'Partial Success',
						color: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
					};
				}
			}

			return {
				label: 'Success',
				color: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
			};
		}

		// For other transform types, use last_run_status if available
		if (!transform.last_run_status) {
			return {
				label: 'Never run',
				color: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
			};
		}

		const status = transform.last_run_status.toLowerCase();
		if (status.includes('success')) {
			return {
				label: 'Success',
				color: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
			};
		} else if (status.includes('error')) {
			return { label: 'Error', color: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200' };
		} else if (status.includes('running') || status.includes('pending')) {
			return {
				label: 'Running',
				color: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
			};
		}
		return {
			label: status,
			color: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
		};
	}

	function getStats(transform: any): string {
		if (type === 'dataset') {
			const stats = transform.last_run_stats;
			if (!stats) return 'No runs yet';

			const totalChunks = stats.total_chunks_to_process ?? 0;
			const embedded = stats.total_chunks_embedded ?? 0;

			// If processing, show processing count
			if (stats.is_processing || stats.processing_batches > 0) {
				const processingBatches = stats.processing_batches ?? 0;
				return `${embedded}/${totalChunks} chunks embedded • ${processingBatches} batches processing`;
			}

			const successRate = totalChunks > 0 ? ((embedded / totalChunks) * 100).toFixed(1) : '0';

			return `${embedded}/${totalChunks} chunks (${successRate}% complete) • ${stats.successful_batches ?? 0}/${stats.total_batches_processed ?? 0} batches`;
		} else if (type === 'collection') {
			const stats = transform.last_run_stats;
			if (!stats) return 'No runs yet';

			const totalFiles = (stats.successful_files ?? 0) + (stats.failed_files ?? 0);
			const successRate =
				totalFiles > 0 ? (((stats.successful_files ?? 0) / totalFiles) * 100).toFixed(1) : '0';

			return `${stats.total_items_created ?? 0} items • ${stats.successful_files ?? 0}/${totalFiles} files (${successRate}% success)`;
		} else if (type === 'visualization') {
			const stats = transform.last_run_stats;
			if (!stats) return 'No data';
			return `${stats.n_points ?? 0} points, ${stats.n_clusters ?? 0} clusters`;
		}
		return '';
	}

	function buildActionMenu(transform: any) {
		const actions: any[] = [];

		if (onView) {
			actions.push({
				label: 'View',
				handler: () => onView!(transform),
				icon: 'eye',
			});
		}

		if (onEdit) {
			actions.push({
				label: 'Edit',
				handler: () => onEdit!(transform),
				icon: 'edit',
			});
		}

		if (onTrigger) {
			actions.push({
				label: 'Trigger',
				handler: () => onTrigger!(transform),
				icon: 'play',
			});
		}

		if (onDelete) {
			actions.push({
				label: 'Delete',
				handler: () => onDelete!(transform),
				icon: 'trash',
				dangerous: true,
			});
		}

		return actions;
	}

	function getDetailPageUrl(): string {
		if (type === 'dataset') {
			return '/dataset-transforms';
		} else if (type === 'collection') {
			return '/collection-transforms';
		} else if (type === 'visualization') {
			return '/visualization-transforms';
		}
		return '/';
	}

	function navigateToTransformsList() {
		const url = getDetailPageUrl();
		if (typeof window !== 'undefined') {
			window.location.hash = url;
		}
	}
</script>

{#if loading}
	<div class="flex items-center justify-center h-32">
		<div class="text-center">
			<svg
				class="animate-spin h-8 w-8 text-blue-600 dark:text-blue-400 mx-auto mb-2"
				xmlns="http://www.w3.org/2000/svg"
				fill="none"
				viewBox="0 0 24 24"
			>
				<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
				></circle>
				<path
					class="opacity-75"
					fill="currentColor"
					d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
				></path>
			</svg>
			<p class="text-sm text-gray-600 dark:text-gray-400">Loading transforms...</p>
		</div>
	</div>
{:else if !transforms || transforms.length === 0}
	<div class="text-center py-8 text-gray-500 dark:text-gray-400">
		<p>No transforms yet</p>
	</div>
{:else}
	<Table striped={true} hoverable={true}>
		<TableHead>
			<TableHeadCell class="px-4 py-3">Title</TableHeadCell>
			<TableHeadCell class="px-4 py-3">Status</TableHeadCell>
			<TableHeadCell class="px-4 py-3">Stats</TableHeadCell>
			<TableHeadCell class="px-4 py-3">Last Run</TableHeadCell>
			{#if onEdit || onTrigger || onDelete || onView}
				<TableHeadCell class="px-4 py-3">
					<span class="sr-only">Actions</span>
				</TableHeadCell>
			{/if}
		</TableHead>
		<TableBody>
			{#each transforms as transform (transform[type === 'dataset' ? 'dataset_transform_id' : type === 'collection' ? 'collection_transform_id' : 'visualization_transform_id'])}
				{@const statusBadge = getStatusBadge(transform)}
				<tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
					<TableBodyCell class="px-4 py-3">
						<button
							onclick={navigateToTransformsList}
							class="font-medium text-blue-600 dark:text-blue-400 hover:underline text-left"
							type="button"
						>
							{transform.title}
						</button>
					</TableBodyCell>
					<TableBodyCell class="px-4 py-3">
						<span class="px-2 py-1 text-xs font-semibold rounded-full {statusBadge.color}">
							{statusBadge.label}
						</span>
					</TableBodyCell>
					<TableBodyCell class="px-4 py-3 text-sm text-gray-600 dark:text-gray-400">
						{getStats(transform)}
					</TableBodyCell>
					<TableBodyCell class="px-4 py-3 text-sm text-gray-600 dark:text-gray-400">
						{#if transform.last_run_stats?.last_run_at}
							{formatDate(transform.last_run_stats.last_run_at)}
						{:else}
							Never
						{/if}
					</TableBodyCell>
					{#if onEdit || onTrigger || onDelete || onView}
						<TableBodyCell class="px-4 py-3">
							<ActionMenu actions={buildActionMenu(transform)} />
						</TableBodyCell>
					{/if}
				</tr>
			{/each}
		</TableBody>
	</Table>
{/if}
