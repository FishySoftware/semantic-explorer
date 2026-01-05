import { writable } from 'svelte/store';

export type ToastVariant = 'success' | 'error' | 'warning' | 'info';

export interface ToastMessage {
	id: number;
	title?: string;
	message: string;
	variant: ToastVariant;
	duration: number;
}

const DEFAULT_DURATION = 4000;

function createToastStore() {
	const { subscribe, update } = writable<ToastMessage[]>([]);
	let nextId = 1;

	function push(
		message: string,
		options: { title?: string; variant?: ToastVariant; duration?: number } = {}
	) {
		const id = nextId++;
		const toast: ToastMessage = {
			id,
			message,
			title: options.title,
			variant: options.variant ?? 'info',
			duration: Math.max(options.duration ?? DEFAULT_DURATION, 1500),
		};

		update((current) => [...current, toast]);

		const timeout = setTimeout(() => dismiss(id), toast.duration);

		return () => {
			clearTimeout(timeout);
			dismiss(id);
		};
	}

	function dismiss(id: number) {
		update((current) => current.filter((toast) => toast.id !== id));
	}

	return {
		subscribe,
		push,
		dismiss,
		success(message: string, title?: string, duration?: number) {
			return push(message, { title, variant: 'success', duration });
		},
		error(message: string, title?: string, duration?: number) {
			return push(message, { title, variant: 'error', duration });
		},
		info(message: string, title?: string, duration?: number) {
			return push(message, { title, variant: 'info', duration });
		},
		warning(message: string, title?: string, duration?: number) {
			return push(message, { title, variant: 'warning', duration });
		},
	};
}

export const toastStore = createToastStore();

export function formatError(error: unknown, fallback = 'Something went wrong'): string {
	if (error instanceof Error) {
		return error.message || fallback;
	}
	if (typeof error === 'string' && error.trim().length > 0) {
		return error;
	}
	return fallback;
}
