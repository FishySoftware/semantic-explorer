<!-- eslint-disable svelte/no-at-html-tags -->
<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	// Helper function for tooltip display with hover persistence
	function showTooltip(event: MouseEvent, text: string) {
		const button = event.target as HTMLElement;
		const tooltip = document.createElement('div');
		tooltip.className =
			'fixed bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 px-3 py-2 rounded text-sm z-50 whitespace-nowrap';
		tooltip.textContent = text;
		tooltip.style.pointerEvents = 'auto';
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

	// Info icon SVG component
	function InfoIcon() {
		return `<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg"><path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"></path></svg>`;
	}

	interface CollectionTransform {
		collection_transform_id: number;
		title: string;
		collection_id: number;
		dataset_id: number;
		owner: string;
		is_enabled: boolean;
		chunk_size: number;
		job_config: any;
		created_at: string;
		updated_at: string;
	}

	interface Collection {
		collection_id: number;
		title: string;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
	}

	interface Stats {
		collection_transform_id: number;
		total_files_processed: number;
		successful_files: number;
		failed_files: number;
		total_items_created: number;
	}

	interface ProcessedFile {
		id: number;
		transform_type: string;
		transform_id: number;
		file_key: string;
		processed_at: string;
		item_count: number;
		process_status: string;
		process_error: string | null;
		processing_duration_ms: number | null;
	}

	let transforms = $state<CollectionTransform[]>([]);
	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);
	let statsMap = $state<Map<number, Stats>>(new Map());
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Failed files modal state
	let showFailedFilesModal = $state(false);
	let failedFilesTransformTitle = $state('');
	let failedFiles = $state<ProcessedFile[]>([]);
	let loadingFailedFiles = $state(false);

	let searchQuery = $state('');

	let showCreateForm = $state(false);
	let editingTransform = $state<CollectionTransform | null>(null);
	let newTitle = $state('');
	let newCollectionId = $state<number | null>(null);
	let newDatasetId = $state<number | null>(null);
	let newChunkSize = $state(200);
	let extractionStrategy = $state('plain_text');
	let extractPreserveFormatting = $state(false);
	let extractExtractTables = $state(true);
	let extractTableFormat = $state('plain_text');
	let extractPreserveHeadings = $state(false);
	let extractHeadingFormat = $state('plain_text');
	let extractPreserveLists = $state(false);
	let extractPreserveCodeBlocks = $state(false);
	let extractIncludeMetadata = $state(false);
	let chunkingStrategy = $state('sentence');
	let chunkOverlap = $state(0);
	let preserveSentenceBoundaries = $state(true);
	let trimWhitespace = $state(true);
	let minChunkSize = $state(50);
	let recursiveSeparators = $state('\n\n,\n, ,.');
	let recursiveKeepSeparator = $state(true);
	let markdownSplitOnHeaders = $state(true);
	let markdownPreserveCodeBlocks = $state(false);
	let semanticEmbedderId = $state<number | null>(null);
	let semanticSimilarityThreshold = $state(0.7);
	let semanticMinChunkSize = $state(50);
	let semanticMaxChunkSize = $state(500);
	let semanticBufferSize = $state(1);
	let creating = $state(false);
	let createError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<CollectionTransform | null>(null);

	$effect(() => {
		if (showCreateForm && !editingTransform && !newTitle) {
			const now = new Date();
			const date = now.toISOString().split('T')[0];
			const time = now.toTimeString().split(' ')[0].replace(/:/g, '').slice(0, 4);
			newTitle = `collection-transform-${date}-${time}`;
		}
	});

	async function fetchTransforms() {
		try {
			loading = true;
			error = null;
			const response = await fetch('/api/collection-transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch collection transforms: ${response.statusText}`);
			}
			transforms = await response.json();
			for (const transform of transforms) {
				fetchStatsForTransform(transform.collection_transform_id);
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch collection transforms');
			error = message;
			toastStore.error(message);
		} finally {
			loading = false;
		}
	}

	async function fetchStatsForTransform(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/stats`);
			if (response.ok) {
				const stats = await response.json();
				statsMap.set(transformId, stats);
				statsMap = statsMap; // Trigger reactivity
			}
		} catch (e) {
			console.error(`Failed to fetch stats for transform ${transformId}:`, e);
		}
	}

	async function openFailedFilesModal(transform: CollectionTransform) {
		failedFilesTransformTitle = transform.title;
		showFailedFilesModal = true;
		loadingFailedFiles = true;
		failedFiles = [];

		try {
			const response = await fetch(
				`/api/collection-transforms/${transform.collection_transform_id}/processed-files`
			);
			if (response.ok) {
				const allFiles: ProcessedFile[] = await response.json();
				// Filter to only failed files
				failedFiles = allFiles.filter((f) => f.process_status === 'failed');
			}
		} catch (e) {
			console.error(
				`Failed to fetch processed files for transform ${transform.collection_transform_id}:`,
				e
			);
			toastStore.error('Failed to fetch failed files');
		} finally {
			loadingFailedFiles = false;
		}
	}

	function closeFailedFilesModal() {
		showFailedFilesModal = false;
		failedFilesTransformTitle = '';
		failedFiles = [];
	}

	async function fetchCollections() {
		try {
			const response = await fetch('/api/collections');
			if (!response.ok) {
				throw new Error(`Failed to fetch collections: ${response.statusText}`);
			}
			collections = await response.json();
		} catch (e) {
			console.error('Failed to fetch collections:', e);
		}
	}

	async function fetchDatasets() {
		try {
			const response = await fetch('/api/datasets');
			if (!response.ok) {
				throw new Error(`Failed to fetch datasets: ${response.statusText}`);
			}
			datasets = await response.json();
		} catch (e) {
			console.error('Failed to fetch datasets:', e);
		}
	}

	async function fetchEmbedders() {
		try {
			const response = await fetch('/api/embedders');
			if (!response.ok) {
				throw new Error(`Failed to fetch embedders: ${response.statusText}`);
			}
			embedders = await response.json();
		} catch (e) {
			console.error('Failed to fetch embedders:', e);
		}
	}

	async function createTransform() {
		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		if (!newCollectionId) {
			createError = 'Collection is required';
			return;
		}

		if (!newDatasetId) {
			createError = 'Dataset is required';
			return;
		}

		try {
			creating = true;
			createError = null;

			const url = editingTransform
				? `/api/collection-transforms/${editingTransform.collection_transform_id}`
				: '/api/collection-transforms';
			const method = editingTransform ? 'PATCH' : 'POST';

			const jobConfig: any = {
				extraction: {
					strategy: extractionStrategy,
					options: {
						preserve_formatting: extractPreserveFormatting,
						extract_tables: extractExtractTables,
						table_format: extractTableFormat,
						preserve_headings: extractPreserveHeadings,
						heading_format: extractHeadingFormat,
						preserve_lists: extractPreserveLists,
						preserve_code_blocks: extractPreserveCodeBlocks,
						include_metadata: extractIncludeMetadata,
					},
				},
				chunking: {
					strategy: chunkingStrategy,
					chunk_size: newChunkSize,
					chunk_overlap: chunkOverlap,
					options: {
						preserve_sentence_boundaries: preserveSentenceBoundaries,
						trim_whitespace: trimWhitespace,
						min_chunk_size: minChunkSize,
					},
				},
			};

			// Add strategy-specific options
			if (chunkingStrategy === 'recursive_character') {
				const separators = recursiveSeparators
					.split(',')
					.map((s) => s.trim())
					.filter((s) => s);
				jobConfig.chunking.options.recursive_character = {
					separators,
					keep_separator: recursiveKeepSeparator,
				};
			} else if (chunkingStrategy === 'markdown_aware') {
				jobConfig.chunking.options.markdown_aware = {
					split_on_headers: markdownSplitOnHeaders,
					preserve_code_blocks: markdownPreserveCodeBlocks,
				};
			} else if (chunkingStrategy === 'semantic') {
				jobConfig.chunking.options.semantic = {
					embedder_id: semanticEmbedderId,
					similarity_threshold: semanticSimilarityThreshold,
					min_chunk_size: semanticMinChunkSize,
					max_chunk_size: semanticMaxChunkSize,
					buffer_size: semanticBufferSize,
				};
			}

			const body = editingTransform
				? {
						title: newTitle,
					}
				: {
						title: newTitle,
						collection_id: newCollectionId,
						dataset_id: newDatasetId,
						chunk_size: newChunkSize,
						job_config: jobConfig,
					};

			const response = await fetch(url, {
				method,
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				throw new Error(
					`Failed to ${editingTransform ? 'update' : 'create'} collection transform: ${response.statusText}`
				);
			}

			const savedTransform = await response.json();

			if (editingTransform) {
				transforms = transforms.map((t) =>
					t.collection_transform_id === savedTransform.collection_transform_id ? savedTransform : t
				);
				toastStore.success('Collection transform updated successfully');
			} else {
				transforms = [...transforms, savedTransform];
				toastStore.success('Collection transform created successfully');
			}

			resetForm();
		} catch (e) {
			const message = formatError(
				e,
				`Failed to ${editingTransform ? 'update' : 'create'} collection transform`
			);
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	async function toggleEnabled(transform: CollectionTransform) {
		try {
			const response = await fetch(
				`/api/collection-transforms/${transform.collection_transform_id}`,
				{
					method: 'PATCH',
					headers: {
						'Content-Type': 'application/json',
					},
					body: JSON.stringify({
						is_enabled: !transform.is_enabled,
					}),
				}
			);

			if (!response.ok) {
				throw new Error(`Failed to toggle transform: ${response.statusText}`);
			}

			const updated = await response.json();
			transforms = transforms.map((t) =>
				t.collection_transform_id === updated.collection_transform_id ? updated : t
			);

			toastStore.success(
				`Collection transform ${updated.is_enabled ? 'enabled' : 'disabled'} successfully`
			);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to toggle collection transform'));
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			const response = await fetch(`/api/collection-transforms/${transformId}/trigger`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({ collection_transform_id: transformId }),
			});

			if (!response.ok) {
				throw new Error(`Failed to trigger transform: ${response.statusText}`);
			}

			toastStore.success('Collection transform triggered successfully');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger collection transform'));
		}
	}

	function openEditForm(transform: CollectionTransform) {
		editingTransform = transform;
		newTitle = transform.title;
		newCollectionId = transform.collection_id;
		newDatasetId = transform.dataset_id;
		newChunkSize = transform.chunk_size;

		// Extraction config
		const extractConfig = transform.job_config?.extraction || {};
		extractionStrategy = extractConfig.strategy || 'plain_text';
		const extractOpts = extractConfig.options || {};
		extractPreserveFormatting = extractOpts.preserve_formatting ?? false;
		extractExtractTables = extractOpts.extract_tables ?? true;
		extractTableFormat = extractOpts.table_format ?? 'plain_text';
		extractPreserveHeadings = extractOpts.preserve_headings ?? false;
		extractHeadingFormat = extractOpts.heading_format ?? 'plain_text';
		extractPreserveLists = extractOpts.preserve_lists ?? false;
		extractPreserveCodeBlocks = extractOpts.preserve_code_blocks ?? false;
		extractIncludeMetadata = extractOpts.include_metadata ?? false;

		// Chunking config
		const chunkConfig = transform.job_config?.chunking || {};
		chunkingStrategy = chunkConfig.strategy || 'sentence';
		chunkOverlap = chunkConfig.chunk_overlap ?? 0;
		const chunkOpts = chunkConfig.options || {};
		preserveSentenceBoundaries = chunkOpts.preserve_sentence_boundaries ?? true;
		trimWhitespace = chunkOpts.trim_whitespace ?? true;
		minChunkSize = chunkOpts.min_chunk_size ?? 50;
		recursiveSeparators = chunkOpts.recursive_character?.separators?.join(',') || '\n\n,\n, ,.';
		recursiveKeepSeparator = chunkOpts.recursive_character?.keep_separator ?? true;
		markdownSplitOnHeaders = chunkOpts.markdown_aware?.split_on_headers ?? true;
		markdownPreserveCodeBlocks = chunkOpts.markdown_aware?.preserve_code_blocks ?? false;
		semanticEmbedderId = chunkOpts.semantic?.embedder_id ?? null;
		semanticSimilarityThreshold = chunkOpts.semantic?.similarity_threshold ?? 0.7;
		semanticMinChunkSize = chunkOpts.semantic?.min_chunk_size ?? 50;
		semanticMaxChunkSize = chunkOpts.semantic?.max_chunk_size ?? 500;
		semanticBufferSize = chunkOpts.semantic?.buffer_size ?? 1;

		showCreateForm = true;
	}

	function resetForm() {
		newTitle = '';
		newCollectionId = null;
		newDatasetId = null;
		newChunkSize = 200;
		extractionStrategy = 'plain_text';
		extractPreserveFormatting = false;
		extractExtractTables = true;
		extractTableFormat = 'plain_text';
		extractPreserveHeadings = false;
		extractHeadingFormat = 'plain_text';
		extractPreserveLists = false;
		extractPreserveCodeBlocks = false;
		extractIncludeMetadata = false;
		chunkingStrategy = 'sentence';
		chunkOverlap = 0;
		preserveSentenceBoundaries = true;
		trimWhitespace = true;
		minChunkSize = 50;
		recursiveSeparators = '\n\n,\n, ,.';
		recursiveKeepSeparator = true;
		markdownSplitOnHeaders = true;
		markdownPreserveCodeBlocks = false;
		semanticEmbedderId = null;
		semanticSimilarityThreshold = 0.7;
		semanticMinChunkSize = 50;
		semanticMaxChunkSize = 500;
		semanticBufferSize = 1;
		showCreateForm = false;
		editingTransform = null;
		createError = null;
	}

	function requestDeleteTransform(transform: CollectionTransform) {
		transformPendingDelete = transform;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.collection_transform_id;
			const response = await fetch(`/api/collection-transforms/${target.collection_transform_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				throw new Error(`Failed to delete collection transform: ${response.statusText}`);
			}

			transforms = transforms.filter(
				(t) => t.collection_transform_id !== target.collection_transform_id
			);
			toastStore.success('Collection transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete collection transform'));
		} finally {
			deleting = null;
		}
	}

	onMount(async () => {
		await Promise.all([fetchTransforms(), fetchCollections(), fetchDatasets(), fetchEmbedders()]);
		const hashParts = window.location.hash.split('?');
		if (hashParts.length > 1) {
			const urlParams = new URLSearchParams(hashParts[1]);

			const action = urlParams.get('action');
			const collectionIdParam = urlParams.get('collection_id');

			let shouldOpenForm = false;

			if (action === 'create' && collectionIdParam) {
				const collectionId = parseInt(collectionIdParam, 10);
				if (!isNaN(collectionId)) {
					newCollectionId = collectionId;
					shouldOpenForm = true;
				}
			}

			if (shouldOpenForm) {
				showCreateForm = true;
				const basePath = hashParts[0];
				window.history.replaceState(
					null,
					'',
					window.location.pathname + window.location.search + basePath
				);
			}
		}
	});

	let filteredTransforms = $derived(
		transforms.filter((t) => {
			if (!searchQuery.trim()) return true;
			const query = searchQuery.toLowerCase();
			return t.title.toLowerCase().includes(query) || t.owner.toLowerCase().includes(query);
		})
	);

	function getCollectionTitle(collectionId: number): string {
		const collection = collections.find((c) => c.collection_id === collectionId);
		return collection ? collection.title : `Collection ${collectionId}`;
	}

	function getDatasetTitle(datasetId: number): string {
		const dataset = datasets.find((d) => d.dataset_id === datasetId);
		return dataset ? dataset.title : `Dataset ${datasetId}`;
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Collection Transforms"
		description="Process files from Collections into Dataset items. Collection transforms extract text from files, chunk them into manageable pieces, and create Dataset items ready for embedding."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Collection Transforms</h1>
		<button
			onclick={() => {
				if (showCreateForm) {
					resetForm();
				} else {
					showCreateForm = true;
				}
			}}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Collection Transform'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
				{editingTransform ? 'Edit Collection Transform' : 'Create New Collection Transform'}
			</h2>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					createTransform();
				}}
			>
				<div class="mb-4">
					<label
						for="transform-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Title
					</label>
					<input
						id="transform-title"
						type="text"
						bind:value={newTitle}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						placeholder="Enter transform title..."
					/>
				</div>

				{#if !editingTransform}
					<div class="mb-4">
						<label
							for="collection-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Source Collection
						</label>
						<select
							id="collection-select"
							bind:value={newCollectionId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select a collection...</option>
							{#each collections as collection (collection.collection_id)}
								<option value={collection.collection_id}>{collection.title}</option>
							{/each}
						</select>
					</div>

					<div class="mb-4">
						<label
							for="dataset-select"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
						>
							Target Dataset
						</label>
						<select
							id="dataset-select"
							bind:value={newDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
						>
							<option value={null}>Select a dataset...</option>
							{#each datasets as dataset (dataset.dataset_id)}
								<option value={dataset.dataset_id}>{dataset.title}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="mb-4">
					<label
						for="chunk-size"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
					>
						Chunk Size
					</label>
					<input
						id="chunk-size"
						type="number"
						bind:value={newChunkSize}
						min="50"
						max="1000"
						disabled={editingTransform !== null}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
					/>
					<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
						{editingTransform
							? 'Chunk size cannot be changed after creation'
							: 'Number of characters per chunk (50-1000)'}
					</p>
				</div>

				<!-- Extraction Configuration -->
				<div class="border-t border-gray-200 dark:border-gray-700 pt-4 mt-4 mb-4">
					<h3 class="text-lg font-medium text-gray-900 dark:text-white mb-3">
						Extraction Configuration
					</h3>
					<div class="space-y-4">
						<div>
							<label
								for="extraction-strategy"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Extraction Strategy
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Method for extracting text: Plain Text is fastest, Structure Preserving keeps formatting, Markdown converts to markdown'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<select
								id="extraction-strategy"
								bind:value={extractionStrategy}
								disabled={editingTransform !== null}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
							>
								<option value="plain_text">Plain Text</option>
								<option value="structure_preserving">Structure Preserving</option>
								<option value="markdown">Markdown</option>
							</select>
							<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
								{editingTransform
									? 'Cannot be changed after creation'
									: 'How to extract text from files'}
							</p>
						</div>

						<div class="space-y-3 bg-gray-50 dark:bg-gray-900/20 p-3 rounded">
							<p class="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase">
								Extraction Options
							</p>

							<div class="flex items-center">
								<input
									id="extract-preserve-formatting"
									type="checkbox"
									bind:checked={extractPreserveFormatting}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="extract-preserve-formatting"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Preserve Formatting
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(
												e,
												'Keep original document formatting like bold, italics, colors'
											)}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>

							<div class="flex items-center">
								<input
									id="extract-tables"
									type="checkbox"
									bind:checked={extractExtractTables}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="extract-tables"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Extract Tables
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) => showTooltip(e, 'Extract data from tables in documents')}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>

							{#if extractExtractTables}
								<div class="ml-6">
									<label
										for="table-format"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Table Format
									</label>
									<select
										id="table-format"
										bind:value={extractTableFormat}
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed text-sm"
									>
										<option value="plain_text">Plain Text</option>
										<option value="markdown">Markdown</option>
										<option value="csv">CSV</option>
									</select>
								</div>
							{/if}

							<div class="flex items-center">
								<input
									id="extract-headings"
									type="checkbox"
									bind:checked={extractPreserveHeadings}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="extract-headings"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Preserve Headings
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(e, 'Keep document headings/titles in extraction')}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>

							{#if extractPreserveHeadings}
								<div class="ml-6">
									<label
										for="heading-format"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Heading Format
									</label>
									<select
										id="heading-format"
										bind:value={extractHeadingFormat}
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed text-sm"
									>
										<option value="plain_text">Plain Text</option>
										<option value="markdown">Markdown</option>
									</select>
								</div>
							{/if}

							<div class="flex items-center">
								<input
									id="extract-lists"
									type="checkbox"
									bind:checked={extractPreserveLists}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="extract-lists"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Preserve Lists
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) => showTooltip(e, 'Keep bullet/numbered lists in extraction')}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>

							<div class="flex items-center">
								<input
									id="extract-code"
									type="checkbox"
									bind:checked={extractPreserveCodeBlocks}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="extract-code"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Preserve Code Blocks
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(e, 'Keep code blocks exactly as-is during extraction')}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>

							<div class="flex items-center">
								<input
									id="extract-metadata"
									type="checkbox"
									bind:checked={extractIncludeMetadata}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="extract-metadata"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Include Metadata
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(e, 'Include document metadata like author, date, title')}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>
						</div>
					</div>
				</div>

				<!-- Chunking Configuration -->
				<div class="border-t border-gray-200 dark:border-gray-700 pt-4 mt-4">
					<h3 class="text-lg font-medium text-gray-900 dark:text-white mb-3">
						Chunking Configuration
					</h3>
					<div class="space-y-4">
						<div>
							<label
								for="chunking-strategy"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Chunking Strategy
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Sentence: by sentences, Fixed: fixed character count, Recursive: hierarchical splitting, Semantic: by meaning, Markdown: respects markdown structure'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<select
								id="chunking-strategy"
								bind:value={chunkingStrategy}
								disabled={editingTransform !== null}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
							>
								<option value="sentence">Sentence</option>
								<option value="fixed_size">Fixed Size</option>
								<option value="recursive_character">Recursive Character</option>
								<option value="semantic">Semantic</option>
								<option value="markdown_aware">Markdown Aware</option>
							</select>
							<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
								{editingTransform
									? 'Cannot be changed after creation'
									: 'How to split text into chunks'}
							</p>
						</div>

						<div>
							<label
								for="chunk-overlap"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Chunk Overlap
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Number of characters to overlap between consecutive chunks for context continuity'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="chunk-overlap"
								type="number"
								bind:value={chunkOverlap}
								min="0"
								max="100"
								disabled={editingTransform !== null}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
							/>
							<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
								Characters to overlap between chunks (0-100)
							</p>
						</div>

						<div>
							<label
								for="min-chunk-size"
								class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Minimum Chunk Size
								<button
									type="button"
									class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
									onmouseenter={(e) =>
										showTooltip(
											e,
											'Smallest acceptable chunk size in characters. Chunks smaller than this are merged with neighbors'
										)}
								>
									{@html InfoIcon()}
								</button>
							</label>
							<input
								id="min-chunk-size"
								type="number"
								bind:value={minChunkSize}
								min="10"
								max="500"
								disabled={editingTransform !== null}
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
							/>
							<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
								Minimum characters per chunk (10-500)
							</p>
						</div>

						<div class="space-y-3">
							<div class="flex items-center">
								<input
									id="preserve-sentence"
									type="checkbox"
									bind:checked={preserveSentenceBoundaries}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="preserve-sentence"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Preserve Sentence Boundaries
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(
												e,
												'Never split in the middle of a sentence. Maintains readability of extracted content'
											)}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>
							<div class="flex items-center">
								<input
									id="trim-whitespace"
									type="checkbox"
									bind:checked={trimWhitespace}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="trim-whitespace"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Trim Whitespace
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(
												e,
												'Remove leading and trailing whitespace from each chunk for cleaner output'
											)}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>
						</div>

						{#if chunkingStrategy === 'recursive_character'}
							<div>
								<label
									for="recursive-separators"
									class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Separators (comma-separated)
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(
												e,
												'Hierarchical separators for recursive splitting. Tries first separator, then next if chunks too big'
											)}
									>
										{@html InfoIcon()}
									</button>
								</label>
								<input
									id="recursive-separators"
									type="text"
									bind:value={recursiveSeparators}
									disabled={editingTransform !== null}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
									placeholder="\n\n,\n, ,."
								/>
								<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
									Separators to use for recursive splitting
								</p>
							</div>
							<div class="flex items-center">
								<input
									id="recursive-keep-sep"
									type="checkbox"
									bind:checked={recursiveKeepSeparator}
									disabled={editingTransform !== null}
									class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
								/>
								<label
									for="recursive-keep-sep"
									class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
								>
									Keep Separator
									<button
										type="button"
										class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
										onmouseenter={(e) =>
											showTooltip(
												e,
												'Include the separator character(s) at the end of chunks instead of discarding them'
											)}
									>
										{@html InfoIcon()}
									</button>
								</label>
							</div>
						{/if}

						{#if chunkingStrategy === 'markdown_aware'}
							<div class="space-y-3 bg-gray-50 dark:bg-gray-900/20 p-3 rounded">
								<p class="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase">
									Markdown Aware Options
								</p>

								<div class="flex items-center">
									<input
										id="markdown-split-headers"
										type="checkbox"
										bind:checked={markdownSplitOnHeaders}
										disabled={editingTransform !== null}
										class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
									/>
									<label
										for="markdown-split-headers"
										class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
									>
										Split on Headers
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Start new chunks at markdown headers (#, ##, etc.) to preserve document structure'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
								</div>
								<div class="flex items-center">
									<input
										id="markdown-code"
										type="checkbox"
										bind:checked={markdownPreserveCodeBlocks}
										disabled={editingTransform !== null}
										class="w-4 h-4 text-blue-600 rounded disabled:opacity-50"
									/>
									<label
										for="markdown-code"
										class="ml-2 flex items-center gap-1 text-sm font-medium text-gray-700 dark:text-gray-300"
									>
										Preserve Code Blocks
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Keep code blocks (```...```) intact and unsplit for proper syntax highlighting'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
								</div>
							</div>
						{/if}

						{#if chunkingStrategy === 'semantic'}
							<div class="space-y-3 bg-gray-50 dark:bg-gray-900/20 p-3 rounded">
								<p class="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase">
									Semantic Options
								</p>

								<div>
									<label
										for="semantic-embedder"
										class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Embedder
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Model used to generate embeddings for comparing chunk similarity and deciding where to split'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
									<select
										id="semantic-embedder"
										bind:value={semanticEmbedderId}
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
									>
										<option value={null}>Select an embedder...</option>
										{#each embedders as embedder (embedder.embedder_id)}
											<option value={embedder.embedder_id}>{embedder.name}</option>
										{/each}
									</select>
									<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
										Required for semantic chunking
									</p>
								</div>

								<div>
									<label
										for="semantic-threshold"
										class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Similarity Threshold
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Minimum similarity score (0-1) to keep chunks together. Higher = stricter splitting on meaning changes'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
									<input
										id="semantic-threshold"
										type="number"
										bind:value={semanticSimilarityThreshold}
										min="0"
										max="1"
										step="0.01"
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
									/>
									<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
										0.0 to 1.0 (default: 0.7)
									</p>
								</div>

								<div>
									<label
										for="semantic-min"
										class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Min Chunk Size
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Minimum characters for semantic chunks. Smaller chunks may be merged with neighbors'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
									<input
										id="semantic-min"
										type="number"
										bind:value={semanticMinChunkSize}
										min="10"
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
									/>
								</div>

								<div>
									<label
										for="semantic-max"
										class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Max Chunk Size
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Maximum characters for semantic chunks. Chunks exceeding this are split even if semantically similar'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
									<input
										id="semantic-max"
										type="number"
										bind:value={semanticMaxChunkSize}
										min="50"
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
									/>
								</div>

								<div>
									<label
										for="semantic-buffer"
										class="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Buffer Size
										<button
											type="button"
											class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-help"
											onmouseenter={(e) =>
												showTooltip(
													e,
													'Characters of context to look ahead/behind when deciding split points. Higher = considers more context'
												)}
										>
											{@html InfoIcon()}
										</button>
									</label>
									<input
										id="semantic-buffer"
										type="number"
										bind:value={semanticBufferSize}
										min="0"
										disabled={editingTransform !== null}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-50 disabled:cursor-not-allowed"
									/>
									<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
										Number of sentences to buffer (default: 1)
									</p>
								</div>
							</div>
						{/if}
					</div>
				</div>

				{#if createError}
					<div
						class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg"
					>
						<p class="text-sm text-red-600 dark:text-red-400">{createError}</p>
					</div>
				{/if}

				<div class="flex gap-3 pt-6">
					<button
						type="submit"
						disabled={creating}
						class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{creating
							? editingTransform
								? 'Updating...'
								: 'Creating...'
							: editingTransform
								? 'Update Transform'
								: 'Create Transform'}
					</button>
					<button
						type="button"
						onclick={resetForm}
						class="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	<div class="mb-4">
		<input
			type="text"
			bind:value={searchQuery}
			placeholder="Search collection transforms..."
			class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
		/>
	</div>

	{#if loading}
		<div class="text-center py-8">
			<p class="text-gray-600 dark:text-gray-400">Loading collection transforms...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
		>
			<p class="text-red-600 dark:text-red-400">{error}</p>
		</div>
	{:else if filteredTransforms.length === 0}
		<div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-8 text-center">
			<p class="text-gray-600 dark:text-gray-400">
				{searchQuery
					? 'No collection transforms found matching your search.'
					: 'No collection transforms yet. Create one to get started!'}
			</p>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each filteredTransforms as transform (transform.collection_transform_id)}
				{@const stats = statsMap.get(transform.collection_transform_id)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6">
					<div class="flex justify-between items-start mb-4">
						<div class="flex-1">
							<div class="flex items-baseline gap-3 mb-2">
								<h3 class="text-xl font-semibold text-gray-900 dark:text-white">
									{transform.title}
								</h3>
								<span class="text-sm text-gray-500 dark:text-gray-400">
									#{transform.collection_transform_id}
								</span>
							</div>
							<div class="text-sm text-gray-600 dark:text-gray-400 space-y-1">
								<p>
									<strong>Collection:</strong>
									<a
										href="#/collections/{transform.collection_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{getCollectionTitle(transform.collection_id)}
									</a>
								</p>
								<p>
									<strong>Dataset:</strong>
									<a
										href="#/datasets/{transform.dataset_id}/details"
										class="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 hover:underline"
									>
										{getDatasetTitle(transform.dataset_id)}
									</a>
								</p>
								<p><strong>Chunk Size:</strong> {transform.chunk_size}</p>
								<p><strong>Owner:</strong> {transform.owner}</p>
								<p>
									<strong>Status:</strong>
									<span
										class={transform.is_enabled
											? 'text-green-600 dark:text-green-400'
											: 'text-gray-500 dark:text-gray-400'}
									>
										{transform.is_enabled ? 'Enabled' : 'Disabled'}
									</span>
								</p>
								<p><strong>Created:</strong> {new Date(transform.created_at).toLocaleString()}</p>
								<p><strong>Updated:</strong> {new Date(transform.updated_at).toLocaleString()}</p>
							</div>
						</div>
						<div class="flex flex-col gap-2">
							<button
								onclick={() => toggleEnabled(transform)}
								class="px-3 py-1 text-sm rounded-lg {transform.is_enabled
									? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/20 dark:text-yellow-400'
									: 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/20 dark:text-green-400'}"
							>
								{transform.is_enabled ? 'Disable' : 'Enable'}
							</button>
							<button
								onclick={() => triggerTransform(transform.collection_transform_id)}
								class="px-3 py-1 text-sm bg-blue-100 text-blue-700 hover:bg-blue-200 rounded-lg dark:bg-blue-900/20 dark:text-blue-400"
							>
								Trigger
							</button>
							<button
								onclick={() => openEditForm(transform)}
								class="px-3 py-1 text-sm bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300"
							>
								Edit
							</button>
							<button
								onclick={() => requestDeleteTransform(transform)}
								disabled={deleting === transform.collection_transform_id}
								class="px-3 py-1 text-sm bg-red-100 text-red-700 hover:bg-red-200 rounded-lg dark:bg-red-900/20 dark:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{deleting === transform.collection_transform_id ? 'Deleting...' : 'Delete'}
							</button>
						</div>
					</div>

					{#if stats}
						<div
							class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-4 gap-4"
						>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Files Processed</p>
								<p class="text-lg font-semibold text-gray-900 dark:text-white">
									{stats.total_files_processed}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Successful</p>
								<p class="text-lg font-semibold text-green-600 dark:text-green-400">
									{stats.successful_files}
								</p>
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Failed</p>
								{#if stats.failed_files > 0}
									<button
										onclick={() => openFailedFilesModal(transform)}
										class="text-lg font-semibold text-red-600 dark:text-red-400 hover:underline cursor-pointer"
										title="Click to view failed files"
									>
										{stats.failed_files}
									</button>
								{:else}
									<p class="text-lg font-semibold text-green-600 dark:text-green-400">0</p>
								{/if}
							</div>
							<div>
								<p class="text-sm text-gray-600 dark:text-gray-400">Items Created</p>
								<p class="text-lg font-semibold text-blue-600 dark:text-blue-400">
									{stats.total_items_created}
								</p>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This action cannot be undone.`
		: ''}
	confirmLabel="Delete"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={() => (transformPendingDelete = null)}
/>

<!-- Failed Files Modal -->
{#if showFailedFilesModal}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
		<div
			class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-4xl w-full mx-4 max-h-[80vh] flex flex-col"
		>
			<div
				class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center"
			>
				<h3 class="text-lg font-semibold text-gray-900 dark:text-white">
					Failed Files - {failedFilesTransformTitle}
				</h3>
				<button
					onclick={closeFailedFilesModal}
					class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
					aria-label="Close modal"
				>
					<svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M6 18L18 6M6 6l12 12"
						/>
					</svg>
				</button>
			</div>

			<div class="p-6 overflow-y-auto flex-1">
				{#if loadingFailedFiles}
					<div class="flex justify-center py-8">
						<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
					</div>
				{:else if failedFiles.length === 0}
					<p class="text-gray-500 dark:text-gray-400 text-center py-8">No failed files found.</p>
				{:else}
					<div class="space-y-4">
						{#each failedFiles as file (file.file_key)}
							<div
								class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4"
							>
								<div class="flex items-start justify-between">
									<div class="flex-1 min-w-0">
										<p
											class="font-mono text-sm text-gray-900 dark:text-white truncate"
											title={file.file_key}
										>
											{file.file_key.split('/').pop() || file.file_key}
										</p>
										<p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
											Processed: {new Date(file.processed_at).toLocaleString()}
										</p>
									</div>
								</div>
								{#if file.process_error}
									<div class="mt-3 bg-red-100 dark:bg-red-900/40 rounded p-3">
										<p class="text-xs font-semibold text-red-700 dark:text-red-300 mb-1">Error:</p>
										<pre
											class="text-xs text-red-600 dark:text-red-400 whitespace-pre-wrap break-words font-mono">{file.process_error}</pre>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
				<button
					onclick={closeFailedFilesModal}
					class="px-4 py-2 bg-gray-100 text-gray-700 hover:bg-gray-200 rounded-lg dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
				>
					Close
				</button>
			</div>
		</div>
	</div>
{/if}
