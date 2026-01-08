<script lang="ts">
	import { onMount } from 'svelte';
	import { Modal, Button } from 'flowbite-svelte';
	import { ChevronDownOutline } from 'flowbite-svelte-icons';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		open?: boolean;
		collectionId?: number | null;
		onSuccess?: () => void;
	}

	interface Collection {
		collection_id: number;
		title: string;
		file_count?: number;
	}

	let { open = false, collectionId = null, onSuccess }: Props = $props();

	let collections = $state<Collection[]>([]);

	let selectedCollectionId = $state<number | null>(null);
	let transformTitle = $state('');

	$effect(() => {
		if (open && collectionId !== null) {
			selectedCollectionId = collectionId;
		}
	});

	// Extraction strategy
	let extractionStrategy = $state('plain_text');
	let preserveFormatting = $state(false);
	let extractTables = $state(true);
	let preserveHeadings = $state(false);
	let preserveLists = $state(false);
	let preserveCodeBlocks = $state(false);

	// Chunking strategy
	let chunkingStrategy = $state('sentence');
	let chunkSize = $state(200);
	let chunkOverlap = $state(0);
	let minChunkSize = $state(50);
	let preserveSentenceBoundaries = $state(true);

	let loadingCollections = $state(true);
	let isCreating = $state(false);
	let error = $state<string | null>(null);
	let showAdvancedExtraction = $state(false);
	let showAdvancedChunking = $state(false);

	onMount(() => {
		fetchCollections();
	});

	async function fetchCollections() {
		try {
			loadingCollections = true;
			const response = await fetch('/api/collections');
			if (!response.ok) throw new Error('Failed to fetch collections');
			const allCollections: Collection[] = await response.json();
			// Filter to only collections with files
			collections = allCollections.filter((c) => (c.file_count ?? 0) > 0);
		} catch (e) {
			console.error('Failed to fetch collections:', e);
		} finally {
			loadingCollections = false;
		}
	}

	async function createTransform() {
		error = null;

		if (!transformTitle.trim()) {
			error = 'Title is required';
			return;
		}

		if (!selectedCollectionId) {
			error = 'Collection is required';
			return;
		}

		try {
			isCreating = true;

			const jobConfig: any = {
				extraction_options: {
					preserve_formatting: preserveFormatting,
					extract_tables: extractTables,
					preserve_headings: preserveHeadings,
					preserve_lists: preserveLists,
					preserve_code_blocks: preserveCodeBlocks,
				},
				chunking_options: {
					chunk_size: chunkSize,
					chunk_overlap: chunkOverlap,
					min_chunk_size: minChunkSize,
					preserve_sentence_boundaries: preserveSentenceBoundaries,
				},
			};

			const response = await fetch('/api/collection-transforms', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: transformTitle.trim(),
					collection_id: selectedCollectionId,
					dataset_id: 0, // Created by transform
					extraction_strategy: extractionStrategy,
					chunking_strategy: chunkingStrategy,
					job_config: jobConfig,
				}),
			});

			if (!response.ok) {
				throw new Error(`Failed to create transform: ${response.statusText}`);
			}

			toastStore.success('Transform created successfully');
			resetForm();
			onSuccess?.();
		} catch (e) {
			const message = formatError(e, 'Failed to create transform');
			error = message;
			toastStore.error(message);
		} finally {
			isCreating = false;
		}
	}

	function resetForm() {
		transformTitle = '';
		selectedCollectionId = collectionId ?? null;
		extractionStrategy = 'plain_text';
		preserveFormatting = false;
		extractTables = true;
		chunkingStrategy = 'sentence';
		chunkSize = 200;
		chunkOverlap = 0;
		minChunkSize = 50;
		preserveSentenceBoundaries = true;
		error = null;
		open = false;
	}

	function handleClose() {
		open = false;
		error = null;
	}

	$effect(() => {
		if (collectionId && !selectedCollectionId) {
			selectedCollectionId = collectionId;
		}
	});
</script>

