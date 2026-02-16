<script lang="ts">
	import { formatDate } from '../utils/ui-helpers';
	import StatusBadge from './StatusBadge.svelte';

	type CellRenderer = 'text' | 'status' | 'date' | 'duration' | 'number' | 'link' | 'custom';

	interface ColumnDef {
		key: string;
		label: string;
		sortable?: boolean;
		renderer?: CellRenderer;
		/** For link renderer: function to get the href */
		getHref?: (_row: any) => string;
		/** For link renderer: function for navigation callback */
		onLinkClick?: (_row: any) => void;
		/** Map a raw value to a StatusBadge variant */
		statusMap?: Record<string, string>;
		/** Custom format function for cell value */
		format?: (_value: any, _row: any) => string;
	}

	interface Props {
		columns: ColumnDef[];
		rows: any[];
		totalCount: number;
		currentPage: number;
		pageSize: number;
		sortBy: string;
		sortDirection: 'asc' | 'desc';
		onPageChange: (_page: number) => void;
		onSort: (_field: string) => void;
		/** Key function to uniquely identify rows */
		rowKey: (_row: any) => string | number;
		/** Function to get error message for a row (if any — renders expandable error row) */
		getRowError?: (_row: any) => string | null;
		/** Label for items in pagination text (e.g., "files", "batches") */
		itemLabel?: string;
		emptyMessage?: string;
		/** Additional actions column content via snippet */
		renderActions?: import('svelte').Snippet<[any]>;
	}

	let {
		columns,
		rows,
		totalCount,
		currentPage,
		pageSize,
		sortBy,
		sortDirection,
		onPageChange,
		onSort,
		rowKey,
		getRowError,
		itemLabel = 'items',
		emptyMessage = 'No items found.',
		renderActions,
	}: Props = $props();

	let totalPages = $derived(totalCount > 0 && pageSize > 0 ? Math.ceil(totalCount / pageSize) : 1);

	function getCellValue(row: any, col: ColumnDef): string {
		const value = row[col.key];

		if (col.format) {
			return col.format(value, row);
		}

		switch (col.renderer) {
			case 'date':
				return formatDate(value);
			case 'duration':
				return value ? `${value}ms` : '-';
			case 'number':
				return value?.toLocaleString() ?? '0';
			default:
				return value ?? '-';
		}
	}

	function getStatusVariant(
		value: string,
		statusMap?: Record<string, string>
	):
		| 'success'
		| 'error'
		| 'warning'
		| 'info'
		| 'pending'
		| 'processing'
		| 'completed'
		| 'completed_with_errors'
		| 'idle'
		| 'failed'
		| 'enabled'
		| 'disabled' {
		if (statusMap && statusMap[value]) {
			return statusMap[value] as any;
		}
		// Default mapping
		const defaultMap: Record<string, string> = {
			success: 'success',
			completed: 'completed',
			failed: 'failed',
			error: 'error',
			processing: 'processing',
			pending: 'pending',
			idle: 'idle',
		};
		return (defaultMap[value] as any) || 'info';
	}

	const colSpan = $derived(columns.length + (renderActions ? 1 : 0));
</script>

{#if rows.length === 0}
	<p class="text-center text-gray-500 dark:text-gray-400 py-8">
		{emptyMessage}
	</p>
{:else}
	<div class="overflow-x-auto">
		<table class="w-full text-sm text-left text-gray-600 dark:text-gray-400">
			<thead class="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
				<tr>
					{#each columns as col (col.key)}
						<th class="px-4 py-3">
							{#if col.sortable}
								<button
									type="button"
									onclick={() => onSort(col.key)}
									class="flex items-center gap-1 font-semibold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
								>
									{col.label}
									{#if sortBy === col.key}
										{sortDirection === 'asc' ? '▲' : '▼'}
									{/if}
								</button>
							{:else}
								<span class="font-semibold text-gray-900 dark:text-white">{col.label}</span>
							{/if}
						</th>
					{/each}
					{#if renderActions}
						<th class="px-4 py-3">
							<span class="font-semibold text-gray-900 dark:text-white">Actions</span>
						</th>
					{/if}
				</tr>
			</thead>
			<tbody>
				{#each rows as row (rowKey(row))}
					<tr
						class="border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
					>
						{#each columns as col (col.key)}
							<td class="px-4 py-3">
								{#if col.renderer === 'status'}
									<StatusBadge
										status={getStatusVariant(row[col.key], col.statusMap)}
										label={row[col.key]}
									/>
								{:else if col.renderer === 'link'}
									{#if col.onLinkClick}
										<button
											onclick={() => col.onLinkClick?.(row)}
											class="font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
										>
											{col.format ? col.format(row[col.key], row) : row[col.key]}
										</button>
									{:else if col.getHref}
										<a
											href={col.getHref(row)}
											class="font-medium text-blue-600 dark:text-blue-400 hover:underline"
										>
											{col.format ? col.format(row[col.key], row) : row[col.key]}
										</a>
									{:else}
										<span class="font-medium text-gray-900 dark:text-white">
											{getCellValue(row, col)}
										</span>
									{/if}
								{:else}
									<span
										class={col.key === columns[0]?.key
											? 'font-medium text-gray-900 dark:text-white'
											: ''}
									>
										{getCellValue(row, col)}
									</span>
								{/if}
							</td>
						{/each}
						{#if renderActions}
							<td class="px-4 py-3">
								{@render renderActions(row)}
							</td>
						{/if}
					</tr>
					{#if getRowError?.(row)}
						<tr class="bg-red-50 dark:bg-red-900/10 border-b border-gray-200 dark:border-gray-700">
							<td colspan={colSpan} class="px-4 py-2 text-xs text-red-600 dark:text-red-400">
								Error: {getRowError(row)}
							</td>
						</tr>
					{/if}
				{/each}
			</tbody>
		</table>
	</div>

	<!-- Pagination -->
	{#if totalPages > 1}
		<div class="mt-4 flex items-center justify-between">
			<div class="text-sm text-gray-600 dark:text-gray-400">
				Showing {(currentPage - 1) * pageSize + 1} to {Math.min(currentPage * pageSize, totalCount)} of
				{totalCount}
				{itemLabel}
			</div>
			<div class="flex gap-2">
				<button
					onclick={() => onPageChange(currentPage - 1)}
					disabled={currentPage === 1}
					class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					Previous
				</button>
				<div class="flex items-center gap-1">
					{#each Array.from({ length: totalPages }, (_, i) => i + 1) as page (page)}
						{#if page === 1 || page === totalPages || (page >= currentPage - 1 && page <= currentPage + 1)}
							<button
								onclick={() => onPageChange(page)}
								class={`px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
									currentPage === page
										? 'bg-blue-600 text-white'
										: 'border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
								}`}
							>
								{page}
							</button>
						{:else if page === currentPage - 2 || page === currentPage + 2}
							<span class="px-2 py-2 text-gray-500">...</span>
						{/if}
					{/each}
				</div>
				<button
					onclick={() => onPageChange(currentPage + 1)}
					disabled={currentPage === totalPages}
					class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					Next
				</button>
			</div>
		</div>
	{/if}
{/if}
