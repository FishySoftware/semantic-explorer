<script lang="ts">
	import { Heading } from 'flowbite-svelte';
	import { formatError, toastStore } from '../utils/notifications';
	import { formatDate } from '../utils/ui-helpers';
	import ConfirmDialog from './ConfirmDialog.svelte';
	import StatusBadge from './StatusBadge.svelte';

	type TransformType = 'collection' | 'dataset' | 'visualization';

	interface TransformInfo {
		id: number;
		title: string;
		is_enabled: boolean;
		created_at: string;
		updated_at: string;
	}

	interface ResourceLink {
		label: string;
		title: string;
		navigatePage: string;
		navigateParams: Record<string, unknown>;
	}

	interface Props {
		transform: TransformInfo;
		transformType: TransformType;
		apiBasePath: string;
		resourceLinks?: ResourceLink[];
		extraFields?: { label: string; value: string | number }[];
		onNavigate?: (_page: string, _params?: Record<string, unknown>) => void;
		onTransformUpdated: (_transform: any) => void;
		onDeleted: () => void;
		triggering?: boolean;
		onTrigger?: () => void;
	}

	let {
		transform,
		transformType,
		apiBasePath,
		resourceLinks = [],
		extraFields = [],
		onNavigate,
		onTransformUpdated,
		onDeleted,
		triggering = false,
		onTrigger,
	}: Props = $props();

	const typeLabels: Record<TransformType, string> = {
		collection: 'Collection transform',
		dataset: 'Dataset transform',
		visualization: 'Visualization transform',
	};

	// Edit mode state
	let editMode = $state(false);
	let editTitle = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete state
	let showDeleteConfirm = $state(false);
	let deleting = $state(false);

	function startEdit() {
		editMode = true;
		editTitle = transform.title;
		editError = null;
	}

	function cancelEdit() {
		editMode = false;
		editTitle = '';
		editError = null;
	}

	async function saveEdit() {
		if (!editTitle.trim()) {
			editError = 'Title is required';
			return;
		}

		try {
			saving = true;
			editError = null;
			const response = await fetch(`${apiBasePath}/${transform.id}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ title: editTitle.trim() }),
			});

			if (!response.ok) {
				throw new Error(`Failed to update transform: ${response.statusText}`);
			}

			const responseData = await response.json();
			const updated = responseData.transform || responseData;
			onTransformUpdated(updated);
			editMode = false;
			toastStore.success(`${typeLabels[transformType]} updated successfully`);
		} catch (e) {
			const message = formatError(e, `Failed to update ${typeLabels[transformType]}`);
			editError = message;
			toastStore.error(message);
		} finally {
			saving = false;
		}
	}

	async function toggleEnabled() {
		try {
			const response = await fetch(`${apiBasePath}/${transform.id}`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ is_enabled: !transform.is_enabled }),
			});

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const responseData = await response.json();
			const updated = responseData.transform || responseData;
			onTransformUpdated(updated);
			toastStore.success(
				`${typeLabels[transformType]} ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, `Failed to toggle ${typeLabels[transformType]}`));
		}
	}

	async function confirmDelete() {
		showDeleteConfirm = false;

		try {
			deleting = true;
			const response = await fetch(`${apiBasePath}/${transform.id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete transform: ${response.statusText}`);
			}

			toastStore.success(`${typeLabels[transformType]} deleted`);
			onDeleted();
		} catch (e) {
			toastStore.error(formatError(e, `Failed to delete ${typeLabels[transformType]}`));
		} finally {
			deleting = false;
		}
	}
</script>

<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
	<div class="flex justify-between items-start mb-4">
		<div class="flex-1">
			{#if editMode}
				<form
					onsubmit={(e) => {
						e.preventDefault();
						saveEdit();
					}}
					class="flex items-center gap-2 mb-2"
				>
					<input
						type="text"
						bind:value={editTitle}
						placeholder="Enter transform title"
						class="text-2xl font-bold px-3 py-1 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white flex-1"
						required
					/>
					<button
						type="submit"
						disabled={saving}
						class="px-3 py-1.5 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{saving ? 'Saving...' : 'Save'}
					</button>
					<button
						type="button"
						onclick={cancelEdit}
						class="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
					>
						Cancel
					</button>
				</form>
				{#if editError}
					<p class="text-sm text-red-600 dark:text-red-400 mt-1">{editError}</p>
				{/if}
			{:else}
				<div class="flex items-baseline gap-3 mb-2">
					<Heading tag="h2" class="text-2xl font-bold">{transform.title}</Heading>
					<span class="text-sm text-gray-500 dark:text-gray-400">#{transform.id}</span>
					<StatusBadge status={transform.is_enabled ? 'enabled' : 'disabled'} />
				</div>
			{/if}
			<p class="text-sm text-gray-500 dark:text-gray-400">
				Created {formatDate(transform.created_at)}
				{#if transform.updated_at && transform.updated_at !== transform.created_at}
					&middot; Updated {formatDate(transform.updated_at)}
				{/if}
			</p>
		</div>
		<div class="flex items-center gap-2 ml-4">
			{#if !editMode}
				<button
					onclick={startEdit}
					title="Edit title"
					class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300 transition-colors"
				>
					Edit
				</button>
			{/if}
			{#if onTrigger}
				<button
					onclick={onTrigger}
					disabled={triggering}
					title="Trigger transform processing"
					class="px-3 py-1 text-sm rounded-lg bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/20 dark:text-blue-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{#if triggering}
						<span class="inline-flex items-center gap-1">
							<svg class="animate-spin h-3.5 w-3.5" viewBox="0 0 24 24" fill="none">
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
									d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
								></path>
							</svg>
							Triggering…
						</span>
					{:else}
						▶ Trigger
					{/if}
				</button>
			{/if}
			<button
				onclick={toggleEnabled}
				title={transform.is_enabled ? 'Disable transform' : 'Enable transform'}
				class={transform.is_enabled
					? 'px-3 py-1 text-sm rounded-lg bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400 transition-colors'
					: 'px-3 py-1 text-sm rounded-lg bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400 transition-colors'}
			>
				{transform.is_enabled ? 'Disable' : 'Enable'}
			</button>
			<button
				onclick={() => (showDeleteConfirm = true)}
				disabled={deleting}
				title="Delete transform"
				class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
			>
				{deleting ? 'Deleting...' : 'Delete'}
			</button>
		</div>
	</div>

	<!-- Resource links and extra fields -->
	<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
		{#each resourceLinks as link (link.label)}
			<div>
				<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">{link.label}</p>
				<button
					onclick={() => onNavigate?.(link.navigatePage, link.navigateParams)}
					class="text-lg font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
				>
					{link.title}
				</button>
			</div>
		{/each}
		{#each extraFields as field (field.label)}
			<div>
				<p class="text-sm text-gray-500 dark:text-gray-400 mb-1">{field.label}</p>
				<p class="text-lg font-medium text-gray-900 dark:text-white">{field.value}</p>
			</div>
		{/each}
	</div>
</div>

<ConfirmDialog
	open={showDeleteConfirm}
	title="Delete {typeLabels[transformType]}"
	message={`Are you sure you want to delete "${transform.title}"? This action cannot be undone.`}
	confirmLabel="Delete"
	variant="danger"
	onConfirm={confirmDelete}
	onCancel={() => (showDeleteConfirm = false)}
/>