<Modal bind:open onclose={handleClose}>
	<div class="p-4">
		<h2 class="text-xl font-bold text-gray-900 dark:text-white mb-4">
			Create Collection Transform
		</h2>

		{#if error}
			<div
				class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded text-red-700 dark:text-red-400 text-sm"
			>
				{error}
			</div>
		{/if}

		<div class="space-y-4 max-h-[70vh] overflow-y-auto">
			<!-- Title -->
			<div>
				<label
					for="transform-title"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Title <span class="text-red-500">*</span>
				</label>
				<input
					id="transform-title"
					type="text"
					bind:value={transformTitle}
					placeholder="e.g., Extract Documentation"
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				/>
			</div>

			<!-- Collection Selection -->
			<div>
				<label
					for="collection-select"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Source Collection <span class="text-red-500">*</span>
				</label>
				{#if loadingCollections}
					<div class="text-sm text-gray-500">Loading collections...</div>
				{:else if collections.length === 0}
					<div class="text-sm text-gray-500">No collections with files available</div>
				{:else}
					<select
						id="collection-select"
						bind:value={selectedCollectionId}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					>
						<option value={null}>Select a collection</option>
						{#each collections as collection (collection.collection_id)}
							<option value={collection.collection_id}>
								{collection.title} ({collection.file_count ?? 0} files)
							</option>
						{/each}
					</select>
				{/if}
			</div>

			<!-- Extraction Strategy -->
			<div>
				<label
					for="extraction-strategy"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Extraction Strategy
				</label>
				<select
					id="extraction-strategy"
					bind:value={extractionStrategy}
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				>
					<option value="plain_text">Plain Text</option>
					<option value="structure_preserving">Structure Preserving</option>
					<option value="markdown">Markdown</option>
				</select>
			</div>

			<!-- Advanced Extraction (Collapsible) -->
			<button
				onclick={() => (showAdvancedExtraction = !showAdvancedExtraction)}
				class="w-full flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white"
			>
				<ChevronDownOutline
					class={`w-4 h-4 transition-transform ${showAdvancedExtraction ? 'rotate-180' : ''}`}
				/>
				Advanced Extraction Options
			</button>

			{#if showAdvancedExtraction}
				<div
					class="bg-gray-50 dark:bg-gray-700/30 rounded-lg p-3 space-y-3 border border-gray-200 dark:border-gray-600"
				>
					<label class="flex items-center gap-2 cursor-pointer">
						<input
							type="checkbox"
							bind:checked={preserveFormatting}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Formatting</span>
					</label>

					<label class="flex items-center gap-2 cursor-pointer">
						<input
							type="checkbox"
							bind:checked={extractTables}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300">Extract Tables</span>
					</label>

					<label class="flex items-center gap-2 cursor-pointer">
						<input
							type="checkbox"
							bind:checked={preserveHeadings}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Headings</span>
					</label>

					<label class="flex items-center gap-2 cursor-pointer">
						<input
							type="checkbox"
							bind:checked={preserveLists}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Lists</span>
					</label>

					<label class="flex items-center gap-2 cursor-pointer">
						<input
							type="checkbox"
							bind:checked={preserveCodeBlocks}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Code Blocks</span>
					</label>
				</div>
			{/if}

			<!-- Chunking Strategy -->
			<div>
				<label
					for="chunking-strategy"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Chunking Strategy
				</label>
				<select
					id="chunking-strategy"
					bind:value={chunkingStrategy}
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				>
					<option value="sentence">Sentence</option>
					<option value="fixed_size">Fixed Size</option>
					<option value="recursive_character">Recursive Character</option>
					<option value="semantic">Semantic</option>
					<option value="markdown_aware">Markdown Aware</option>
				</select>
			</div>

			<!-- Basic Chunking Options -->
			<div class="grid grid-cols-2 gap-3">
				<div>
					<label
						for="chunk-size"
						class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Chunk Size
					</label>
					<input
						id="chunk-size"
						type="number"
						min="50"
						max="5000"
						bind:value={chunkSize}
						class="w-full px-2 py-1.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					/>
				</div>

				<div>
					<label
						for="chunk-overlap"
						class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Overlap
					</label>
					<input
						id="chunk-overlap"
						type="number"
						min="0"
						max="500"
						bind:value={chunkOverlap}
						class="w-full px-2 py-1.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					/>
				</div>
			</div>

			<!-- Advanced Chunking (Collapsible) -->
			<button
				onclick={() => (showAdvancedChunking = !showAdvancedChunking)}
				class="w-full flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white"
			>
				<ChevronDownOutline
					class={`w-4 h-4 transition-transform ${showAdvancedChunking ? 'rotate-180' : ''}`}
				/>
				Advanced Chunking Options
			</button>

			{#if showAdvancedChunking}
				<div
					class="bg-gray-50 dark:bg-gray-700/30 rounded-lg p-3 space-y-3 border border-gray-200 dark:border-gray-600"
				>
					<div>
						<label
							for="min-chunk-size"
							class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Min Chunk Size
						</label>
						<input
							id="min-chunk-size"
							type="number"
							min="1"
							max="1000"
							bind:value={minChunkSize}
							class="w-full px-2 py-1.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						/>
					</div>

					<label class="flex items-center gap-2 cursor-pointer">
						<input
							type="checkbox"
							bind:checked={preserveSentenceBoundaries}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300"
							>Preserve Sentence Boundaries</span
						>
					</label>
				</div>
			{/if}
		</div>

		<!-- Actions -->
		<div class="flex gap-3 mt-6">
			<Button onclick={createTransform} disabled={isCreating} color="blue" class="flex-1">
				{isCreating ? 'Creating...' : 'Create Transform'}
			</Button>
			<Button onclick={handleClose} color="alternative" class="flex-1">Cancel</Button>
		</div>
	</div>
</Modal>
