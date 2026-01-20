/**
 * Common UI helper functions used across components
 */

/**
 * Format a date string to a human-readable format
 * @param dateString ISO 8601 date string or null
 * @param includeTime Whether to include time in the output
 * @returns Formatted date string or fallback
 */
export function formatDate(
	dateString: string | null | undefined,
	includeTime: boolean = true
): string {
	if (!dateString) return 'N/A';

	try {
		const date = new Date(dateString);
		if (isNaN(date.getTime())) return 'Invalid date';

		if (includeTime) {
			return date.toLocaleString(undefined, {
				year: 'numeric',
				month: 'short',
				day: 'numeric',
				hour: '2-digit',
				minute: '2-digit',
			});
		} else {
			return date.toLocaleDateString(undefined, {
				year: 'numeric',
				month: 'short',
				day: 'numeric',
			});
		}
	} catch {
		return 'Invalid date';
	}
}

/**
 * Format a relative time (e.g., "2 hours ago")
 * @param dateString ISO 8601 date string
 * @returns Relative time string
 */
export function formatRelativeTime(dateString: string | null | undefined): string {
	if (!dateString) return 'N/A';

	try {
		const date = new Date(dateString);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffSecs = Math.floor(diffMs / 1000);
		const diffMins = Math.floor(diffSecs / 60);
		const diffHours = Math.floor(diffMins / 60);
		const diffDays = Math.floor(diffHours / 24);

		if (diffSecs < 60) return 'just now';
		if (diffMins < 60) return `${diffMins} minute${diffMins !== 1 ? 's' : ''} ago`;
		if (diffHours < 24) return `${diffHours} hour${diffHours !== 1 ? 's' : ''} ago`;
		if (diffDays < 30) return `${diffDays} day${diffDays !== 1 ? 's' : ''} ago`;

		return formatDate(dateString, false);
	} catch {
		return 'N/A';
	}
}

/**
 * Format bytes to human-readable file size
 * @param bytes Number of bytes
 * @param decimals Number of decimal places
 * @returns Formatted string (e.g., "1.5 MB")
 */
export function formatFileSize(bytes: number | null | undefined, decimals: number = 2): string {
	if (bytes === null || bytes === undefined || bytes === 0) return '0 Bytes';

	const k = 1024;
	const dm = decimals < 0 ? 0 : decimals;
	const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];

	const i = Math.floor(Math.log(bytes) / Math.log(k));

	return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

/**
 * Format a number with thousand separators
 * @param num Number to format
 * @returns Formatted string (e.g., "1,234,567")
 */
export function formatNumber(num: number | null | undefined): string {
	if (num === null || num === undefined) return 'N/A';
	return num.toLocaleString();
}

/**
 * Truncate text with ellipsis
 * @param text Text to truncate
 * @param maxLength Maximum length before truncation
 * @returns Truncated text with ellipsis if needed
 */
export function truncateText(text: string, maxLength: number = 50): string {
	if (!text || text.length <= maxLength) return text;
	return text.substring(0, maxLength) + '...';
}

/**
 * Show a tooltip on hover
 * @param event Mouse event from hover
 * @param text Tooltip text
 */
export function showTooltip(event: MouseEvent, text: string): void {
	const button = event.target as HTMLElement;
	const tooltip = document.createElement('div');
	tooltip.className =
		'fixed bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 px-3 py-2 rounded text-sm z-50 max-w-md pointer-events-auto';
	tooltip.textContent = text;
	document.body.appendChild(tooltip);

	const updatePosition = () => {
		const rect = button.getBoundingClientRect();
		tooltip.style.left = rect.left + rect.width / 2 - tooltip.offsetWidth / 2 + 'px';
		tooltip.style.top = rect.top - tooltip.offsetHeight - 5 + 'px';
	};

	updatePosition();

	const hideTooltip = () => {
		tooltip.remove();
		button.removeEventListener('mouseleave', hideTooltip);
		tooltip.removeEventListener('mouseleave', hideTooltip);
	};

	button.addEventListener('mouseleave', hideTooltip);
	tooltip.addEventListener('mouseleave', hideTooltip);
}

/**
 * Debounce a function call
 * @param func Function to debounce
 * @param wait Wait time in milliseconds
 * @returns Debounced function
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function debounce<T extends (...args: any[]) => any>(
	func: T,
	wait: number
): (...args: Parameters<T>) => void {
	let timeout: ReturnType<typeof setTimeout> | null = null;

	return (...args: Parameters<T>) => {
		if (timeout) clearTimeout(timeout);
		timeout = setTimeout(() => func(...args), wait);
	};
}

/**
 * Copy text to clipboard
 * @param text Text to copy
 * @returns Promise that resolves when copied
 */
export async function copyToClipboard(text: string): Promise<void> {
	if (navigator.clipboard && window.isSecureContext) {
		await navigator.clipboard.writeText(text);
	} else {
		// Fallback for older browsers
		const textArea = document.createElement('textarea');
		textArea.value = text;
		textArea.style.position = 'fixed';
		textArea.style.left = '-999999px';
		document.body.appendChild(textArea);
		textArea.select();
		try {
			document.execCommand('copy');
		} finally {
			document.body.removeChild(textArea);
		}
	}
}

/**
 * Get status badge color class
 * @param status Status string
 * @returns Tailwind CSS color classes
 */
export function getStatusColor(status: string): string {
	const normalized = status.toLowerCase();

	if (normalized === 'completed' || normalized === 'success' || normalized === 'active') {
		return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300';
	}

	if (normalized === 'processing' || normalized === 'pending' || normalized === 'in_progress') {
		return 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300';
	}

	if (normalized === 'failed' || normalized === 'error') {
		return 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300';
	}

	if (normalized === 'cancelled' || normalized === 'disabled') {
		return 'bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-300';
	}

	return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300';
}
