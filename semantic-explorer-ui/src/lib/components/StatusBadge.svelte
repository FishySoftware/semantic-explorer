<script lang="ts">
	type StatusVariant =
		| 'enabled'
		| 'disabled'
		| 'success'
		| 'error'
		| 'warning'
		| 'info'
		| 'pending'
		| 'processing'
		| 'completed'
		| 'completed_with_errors'
		| 'idle'
		| 'failed';

	interface Props {
		status: StatusVariant;
		label?: string;
	}

	let { status, label }: Props = $props();

	const variantClasses: Record<StatusVariant, string> = {
		enabled: 'bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400',
		disabled: 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400',
		success: 'bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400',
		error: 'bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400',
		warning: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/20 dark:text-yellow-400',
		info: 'bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400',
		pending: 'bg-purple-100 text-purple-700 dark:bg-purple-900/20 dark:text-purple-400',
		processing: 'bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400',
		completed: 'bg-green-100 text-green-700 dark:bg-green-900/20 dark:text-green-400',
		completed_with_errors:
			'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/20 dark:text-yellow-400',
		idle: 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400',
		failed: 'bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-400',
	};

	const defaultLabels: Record<StatusVariant, string> = {
		enabled: 'Enabled',
		disabled: 'Disabled',
		success: 'Success',
		error: 'Error',
		warning: 'Warning',
		info: 'Info',
		pending: 'Pending',
		processing: 'Processing',
		completed: 'Completed',
		completed_with_errors: 'Completed with Errors',
		idle: 'Idle',
		failed: 'Failed',
	};
</script>

<span
	class="px-3 py-1 text-sm rounded-lg font-medium {variantClasses[status]}"
	role="status"
	aria-live="polite"
	aria-label={`Status: ${label || defaultLabels[status]}`}
>
	{label || defaultLabels[status]}
</span>
