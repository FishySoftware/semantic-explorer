<script lang="ts">
	import { Button, Modal, Tooltip } from 'flowbite-svelte';
	import { QuestionCircleSolid } from 'flowbite-svelte-icons';
	import { formatError, toastStore } from '../utils/notifications';

	interface Props {
		open?: boolean;
		collectionId?: number | null;
		collectionTitle?: string | null;
		onSuccess?: (_transformId: number, _transformTitle: string) => void;
	}

	interface Collection {
		collection_id: number;
		title: string;
		file_count?: number;
	}

	interface Dataset {
		dataset_id: number;
		title: string;
	}

	interface Embedder {
		embedder_id: number;
		name: string;
		provider: string;
	}

	interface PaginatedEmbedderList {
		items: Embedder[];
		total_count: number;
		limit: number;
		offset: number;
	}

	interface PaginatedDatasetList {
		items: Dataset[];
		total_count: number;
		limit: number;
		offset: number;
	}

	let {
		open = $bindable(false),
		collectionId = null,
		collectionTitle = null,
		onSuccess,
	}: Props = $props();

	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);

	let selectedCollectionId = $state<number | null>(null);
	let transformTitle = $state('');
	let datasetOption = $state<'new' | number>('new');
	let newDatasetName = $state('');
	let selectedEmbedderId = $state<number | null>(null);
	let autoCreateDatasetTransform = $state(false);
	let datasetTransformName = $state('');
	let selectedDatasetTransformEmbedderIds = $state<number[]>([]);
	let datasetTransformBatchSize = $state<number | null>(null);

	$effect(() => {
		if (open) {
			// Refetch collections when modal opens to ensure we have latest file counts
			fetchCollections();
			if (collectionId !== null) {
				selectedCollectionId = collectionId;
				// Use provided collectionTitle if available, otherwise look it up
				const title =
					collectionTitle || collections.find((c) => c.collection_id === collectionId)?.title;
				if (title) {
					newDatasetName = `${title}-dataset`;
					transformTitle = `${title}-transform`;
				}
			}
		}
	});

	$effect(() => {
		if (selectedCollectionId !== null) {
			const collection = collections.find((c) => c.collection_id === selectedCollectionId);
			if (collection) {
				transformTitle = `${collection.title}-transform`;
			}
		}
	});

	// Auto-generate dataset transform name when transform title changes
	$effect(() => {
		if (transformTitle && !datasetTransformName) {
			datasetTransformName = `${transformTitle}-embeddings`;
		}
	});

	// Extraction strategy
	let extractionStrategy = $state('plain_text');
	let preserveFormatting = $state(true);
	let extractTables = $state(true);
	let tableFormat = $state('plain_text');
	let preserveHeadings = $state(true);
	let headingFormat = $state('plain_text');
	let preserveLists = $state(true);
	let preserveCodeBlocks = $state(true);
	let includeMetadata = $state(true);
	let appendMetadataToText = $state(true);

	// Update table and heading format when extraction strategy changes
	$effect(() => {
		if (extractionStrategy === 'markdown') {
			tableFormat = 'markdown';
			headingFormat = 'markdown';
		} else if (extractionStrategy === 'plain_text') {
			tableFormat = 'plain_text';
			headingFormat = 'plain_text';
		}
	});

	// Chunking strategy
	let chunkingStrategy = $state('sentence');
	let chunkSize = $state(200);
	let chunkOverlap = $state(0);
	let minChunkSize = $state(50);
	let preserveSentenceBoundaries = $state(true);

	let loadingCollections = $state(true);
	let loadingDatasets = $state(true);
	let loadingEmbedders = $state(true);
	let isCreating = $state(false);
	let error = $state<string | null>(null);

	// Fetch data when modal opens
	$effect(() => {
		if (open) {
			fetchDatasets();
			fetchEmbedders();
		}
	});

	async function fetchCollections() {
		try {
			loadingCollections = true;
			const response = await fetch('/api/collections');
			if (!response.ok) throw new Error('Failed to fetch collections');
			const data = await response.json();
			const allCollections: Collection[] = data.collections ?? [];
			// Filter to only collections with files
			collections = allCollections.filter((c) => (c.file_count ?? 0) > 0);
		} catch (e) {
			console.error('Failed to fetch collections:', e);
		} finally {
			loadingCollections = false;
		}
	}

	async function fetchDatasets() {
		try {
			loadingDatasets = true;
			const response = await fetch('/api/datasets?limit=100&offset=0');
			if (!response.ok) throw new Error('Failed to fetch datasets');
			const data: PaginatedDatasetList = await response.json();
			datasets = data.items ?? [];
			// Default to first dataset if available, otherwise 'new'
			if (datasets.length > 0 && datasetOption === 'new') {
				// Keep 'new' as default, but first dataset is available as secondary option
			}
		} catch (e) {
			console.error('Failed to fetch datasets:', e);
		} finally {
			loadingDatasets = false;
		}
	}

	async function fetchEmbedders() {
		try {
			loadingEmbedders = true;
			const response = await fetch('/api/embedders?limit=10&offset=0');
			if (!response.ok) throw new Error('Failed to fetch embedders');
			const data: PaginatedEmbedderList = await response.json();
			// Handle paginated response from the API
			embedders = data.items || [];
		} catch (e) {
			console.error('Failed to fetch embedders:', e);
		} finally {
			loadingEmbedders = false;
		}
	}

	function toggleDatasetTransformEmbedder(embedderId: number) {
		if (selectedDatasetTransformEmbedderIds.includes(embedderId)) {
			selectedDatasetTransformEmbedderIds = selectedDatasetTransformEmbedderIds.filter(
				(id) => id !== embedderId
			);
		} else {
			selectedDatasetTransformEmbedderIds = [...selectedDatasetTransformEmbedderIds, embedderId];
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

		if (chunkingStrategy === 'semantic' && !selectedEmbedderId) {
			error = 'Embedder is required for semantic chunking';
			return;
		}

		if (autoCreateDatasetTransform && selectedDatasetTransformEmbedderIds.length === 0) {
			error = 'At least one embedder is required for dataset transform';
			return;
		}

		try {
			isCreating = true;

			// Create dataset if needed
			let targetDatasetId: number;
			if (datasetOption === 'new') {
				const datasetName = newDatasetName.trim() || `${transformTitle}-dataset`;
				const datasetResponse = await fetch('/api/datasets', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({
						title: datasetName,
						details: null,
						tags: [],
						is_public: false,
					}),
				});

				if (!datasetResponse.ok) {
					const errorText = await datasetResponse.text();
					throw new Error(`Failed to create dataset: ${datasetResponse.statusText} - ${errorText}`);
				}

				const newDataset = await datasetResponse.json();
				targetDatasetId = newDataset.dataset_id;
			} else {
				targetDatasetId = datasetOption as number;
			}

			const jobConfig: any = {
				extraction: {
					strategy: extractionStrategy,
					options: {
						preserve_formatting: preserveFormatting,
						extract_tables: extractTables,
						table_format: tableFormat,
						preserve_headings: preserveHeadings,
						heading_format: headingFormat,
						preserve_lists: preserveLists,
						preserve_code_blocks: preserveCodeBlocks,
						include_metadata: includeMetadata,
						append_metadata_to_text: appendMetadataToText,
					},
				},
				chunking: {
					strategy: chunkingStrategy,
					chunk_size: chunkSize,
					chunk_overlap: chunkOverlap,
					options: {
						preserve_sentence_boundaries: preserveSentenceBoundaries,
						min_chunk_size: minChunkSize,
						...(chunkingStrategy === 'semantic' && selectedEmbedderId
							? { semantic: { embedder_id: selectedEmbedderId } }
							: {}),
					},
				},
			};

			const response = await fetch('/api/collection-transforms', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					title: transformTitle.trim(),
					collection_id: selectedCollectionId,
					dataset_id: targetDatasetId,
					chunk_size: chunkSize,
					job_config: jobConfig,
				}),
			});

			if (!response.ok) {
				const errorText = await response.text();
				throw new Error(`Failed to create transform: ${response.statusText} - ${errorText}`);
			}

			const createdTransform = await response.json();
			const createdTransformId = createdTransform.collection_transform_id;
			const createdTransformTitle = createdTransform.title;

			// Create dataset transform if auto-create is enabled
			if (autoCreateDatasetTransform && selectedDatasetTransformEmbedderIds.length > 0) {
				try {
					const datasetTransformResponse = await fetch('/api/dataset-transforms', {
						method: 'POST',
						headers: { 'Content-Type': 'application/json' },
						body: JSON.stringify({
							title: datasetTransformName.trim() || `${transformTitle}-embeddings`,
							source_dataset_id: targetDatasetId,
							embedder_ids: selectedDatasetTransformEmbedderIds,
							embedding_batch_size: datasetTransformBatchSize,
						}),
					});

					if (!datasetTransformResponse.ok) {
						const errorText = await datasetTransformResponse.text();
						console.error('Failed to create dataset transform:', errorText, {
							title: `${transformTitle}-embeddings`,
							source_dataset_id: targetDatasetId,
							embedder_ids: selectedDatasetTransformEmbedderIds,
							embedding_batch_size: datasetTransformBatchSize,
						});
						toastStore.error(
							'Failed to create dataset transform. Please check your selections and try again.'
						);
					}
				} catch (e) {
					console.error('Error creating dataset transform:', e);
					toastStore.error('Error creating dataset transform. Please try again.');
				}
			}

			toastStore.success('Transform created successfully');
			resetForm();
			onSuccess?.(createdTransformId, createdTransformTitle);
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
		datasetOption = 'new';
		newDatasetName = '';
		selectedEmbedderId = null;
		autoCreateDatasetTransform = false;
		datasetTransformName = '';
		selectedDatasetTransformEmbedderIds = [];
		datasetTransformBatchSize = null;
		extractionStrategy = 'plain_text';
		preserveFormatting = false;
		extractTables = true;
		tableFormat = 'plain_text';
		preserveHeadings = false;
		headingFormat = 'plain_text';
		preserveLists = false;
		preserveCodeBlocks = false;
		includeMetadata = true;
		appendMetadataToText = true;
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
		// Reset selectedCollectionId so next open properly syncs with collectionId prop
		selectedCollectionId = null;
	}
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

		<div class="p-4 space-y-4 max-h-[70vh] overflow-y-auto">
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

			<!-- Dataset Selection/Creation -->
			<div>
				<label
					for="dataset-option"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					Target Dataset <span class="text-red-500">*</span>
				</label>
				{#if loadingDatasets}
					<div class="text-sm text-gray-500">Loading datasets...</div>
				{:else}
					<select
						id="dataset-option"
						bind:value={datasetOption}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					>
						<option value="new">Create new dataset</option>
						{#each datasets as dataset (dataset.dataset_id)}
							<option value={dataset.dataset_id}>{dataset.title}</option>
						{/each}
					</select>
				{/if}
			</div>

			<!-- New Dataset Name (shown when creating new) -->
			{#if datasetOption === 'new'}
				<div>
					<label
						for="new-dataset-name"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						New Dataset Name
					</label>
					<input
						id="new-dataset-name"
						type="text"
						bind:value={newDatasetName}
						placeholder="e.g., my-collection-dataset"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
					/>
				</div>
			{/if}

			<!-- Extraction Strategy -->
			<div>
				<div class="flex items-center gap-2">
					<label
						for="extraction-strategy"
						class="text-sm font-medium text-gray-700 dark:text-gray-300"
					>
						Extraction Strategy
					</label>
					<button
						type="button"
						id="extract-help"
						class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 focus:outline-none"
					>
						<QuestionCircleSolid class="w-4 h-4" />
					</button>
					<Tooltip
						triggeredBy="#extract-help"
						placement="right"
						class="max-w-xs text-center bg-gray-900 dark:bg-white text-white dark:text-gray-900 border-0"
					>
						Method for extracting text: Plain Text is fastest, Structure Preserving keeps
						formatting, Markdown converts to markdown
					</Tooltip>
				</div>
				<select
					id="extraction-strategy"
					bind:value={extractionStrategy}
					class="w-full mt-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				>
					<option value="plain_text">Plain Text</option>
					<option value="structure_preserving">Structure Preserving</option>
					<option value="markdown">Markdown</option>
				</select>
			</div>

			<!-- Extraction Options -->
			<div class="space-y-3">
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

				{#if extractTables}
					<div class="ml-6">
						<label
							for="table-format"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Table Format
						</label>
						<select
							id="table-format"
							bind:value={tableFormat}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						>
							<option value="plain_text">Plain Text</option>
							<option value="markdown">Markdown</option>
							<option value="csv">CSV</option>
						</select>
					</div>
				{/if}

				<label class="flex items-center gap-2 cursor-pointer">
					<input
						type="checkbox"
						bind:checked={preserveHeadings}
						class="w-4 h-4 text-blue-600 rounded focus:ring-2"
					/>
					<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Headings</span>
				</label>

				{#if preserveHeadings}
					<div class="ml-6">
						<label
							for="heading-format"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Heading Format
						</label>
						<select
							id="heading-format"
							bind:value={headingFormat}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						>
							<option value="plain_text">Plain Text</option>
							<option value="markdown">Markdown</option>
						</select>
					</div>
				{/if}

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

				<label class="flex items-center gap-2 cursor-pointer">
					<input
						type="checkbox"
						bind:checked={includeMetadata}
						class="w-4 h-4 text-blue-600 rounded focus:ring-2"
					/>
					<span class="text-sm text-gray-700 dark:text-gray-300">Extract Document Metadata</span>
					<button
						type="button"
						id="metadata-help"
						class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 focus:outline-none"
					>
						<QuestionCircleSolid class="w-4 h-4" />
					</button>
					<Tooltip
						triggeredBy="#metadata-help"
						placement="right"
						class="max-w-xs text-center bg-gray-900 dark:bg-white text-white dark:text-gray-900 border-0"
					>
						Extract metadata like author, title, dates from documents
					</Tooltip>
				</label>

				{#if includeMetadata}
					<label class="flex items-center gap-2 cursor-pointer ml-6">
						<input
							type="checkbox"
							bind:checked={appendMetadataToText}
							class="w-4 h-4 text-blue-600 rounded focus:ring-2"
						/>
						<span class="text-sm text-gray-700 dark:text-gray-300">Append Metadata to Text</span>
						<button
							type="button"
							id="append-metadata-help"
							class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 focus:outline-none"
						>
							<QuestionCircleSolid class="w-4 h-4" />
						</button>
						<Tooltip
							triggeredBy="#append-metadata-help"
							placement="right"
							class="max-w-xs text-center bg-gray-900 dark:bg-white text-white dark:text-gray-900 border-0"
						>
							Append metadata as text for better semantic search
						</Tooltip>
					</label>
				{/if}
			</div>

			<!-- Chunking Strategy -->
			<div>
				<div class="flex items-center gap-2">
					<label
						for="chunking-strategy"
						class="text-sm font-medium text-gray-700 dark:text-gray-300"
					>
						Chunking Strategy
					</label>
					<button
						type="button"
						id="chunk-help"
						class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 focus:outline-none"
					>
						<QuestionCircleSolid class="w-4 h-4" />
					</button>
					<Tooltip
						triggeredBy="#chunk-help"
						placement="right"
						class="max-w-xs text-center bg-gray-900 dark:bg-white text-white dark:text-gray-900 border-0"
					>
						How to break text: Sentence keeps natural boundaries, Fixed Size uses chunks, Semantic
						groups related content
					</Tooltip>
				</div>
				<select
					id="chunking-strategy"
					bind:value={chunkingStrategy}
					class="w-full mt-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
				>
					<option value="sentence">Sentence</option>
					<option value="fixed_size">Fixed Size</option>
					<option value="recursive_character">Recursive Character</option>
					<option value="semantic">Semantic</option>
					<option value="markdown_aware">Markdown Aware</option>
					<option value="table_aware">Table Aware</option>
					<option value="code_aware">Code Aware</option>
					<option value="token_based">Token Based</option>
				</select>
			</div>

			<!-- Embedders (shown only for semantic chunking) -->
			{#if chunkingStrategy === 'semantic'}
				<div>
					<label
						for="embedder-select"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Embedder <span class="text-red-500">*</span>
					</label>
					{#if loadingEmbedders}
						<div class="text-sm text-gray-500">Loading embedders...</div>
					{:else if embedders.length === 0}
						<div class="text-sm text-gray-500">No embedders available</div>
					{:else}
						<select
							id="embedder-select"
							bind:value={selectedEmbedderId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
						>
							<option value={null}>Select an embedder</option>
							{#each embedders as embedder (embedder.embedder_id)}
								<option value={embedder.embedder_id}>
									{embedder.name} ({embedder.provider})
								</option>
							{/each}
						</select>
					{/if}
				</div>
			{/if}

			<!-- Chunking Options -->
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
				<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Sentence Boundaries</span>
			</label>

			<!-- Dataset Transform Configuration -->
			<div class="border-t border-gray-300 dark:border-gray-600 pt-4 mt-4">
				<label class="flex items-center gap-2 cursor-pointer">
					<input
						type="checkbox"
						bind:checked={autoCreateDatasetTransform}
						class="w-4 h-4 text-blue-600 rounded focus:ring-2"
					/>
					<span class="text-sm font-medium text-gray-700 dark:text-gray-300">
						Auto-create Dataset Transform (Embeddings)
					</span>
				</label>

				{#if autoCreateDatasetTransform}
					<div class="mt-4 space-y-4 bg-gray-50 dark:bg-gray-900/20 p-4 rounded-lg">
						<!-- Dataset Transform Name -->
						<div>
							<label
								for="dataset-transform-name"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Dataset Transform Name
							</label>
							<input
								id="dataset-transform-name"
								type="text"
								bind:value={datasetTransformName}
								placeholder="e.g., my-transform-embeddings"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
							/>
						</div>

						<!-- Embedders Selection -->
						<div>
							<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
								Embedders <span class="text-red-500">*</span>
							</div>
							{#if loadingEmbedders}
								<div class="text-sm text-gray-500">Loading embedders...</div>
							{:else if embedders.length === 0}
								<div class="text-sm text-gray-500">No embedders available</div>
							{:else}
								<div
									class="space-y-2 border border-gray-300 dark:border-gray-600 rounded-lg p-3 dark:bg-gray-700"
								>
									{#each embedders as embedder (embedder.embedder_id)}
										<label class="flex items-center gap-2 cursor-pointer">
											<input
												type="checkbox"
												checked={selectedDatasetTransformEmbedderIds.includes(embedder.embedder_id)}
												onchange={() => toggleDatasetTransformEmbedder(embedder.embedder_id)}
												class="w-4 h-4 text-blue-600 rounded focus:ring-2"
											/>
											<span class="text-sm text-gray-700 dark:text-gray-300">
												{embedder.name}
												<span class="text-xs text-gray-500 dark:text-gray-400"
													>({embedder.provider})</span
												>
											</span>
										</label>
									{/each}
								</div>
							{/if}
						</div>

						<!-- Embedding Batch Size -->
						<div>
							<label
								for="dataset-transform-batch-size"
								class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
							>
								Embedding Batch Size <span class="text-xs text-gray-500 dark:text-gray-400"
									>(optional)</span
								>
							</label>
							<input
								id="dataset-transform-batch-size"
								type="number"
								bind:value={datasetTransformBatchSize}
								min="1"
								max="1000"
								placeholder="Leave empty for default"
								class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white text-sm"
							/>
							<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
								Number of embeddings to process per batch. Lower values use less memory, higher
								values process faster.
							</p>
						</div>
					</div>
				{/if}
			</div>
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
