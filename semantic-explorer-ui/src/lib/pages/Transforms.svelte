<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '../components/ConfirmDialog.svelte';
	import PageHeader from '../components/PageHeader.svelte';
	import { formatError, toastStore } from '../utils/notifications';

	type JobType = 'collection_to_dataset' | 'dataset_to_vector_storage';

	interface Transform {
		transform_id: number;
		title: string;
		collection_id: number | null;
		dataset_id: number;
		owner: string;
		is_enabled: boolean;
		chunk_size: number;
		job_type: JobType;
		source_dataset_id: number | null;
		target_dataset_id: number | null;
		embedder_ids: number[] | null;
		job_config: Record<string, any>;
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
		provider: string;
	}

	interface TransformStats {
		transform_id: number;
		total_files_in_collection?: number;
		total_files_processed?: number;
		successful_files?: number;
		failed_files?: number;
		total_items_created?: number;
		total_items_processed?: number;
		successful_items?: number;
		failed_items?: number;
		total_chunks_embedded?: number;
		total_chunks_failed?: number;
	}

	interface ProcessedFile {
		id: number;
		transform_id: number;
		file_key: string;
		processed_at: string;
		item_count: number;
		process_status: string;
		process_error: string | null;
		processing_duration_ms: number | null;
	}

	let transforms = $state<Transform[]>([]);
	let collections = $state<Collection[]>([]);
	let datasets = $state<Dataset[]>([]);
	let embedders = $state<Embedder[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let showCreateForm = $state(false);
	let newJobType = $state<JobType>('collection_to_dataset');
	let newTitle = $state('');
	let newCollectionId = $state<number | null>(null);
	let newDatasetId = $state<number | null>(null);
	let newChunkSize = $state(200);
	let newEmbedderIds = $state<number[]>([]);
	let creating = $state(false);
	let createError = $state<string | null>(null);
	let extractionStrategy = $state<'plain_text' | 'structure_preserving' | 'markdown'>('plain_text');
	let preserveFormatting = $state(false);
	let extractTables = $state(true);
	let tableFormat = $state<'markdown' | 'csv' | 'plain_text'>('plain_text');
	let preserveHeadings = $state(false);
	let headingFormat = $state<'markdown' | 'plain_text'>('plain_text');
	let preserveLists = $state(false);
	let preserveCodeBlocks = $state(false);
	let includeMetadata = $state(false);
	let chunkingStrategy = $state<
		'sentence' | 'recursive_character' | 'semantic' | 'fixed_size' | 'markdown_aware'
	>('sentence');
	let chunkOverlap = $state(0);
	let semanticEmbedderId = $state<number | null>(null);
	let semanticSimilarityThreshold = $state(0.7);
	let semanticMinChunkSize = $state(50);
	let semanticMaxChunkSize = $state(500);
	let semanticBufferSize = $state(1);
	let recursiveSeparators = $state('\n\n, \n, . , , ');
	let recursiveKeepSeparator = $state(true);
	let markdownSplitOnHeaders = $state(true);
	let markdownPreserveCodeBlocks = $state(false);
	let preserveSentenceBoundaries = $state(true);
	let trimWhitespace = $state(true);
	let minChunkSize = $state(50);

	$effect(() => {
		if (showCreateForm && !newTitle) {
			const timestamp = new Date().toISOString().replace(/[-:.]/g, '').slice(0, 15);
			newTitle = `transform-${timestamp}`;
		}
	});

	let editingTransform = $state<Transform | null>(null);
	let editTitle = $state('');
	let editEnabled = $state(false);
	let updating = $state(false);
	let updateError = $state<string | null>(null);

	let deleting = $state<number | null>(null);
	let transformPendingDelete = $state<Transform | null>(null);

	let expandedTransformId = $state<number | null>(null);
	let expandedConfigs = $state<Record<string, boolean>>({});
	let transformStats = $state<Record<number, TransformStats>>({});
	let processedFiles = $state<Record<number, ProcessedFile[]>>({});
	let loadingStats = $state<Record<number, boolean>>({});
	let triggering = $state<Record<number, boolean>>({});

	let autoRefreshInterval: ReturnType<typeof setInterval> | null = null;
	let autoRefreshTransformsInterval: ReturnType<typeof setInterval> | null = null;
	const AUTO_REFRESH_INTERVAL_MS = 4000;

	async function fetchTransforms(
		showToastOnError: boolean = true,
		isBackgroundRefresh: boolean = false
	) {
		try {
			if (!isBackgroundRefresh) {
				loading = true;
				error = null;
			}
			const response = await fetch('/api/transforms');
			if (!response.ok) {
				throw new Error(`Failed to fetch transforms: ${response.statusText}`);
			}
			const newTransforms: Transform[] = await response.json();

			if (transforms.length > 0) {
				// Update existing transforms in-place (only changed fields)
				newTransforms.forEach((newTransform: Transform) => {
					const existingIndex = transforms.findIndex(
						(t) => t.transform_id === newTransform.transform_id
					);
					if (existingIndex >= 0) {
						const existing = transforms[existingIndex];
						// Only update fields that have changed to minimize re-renders
						if (existing.title !== newTransform.title) existing.title = newTransform.title;
						if (existing.is_enabled !== newTransform.is_enabled)
							existing.is_enabled = newTransform.is_enabled;
						if (existing.chunk_size !== newTransform.chunk_size)
							existing.chunk_size = newTransform.chunk_size;
						if (existing.updated_at !== newTransform.updated_at)
							existing.updated_at = newTransform.updated_at;
						if (JSON.stringify(existing.job_config) !== JSON.stringify(newTransform.job_config)) {
							existing.job_config = newTransform.job_config;
						}
						if (
							JSON.stringify(existing.embedder_ids) !== JSON.stringify(newTransform.embedder_ids)
						) {
							existing.embedder_ids = newTransform.embedder_ids;
						}
					} else {
						// New transform - add it
						transforms = [...transforms, newTransform];
					}
				});
				// Remove transforms that no longer exist
				const removedTransforms = transforms.filter(
					(t) => !newTransforms.some((nt: Transform) => nt.transform_id === t.transform_id)
				);
				if (removedTransforms.length > 0) {
					transforms = transforms.filter((t) =>
						newTransforms.some((nt: Transform) => nt.transform_id === t.transform_id)
					);
				}
			} else {
				transforms = newTransforms;
			}
		} catch (e) {
			const message = formatError(e, 'Failed to fetch transforms');
			if (!isBackgroundRefresh) {
				error = message;
			}
			if (showToastOnError) {
				toastStore.error(message);
			}
		} finally {
			if (!isBackgroundRefresh) {
				loading = false;
			}
		}
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
			toastStore.error(formatError(e, 'Failed to fetch collections'));
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
			toastStore.error(formatError(e, 'Failed to fetch datasets'));
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
			toastStore.error(formatError(e, 'Failed to fetch embedders'));
		}
	}

	async function fetchDatasetEmbedders() {
		try {
			const response = await fetch('/api/datasets/embedders');
			if (!response.ok) {
				throw new Error(`Failed to fetch dataset embedders: ${response.statusText}`);
			}
			await response.json();
		} catch (e) {
			console.error('Failed to fetch dataset embedders:', e);
			toastStore.error(formatError(e, 'Failed to fetch dataset embedders'));
		}
	}

	async function createTransform() {
		createError = null;

		if (!newTitle.trim()) {
			createError = 'Title is required';
			return;
		}

		let body: any = {
			title: newTitle,
			job_type: newJobType,
		};

		if (newJobType === 'collection_to_dataset') {
			if (!newCollectionId) {
				createError = 'Collection is required';
				return;
			}
			if (!newDatasetId) {
				createError = 'Dataset is required';
				return;
			}
			if (chunkingStrategy === 'semantic' && !semanticEmbedderId) {
				createError = 'Embedder is required for semantic chunking';
				return;
			}
			body.collection_id = newCollectionId;
			body.dataset_id = newDatasetId;
			body.chunk_size = newChunkSize;

			// Build extraction config
			const extractionConfig: any = {
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
				},
			};

			// Build chunking config
			const chunkingConfig: any = {
				strategy: chunkingStrategy,
				chunk_size: newChunkSize,
				chunk_overlap: chunkOverlap,
				options: {
					preserve_sentence_boundaries: preserveSentenceBoundaries,
					trim_whitespace: trimWhitespace,
					min_chunk_size: minChunkSize,
				},
			};

			// Add strategy-specific options
			if (chunkingStrategy === 'semantic') {
				chunkingConfig.embedder_id = semanticEmbedderId;
				chunkingConfig.options.semantic = {
					embedder_id: semanticEmbedderId,
					similarity_threshold: semanticSimilarityThreshold,
					min_chunk_size: semanticMinChunkSize,
					max_chunk_size: semanticMaxChunkSize,
					buffer_size: semanticBufferSize,
				};
			} else if (chunkingStrategy === 'recursive_character') {
				const separators = recursiveSeparators
					.split(',')
					.map((s) => s.trim())
					.filter((s) => s);
				chunkingConfig.options.recursive_character = {
					separators,
					keep_separator: recursiveKeepSeparator,
				};
			} else if (chunkingStrategy === 'markdown_aware') {
				chunkingConfig.options.markdown_aware = {
					split_on_headers: markdownSplitOnHeaders,
					preserve_code_blocks: markdownPreserveCodeBlocks,
				};
			}

			// Add to body as job_config
			body.job_config = {
				extraction: extractionConfig,
				chunking: chunkingConfig,
			};
		} else if (newJobType === 'dataset_to_vector_storage') {
			if (!newDatasetId) {
				createError = 'Dataset is required';
				return;
			}
			if (newEmbedderIds.length === 0) {
				createError = 'At least one embedder is required';
				return;
			}
			body.dataset_id = newDatasetId;
			body.embedder_ids = newEmbedderIds;
		}

		try {
			creating = true;
			createError = null;
			const response = await fetch('/api/transforms', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify(body),
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || `Failed to create transform: ${response.statusText}`);
			}

			const newTransform = await response.json();
			transforms = [...transforms, newTransform];
			toastStore.success('Transform created');

			resetCreateForm();
			showCreateForm = false;
			// Expand the newly created transform to show its details
			await toggleExpand(newTransform.transform_id);
		} catch (e) {
			const message = formatError(e, 'Failed to create transform');
			createError = message;
			toastStore.error(message);
		} finally {
			creating = false;
		}
	}

	function resetCreateForm() {
		newTitle = '';
		newJobType = 'collection_to_dataset';
		newCollectionId = null;
		newDatasetId = null;
		newChunkSize = 200;
		newEmbedderIds = [];
		createError = null;

		// Reset extraction config
		extractionStrategy = 'plain_text';
		preserveFormatting = false;
		extractTables = true;
		tableFormat = 'plain_text';
		preserveHeadings = false;
		headingFormat = 'plain_text';
		preserveLists = false;
		preserveCodeBlocks = false;
		includeMetadata = false;

		// Reset chunking config
		chunkingStrategy = 'sentence';
		chunkOverlap = 0;
		semanticEmbedderId = null;
		semanticSimilarityThreshold = 0.7;
		semanticMinChunkSize = 50;
		semanticMaxChunkSize = 500;
		semanticBufferSize = 1;
		recursiveSeparators = '\\n\\n, \\n, . , , ';
		recursiveKeepSeparator = true;
		markdownSplitOnHeaders = true;
		markdownPreserveCodeBlocks = false;
		preserveSentenceBoundaries = true;
		trimWhitespace = true;
		minChunkSize = 50;
	}

	function startEdit(transform: Transform) {
		editingTransform = transform;
		editTitle = transform.title;
		editEnabled = transform.is_enabled;
		updateError = null;
	}

	function cancelEdit() {
		editingTransform = null;
		editTitle = '';
		editEnabled = false;
		updateError = null;
	}

	async function updateTransform() {
		if (!editingTransform) return;

		if (!editTitle.trim()) {
			updateError = 'Title is required';
			return;
		}

		try {
			updating = true;
			updateError = null;
			const response = await fetch(`/api/transforms/${editingTransform.transform_id}`, {
				method: 'PUT',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({
					title: editTitle,
					enabled: editEnabled,
				}),
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || `Failed to update transform: ${response.statusText}`);
			}

			const updatedTransform = await response.json();
			transforms = transforms.map((t) =>
				t.transform_id === updatedTransform.transform_id ? updatedTransform : t
			);

			cancelEdit();
			toastStore.success('Transform updated');
		} catch (e) {
			const message = formatError(e, 'Failed to update transform');
			updateError = message;
			toastStore.error(message);
		} finally {
			updating = false;
		}
	}

	function requestDeleteTransform(transform: Transform) {
		transformPendingDelete = transform;
	}

	function cancelDeleteTransform() {
		transformPendingDelete = null;
	}

	async function confirmDeleteTransform() {
		if (!transformPendingDelete) {
			return;
		}

		const target = transformPendingDelete;
		transformPendingDelete = null;

		try {
			deleting = target.transform_id;
			const response = await fetch(`/api/transforms/${target.transform_id}`, {
				method: 'DELETE',
			});

			if (!response.ok) {
				let errorMessage: string | null = null;
				try {
					const errorPayload = await response.json();
					errorMessage = errorPayload?.error ?? null;
				} catch {
					// Ignore JSON parse errors
				}
				throw new Error(errorMessage ?? `Failed to delete transform: ${response.statusText}`);
			}

			transforms = transforms.filter((t) => t.transform_id !== target.transform_id);
			toastStore.success('Transform deleted');
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to delete transform'));
		} finally {
			deleting = null;
		}
	}

	async function toggleExpand(transformId: number) {
		if (expandedTransformId === transformId) {
			expandedTransformId = null;
			return;
		}

		fetchEmbedders();
		expandedTransformId = transformId;

		if (!transformStats[transformId]) {
			await fetchTransformStats(transformId);
		}
		if (!processedFiles[transformId]) {
			await fetchProcessedFiles(transformId);
		}
	}

	async function fetchTransformStats(transformId: number, isBackgroundRefresh: boolean = false) {
		try {
			if (!isBackgroundRefresh) {
				loadingStats[transformId] = true;
			}
			const response = await fetch(`/api/transforms/${transformId}/stats`);
			if (!response.ok) {
				throw new Error(`Failed to fetch stats: ${response.statusText}`);
			}
			const stats = await response.json();

			if (transformStats[transformId]) {
				if ('total_files_processed' in stats)
					transformStats[transformId].total_files_processed = stats.total_files_processed;
				if ('successful_files' in stats)
					transformStats[transformId].successful_files = stats.successful_files;
				if ('failed_files' in stats) transformStats[transformId].failed_files = stats.failed_files;
				if ('total_items_created' in stats)
					transformStats[transformId].total_items_created = stats.total_items_created;
				if ('total_items_processed' in stats)
					transformStats[transformId].total_items_processed = stats.total_items_processed;
				if ('successful_items' in stats)
					transformStats[transformId].successful_items = stats.successful_items;
				if ('failed_items' in stats) transformStats[transformId].failed_items = stats.failed_items;
				if ('total_chunks_embedded' in stats)
					transformStats[transformId].total_chunks_embedded = stats.total_chunks_embedded;
				if ('total_chunks_failed' in stats)
					transformStats[transformId].total_chunks_failed = stats.total_chunks_failed;
			} else {
				transformStats[transformId] = stats;
			}
		} catch (e) {
			console.error('Failed to fetch transform stats:', e);
		} finally {
			if (!isBackgroundRefresh) {
				loadingStats[transformId] = false;
			}
		}
	}

	async function fetchProcessedFiles(transformId: number) {
		try {
			const response = await fetch(`/api/transforms/${transformId}/processed-files`);
			if (!response.ok) {
				throw new Error(`Failed to fetch processed files: ${response.statusText}`);
			}
			const files = await response.json();

			if (processedFiles[transformId]) {
				processedFiles[transformId] = files;
			} else {
				processedFiles[transformId] = files;
			}
		} catch (e) {
			console.error('Failed to fetch processed files:', e);
			if (!processedFiles[transformId]) {
				processedFiles[transformId] = [];
			}
		}
	}

	async function triggerTransform(transformId: number) {
		try {
			triggering[transformId] = true;
			const response = await fetch('/api/transforms/trigger', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
				},
				body: JSON.stringify({ transform_id: transformId }),
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Failed to trigger transform');
			}

			const transform = transforms.find((t) => t.transform_id === transformId);
			const message =
				transform?.job_type === 'dataset_to_vector_storage'
					? 'Embedding job triggered successfully. Dataset items will be embedded shortly.'
					: 'Transform scan triggered successfully. New files will be processed shortly.';
			toastStore.success(message, 'Transform triggered', 7000);

			setTimeout(() => {
				fetchTransformStats(transformId, true);
				fetchProcessedFiles(transformId);
			}, 1000);
		} catch (e) {
			toastStore.error(formatError(e, 'Failed to trigger transform'));
		} finally {
			triggering[transformId] = false;
		}
	}

	onMount(() => {
		fetchTransforms();
		fetchCollections();
		fetchDatasets();
		fetchEmbedders();
		fetchDatasetEmbedders();

		autoRefreshTransformsInterval = setInterval(async () => {
			await fetchTransforms(false, true);
		}, AUTO_REFRESH_INTERVAL_MS);

		autoRefreshInterval = setInterval(async () => {
			if (expandedTransformId !== null) {
				const transformExists = transforms.some((t) => t.transform_id === expandedTransformId);
				if (transformExists) {
					await fetchTransformStats(expandedTransformId, true);
					await fetchProcessedFiles(expandedTransformId);
				} else {
					expandedTransformId = null;
				}
			}
		}, AUTO_REFRESH_INTERVAL_MS);

		return () => {
			if (autoRefreshInterval !== null) {
				clearInterval(autoRefreshInterval);
			}
			if (autoRefreshTransformsInterval !== null) {
				clearInterval(autoRefreshTransformsInterval);
			}
		};
	});

	function getCollectionTitle(collectionId: number | null): string {
		if (!collectionId) return 'N/A';
		return (
			collections.find((c) => c.collection_id === collectionId)?.title ||
			`Collection ${collectionId}`
		);
	}

	function getDatasetTitle(datasetId: number | null): string {
		if (!datasetId) return 'N/A';
		return datasets.find((d) => d.dataset_id === datasetId)?.title || `Dataset ${datasetId}`;
	}

	function getEmbedderName(embedderId: number): string {
		return embedders.find((e) => e.embedder_id === embedderId)?.name || `Embedder ${embedderId}`;
	}

	function getAvailableEmbedders(): Embedder[] {
		// For dataset_to_vector_storage transforms, show all embedders
		// since users should be able to select any embedder when creating a new transform
		return embedders;
	}

	function getEmbedderFromFileKey(fileKey: string, transform: Transform): Embedder | null {
		if (transform.job_type === 'dataset_to_vector_storage' && transform.embedder_ids) {
			const match = fileKey.match(/batches\/(\d+)_/);
			if (match) {
				const embedderId = parseInt(match[1], 10);
				return embedders.find((e) => e.embedder_id === embedderId) || null;
			}
		}
		return null;
	}

	function getJobTypeLabel(jobType: JobType): string {
		const labels: Record<JobType, string> = {
			collection_to_dataset: 'Collection → Dataset',
			dataset_to_vector_storage: 'Dataset → Vector Storage',
		};
		return labels[jobType] || jobType;
	}

	function formatDate(dateStr: string): string {
		const date = new Date(dateStr);
		return date.toLocaleString();
	}

	function formatCount(count: number | undefined): string {
		if (count === undefined || count === null) return '0';
		if (count < 1000) return count.toString();
		if (count < 1000000) return (count / 1000).toFixed(1).replace(/\.0$/, '') + 'K';
		if (count < 1000000000) return (count / 1000000).toFixed(1).replace(/\.0$/, '') + 'M';
		return (count / 1000000000).toFixed(1).replace(/\.0$/, '') + 'B';
	}
</script>

<div class="max-w-7xl mx-auto">
	<PageHeader
		title="Transforms"
		description="Provides configurable pipelines to accomplish various operations. Collection transforms extract text from documents and generate chunks to populate a dataset. Dataset transforms populate embedded datasets—one dataset can have multiple embedders configured to populate multiple vector stores for comparison purposes."
	/>

	<div class="flex justify-between items-center mb-6">
		<h1 class="text-3xl font-bold text-gray-900 dark:text-white">Transforms</h1>
		<button
			onclick={() => (showCreateForm = !showCreateForm)}
			class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
		>
			{showCreateForm ? 'Cancel' : 'Create Transform'}
		</button>
	</div>

	{#if showCreateForm}
		<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6">
			<h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Create New Transform</h2>

			{#if createError}
				<div
					class="mb-4 p-3 bg-red-100 dark:bg-red-900/30 border border-red-400 text-red-700 dark:text-red-400 rounded"
				>
					{createError}
				</div>
			{/if}

			<form
				onsubmit={(e) => {
					e.preventDefault();
					createTransform();
				}}
				class="space-y-4"
			>
				<div>
					<label
						for="job-type"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Transform Type
					</label>
					<select
						id="job-type"
						bind:value={newJobType}
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
								  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
								  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
					>
						<option value="collection_to_dataset"
							>Collection → Dataset (Extract & Chunk Files)</option
						>
						<option value="dataset_to_vector_storage"
							>Dataset → Vector Storage (Generate Embeddings)</option
						>
					</select>
					<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
						{#if newJobType === 'collection_to_dataset'}
							Extract text from files in a collection, chunk them, and store in a dataset
						{:else if newJobType === 'dataset_to_vector_storage'}
							Generate embeddings from dataset items using multiple embedders and store in vector
							database
						{/if}
					</p>
				</div>

				<div>
					<label
						for="transform-title"
						class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
					>
						Title
					</label>
					<input
						id="transform-title"
						type="text"
						bind:value={newTitle}
						placeholder="My Transform"
						class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
								  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
								  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
					/>
				</div>

				{#if newJobType === 'collection_to_dataset'}
					<div>
						<label
							for="transform-collection"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Source Collection
						</label>
						<select
							id="transform-collection"
							bind:value={newCollectionId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
									  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
									  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
						>
							<option value={null}>Select a collection...</option>
							{#each collections as collection (collection.collection_id)}
								<option value={collection.collection_id}>
									{collection.title}
								</option>
							{/each}
						</select>
					</div>

					<div>
						<label
							for="transform-dataset"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Target Dataset
						</label>
						<select
							id="transform-dataset"
							bind:value={newDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
									  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
									  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
						>
							<option value={null}>Select a dataset...</option>
							{#each datasets as dataset (dataset.dataset_id)}
								<option value={dataset.dataset_id}>
									{dataset.title}
								</option>
							{/each}
						</select>
					</div>

					<div>
						<label
							for="chunk-size"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Chunk Size
						</label>
						<input
							id="chunk-size"
							type="number"
							bind:value={newChunkSize}
							min="100"
							max="10000"
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
									  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
									  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
						/>
					</div>

					<!-- Extraction Configuration -->
					<div class="border-t border-gray-200 dark:border-gray-700 pt-4 mt-4">
						<h3 class="text-lg font-medium text-gray-900 dark:text-white mb-3">
							Extraction Configuration
						</h3>

						<div class="space-y-4">
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
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
											  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
											  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
								>
									<option value="plain_text">Plain Text</option>
									<option value="structure_preserving">Structure Preserving</option>
									<option value="markdown">Markdown</option>
								</select>
								<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
									{#if extractionStrategy === 'plain_text'}
										Extract raw text without formatting
									{:else if extractionStrategy === 'structure_preserving'}
										Preserve document structure (headings, lists, tables)
									{:else if extractionStrategy === 'markdown'}
										Convert to Markdown format
									{/if}
								</p>
							</div>

							<div class="grid grid-cols-2 gap-4">
								<label class="flex items-center space-x-2">
									<input
										type="checkbox"
										bind:checked={preserveFormatting}
										class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Formatting</span>
								</label>

								<label class="flex items-center space-x-2">
									<input
										type="checkbox"
										bind:checked={extractTables}
										class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">Extract Tables</span>
								</label>

								<label class="flex items-center space-x-2">
									<input
										type="checkbox"
										bind:checked={preserveHeadings}
										class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Headings</span>
								</label>

								<label class="flex items-center space-x-2">
									<input
										type="checkbox"
										bind:checked={preserveLists}
										class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Lists</span>
								</label>

								<label class="flex items-center space-x-2">
									<input
										type="checkbox"
										bind:checked={preserveCodeBlocks}
										class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">Preserve Code Blocks</span>
								</label>

								<label class="flex items-center space-x-2">
									<input
										type="checkbox"
										bind:checked={includeMetadata}
										class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
									/>
									<span class="text-sm text-gray-700 dark:text-gray-300">Include Metadata</span>
								</label>
							</div>

							{#if extractTables}
								<div>
									<label
										for="table-format"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Table Format
									</label>
									<select
										id="table-format"
										bind:value={tableFormat}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
												  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
												  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
									>
										<option value="plain_text">Plain Text</option>
										<option value="markdown">Markdown</option>
										<option value="csv">CSV</option>
									</select>
								</div>
							{/if}

							{#if preserveHeadings}
								<div>
									<label
										for="heading-format"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Heading Format
									</label>
									<select
										id="heading-format"
										bind:value={headingFormat}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
												  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
												  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
									>
										<option value="plain_text">Plain Text</option>
										<option value="markdown">Markdown</option>
									</select>
								</div>
							{/if}
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
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Chunking Strategy
								</label>
								<select
									id="chunking-strategy"
									bind:value={chunkingStrategy}
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
											  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
											  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
								>
									<option value="sentence">Sentence</option>
									<option value="recursive_character">Recursive Character</option>
									<option value="semantic">Semantic</option>
									<option value="fixed_size">Fixed Size</option>
									<option value="markdown_aware">Markdown Aware</option>
								</select>
								<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
									{#if chunkingStrategy === 'sentence'}
										Split by sentences for natural language
									{:else if chunkingStrategy === 'recursive_character'}
										Split by separators recursively
									{:else if chunkingStrategy === 'semantic'}
										Group by semantic similarity
									{:else if chunkingStrategy === 'fixed_size'}
										Fixed character size chunks
									{:else if chunkingStrategy === 'markdown_aware'}
										Split by markdown structure
									{/if}
								</p>
							</div>

							<div>
								<label
									for="chunk-overlap"
									class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
								>
									Chunk Overlap
								</label>
								<input
									id="chunk-overlap"
									type="number"
									bind:value={chunkOverlap}
									min="0"
									max="500"
									class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
											  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
											  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
								/>
								<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
									Number of overlapping characters between chunks
								</p>
							</div>

							{#if chunkingStrategy === 'semantic'}
								<div
									class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-md p-4 space-y-3"
								>
									<p class="text-sm font-medium text-blue-900 dark:text-blue-300">
										Semantic Chunking Options
									</p>

									<div>
										<label
											for="semantic-embedder"
											class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
										>
											Embedder for Semantic Chunking
										</label>
										<select
											id="semantic-embedder"
											bind:value={semanticEmbedderId}
											class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
													  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
													  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
										>
											<option value={null}>Select an embedder...</option>
											{#each embedders as embedder (embedder.embedder_id)}
												<option value={embedder.embedder_id}>
													{embedder.name} ({embedder.provider})
												</option>
											{/each}
										</select>
									</div>

									<div class="grid grid-cols-2 gap-3">
										<div>
											<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
												Similarity Threshold
											</div>
											<input
												type="number"
												bind:value={semanticSimilarityThreshold}
												min="0"
												max="1"
												step="0.1"
												class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
														  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
														  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
											/>
										</div>

										<div>
											<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
												Buffer Size
											</div>
											<input
												type="number"
												bind:value={semanticBufferSize}
												min="1"
												max="10"
												class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
														  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
														  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
											/>
										</div>

										<div>
											<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
												Min Chunk Size
											</div>
											<input
												type="number"
												bind:value={semanticMinChunkSize}
												min="10"
												max="1000"
												class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
														  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
														  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
											/>
										</div>

										<div>
											<div class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
												Max Chunk Size
											</div>
											<input
												type="number"
												bind:value={semanticMaxChunkSize}
												min="100"
												max="10000"
												class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
														  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
														  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
											/>
										</div>
									</div>
								</div>
							{:else if chunkingStrategy === 'recursive_character'}
								<div
									class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-md p-4 space-y-3"
								>
									<p class="text-sm font-medium text-green-900 dark:text-green-300">
										Recursive Character Options
									</p>

									<div>
										<label
											for="recursive-separators"
											class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
										>
											Separators (comma-separated)
										</label>
										<input
											id="recursive-separators"
											type="text"
											bind:value={recursiveSeparators}
											placeholder="\\n\\n, \\n, . , , "
											class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
													  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
													  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
										/>
										<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
											Separators to split text, tried in order
										</p>
									</div>

									<label class="flex items-center space-x-2">
										<input
											type="checkbox"
											bind:checked={recursiveKeepSeparator}
											class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
										/>
										<span class="text-sm text-gray-700 dark:text-gray-300"
											>Keep Separator in Chunks</span
										>
									</label>
								</div>
							{:else if chunkingStrategy === 'markdown_aware'}
								<div
									class="bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-800 rounded-md p-4 space-y-3"
								>
									<p class="text-sm font-medium text-purple-900 dark:text-purple-300">
										Markdown Aware Options
									</p>

									<label class="flex items-center space-x-2">
										<input
											type="checkbox"
											bind:checked={markdownSplitOnHeaders}
											class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
										/>
										<span class="text-sm text-gray-700 dark:text-gray-300">Split on Headers</span>
									</label>

									<label class="flex items-center space-x-2">
										<input
											type="checkbox"
											bind:checked={markdownPreserveCodeBlocks}
											class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
										/>
										<span class="text-sm text-gray-700 dark:text-gray-300"
											>Preserve Code Blocks</span
										>
									</label>
								</div>
							{/if}

							<div class="grid grid-cols-2 gap-4">
								<div>
									<label
										for="min-chunk-size"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Minimum Chunk Size
									</label>
									<input
										id="min-chunk-size"
										type="number"
										bind:value={minChunkSize}
										min="10"
										max="1000"
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
												  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
												  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
									/>
								</div>

								<div class="flex items-center">
									<label class="flex items-center space-x-2">
										<input
											type="checkbox"
											bind:checked={preserveSentenceBoundaries}
											class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
										/>
										<span class="text-sm text-gray-700 dark:text-gray-300"
											>Preserve Sentence Boundaries</span
										>
									</label>
								</div>
							</div>

							<label class="flex items-center space-x-2">
								<input
									type="checkbox"
									bind:checked={trimWhitespace}
									class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
								/>
								<span class="text-sm text-gray-700 dark:text-gray-300">Trim Whitespace</span>
							</label>
						</div>
					</div>
				{:else if newJobType === 'dataset_to_vector_storage'}
					<div>
						<label
							for="dataset"
							class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
						>
							Source Dataset
						</label>
						<select
							id="dataset"
							bind:value={newDatasetId}
							class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
									  bg-white dark:bg-gray-700 text-gray-900 dark:text-white
									  focus:ring-2 focus:ring-blue-500 focus:border-transparent"
						>
							<option value={null}>Select a dataset...</option>
							{#each datasets as dataset (dataset.dataset_id)}
								<option value={dataset.dataset_id}>
									{dataset.title}
								</option>
							{/each}
						</select>
					</div>

					<fieldset>
						<legend class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
							Embedders (select multiple)
						</legend>
						<div
							class="space-y-2 max-h-48 overflow-y-auto border border-gray-300 dark:border-gray-600 rounded-md p-3 bg-gray-50 dark:bg-gray-900"
						>
							{#if !newDatasetId}
								<p class="text-sm text-gray-500 dark:text-gray-400">
									Select a dataset above to view available embedders.
								</p>
							{:else if getAvailableEmbedders().length === 0}
								<p class="text-sm text-gray-500 dark:text-gray-400">
									No embedders configured for this dataset. <a
										href="#/embedders"
										class="text-blue-600 dark:text-blue-400 hover:underline">Create one first</a
									>.
								</p>
							{:else}
								{#each getAvailableEmbedders() as embedder (embedder.embedder_id)}
									<label
										class="flex items-center space-x-2 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 p-2 rounded"
									>
										<input
											type="checkbox"
											bind:group={newEmbedderIds}
											value={embedder.embedder_id}
											class="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 rounded"
										/>
										<span class="text-sm text-gray-900 dark:text-white">
											{embedder.name}
											<span class="text-xs text-gray-500 dark:text-gray-400">
												({embedder.provider})
											</span>
										</span>
									</label>
								{/each}
							{/if}
						</div>
						<p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
							Selected: {newEmbedderIds.length} embedder{newEmbedderIds.length !== 1 ? 's' : ''}
						</p>
					</fieldset>
				{/if}

				<div class="flex gap-3">
					<button
						type="submit"
						disabled={creating}
						class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700
							   disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
					>
						{creating ? 'Creating...' : 'Create Transform'}
					</button>
					<button
						type="button"
						onclick={() => {
							resetCreateForm();
							showCreateForm = false;
						}}
						class="px-4 py-2 bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-300
							   rounded-md hover:bg-gray-400 dark:hover:bg-gray-500 transition-colors"
					>
						Cancel
					</button>
				</div>
			</form>
		</div>
	{/if}

	{#if loading}
		<div class="text-center py-12">
			<div
				class="inline-block animate-spin rounded-full h-8 w-8 border-4 border-gray-300 border-t-blue-600"
			></div>
			<p class="mt-2 text-gray-600 dark:text-gray-400">Loading transforms...</p>
		</div>
	{:else if error}
		<div
			class="bg-red-100 dark:bg-red-900/30 border border-red-400 text-red-700 dark:text-red-400 px-4 py-3 rounded"
		>
			{error}
		</div>
	{:else if transforms.length === 0}
		<div class="bg-gray-100 dark:bg-gray-800 rounded-lg p-8 text-center">
			<svg
				class="mx-auto h-12 w-12 text-gray-400"
				fill="none"
				stroke="currentColor"
				viewBox="0 0 24 24"
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"
				/>
			</svg>
			<p class="mt-2 text-gray-600 dark:text-gray-400">No transforms yet</p>
			<p class="text-sm text-gray-500 dark:text-gray-500 mt-1">Create transforms to:</p>
			<ul class="text-sm text-gray-500 dark:text-gray-500 mt-2 space-y-1">
				<li>• Extract and chunk files from collections into datasets</li>
				<li>• Process datasets through external APIs</li>
				<li>• Generate embeddings</li>
			</ul>
		</div>
	{:else}
		<div class="space-y-4">
			{#each transforms as transform (transform.transform_id)}
				<div class="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden">
					{#if editingTransform?.transform_id === transform.transform_id}
						<div class="p-6">
							<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">
								Edit Transform
							</h3>

							{#if updateError}
								<div
									class="mb-4 p-3 bg-red-100 dark:bg-red-900/30 border border-red-400 text-red-700 dark:text-red-400 rounded"
								>
									{updateError}
								</div>
							{/if}

							<form
								onsubmit={(e) => {
									e.preventDefault();
									updateTransform();
								}}
								class="space-y-4"
							>
								<div>
									<label
										for="edit-title"
										class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
									>
										Title
									</label>
									<input
										id="edit-title"
										type="text"
										bind:value={editTitle}
										class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md
											   bg-white dark:bg-gray-700 text-gray-900 dark:text-white
											   focus:ring-2 focus:ring-blue-500 focus:border-transparent"
									/>
								</div>

								<div class="flex items-center">
									<input
										id="edit-enabled"
										type="checkbox"
										bind:checked={editEnabled}
										class="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 rounded"
									/>
									<label
										for="edit-enabled"
										class="ml-2 block text-sm text-gray-700 dark:text-gray-300"
									>
										Enabled (automatically process new files)
									</label>
								</div>

								<div class="flex gap-3">
									<button
										type="submit"
										disabled={updating}
										class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700
											   disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
									>
										{updating ? 'Saving...' : 'Save Changes'}
									</button>
									<button
										type="button"
										onclick={cancelEdit}
										class="px-4 py-2 bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-300
											   rounded-md hover:bg-gray-400 dark:hover:bg-gray-500 transition-colors"
									>
										Cancel
									</button>
								</div>
							</form>
						</div>
					{:else}
						<div class="p-6">
							<div class="flex justify-between items-start mb-2">
								<div class="flex-1">
									<div class="flex items-center gap-3 flex-wrap">
										<h3 class="text-xl font-semibold text-gray-900 dark:text-white">
											{transform.title}
										</h3>
										<span
											class="px-2 py-0.5 text-xs font-mono bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300 rounded"
											title="Transform ID"
										>
											#{transform.transform_id}
										</span>
										<span
											class="px-2 py-1 text-xs rounded-full bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400"
										>
											{getJobTypeLabel(transform.job_type)}
										</span>
										<span
											class={`px-2 py-1 text-xs rounded-full ${
												transform.is_enabled
													? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
													: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-400'
											}`}
										>
											{transform.is_enabled ? 'Enabled' : 'Disabled'}
										</span>
									</div>
									<div class="mt-2 space-y-1 text-sm text-gray-600 dark:text-gray-400">
										{#if transform.job_type === 'collection_to_dataset'}
											<p>
												<span class="font-medium">Source:</span>
												<a
													href={`#/collections/${transform.collection_id}`}
													class="text-blue-600 dark:text-blue-400 hover:underline"
												>
													{getCollectionTitle(transform.collection_id || 0)}
												</a>
											</p>
											<p>
												<span class="font-medium">Target:</span>
												<a
													href={`#/datasets/${transform.dataset_id}`}
													class="text-blue-600 dark:text-blue-400 hover:underline"
												>
													{getDatasetTitle(transform.dataset_id)}
												</a>
											</p>
											<p>
												<span class="font-medium">Chunk Size:</span>
												{transform.chunk_size} characters
											</p>
										{:else if transform.job_type === 'dataset_to_vector_storage'}
											<p>
												<span class="font-medium">Dataset:</span>
												<a
													href={`#/datasets/${transform.dataset_id}`}
													class="text-blue-600 dark:text-blue-400 hover:underline"
												>
													{getDatasetTitle(transform.dataset_id)}
												</a>
											</p>
											<p>
												<span class="font-medium">Embedders:</span>
												{#if transform.embedder_ids && transform.embedder_ids.length > 0}
													<span class="inline-flex flex-wrap gap-1">
														{#each transform.embedder_ids as embedderId (embedderId)}
															<span
																class="inline-block px-2 py-0.5 text-xs bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400 rounded"
																title="Embedder #{embedderId}"
															>
																{getEmbedderName(embedderId)}
															</span>
														{/each}
													</span>
												{:else}
													N/A
												{/if}
											</p>
										{/if}
										<p>
											<span class="font-medium">Created:</span>
											{formatDate(transform.created_at)}
										</p>

										{#if Object.keys(transform.job_config).length > 0}
											<div class="mb-4">
												<button
													onclick={() => {
														const key = `job_config_${transform.transform_id}`;
														expandedConfigs[key] = !expandedConfigs[key];
													}}
													class="flex items-center gap-2 font-semibold mb-1 text-gray-900 dark:text-white cursor-pointer hover:text-blue-600 dark:hover:text-blue-400"
												>
													<span
														class="inline-block transform transition-transform duration-200"
														style="transform: rotate({expandedConfigs[
															`job_config_${transform.transform_id}`
														]
															? '90deg'
															: '0deg'})">▶</span
													>
													Job Config
												</button>
												{#if expandedConfigs[`job_config_${transform.transform_id}`]}
													<pre
														class="bg-gray-100 dark:bg-gray-900 rounded p-2 overflow-x-auto text-xs mt-2">{JSON.stringify(
															transform.job_config,
															null,
															2
														)}</pre>
												{/if}
											</div>
										{/if}

										{#if transform.job_config?.chunking}
											<div class="mb-4">
												<button
													onclick={() => {
														const key = `chunking_${transform.transform_id}`;
														expandedConfigs[key] = !expandedConfigs[key];
													}}
													class="flex items-center gap-2 font-semibold mb-1 text-gray-900 dark:text-white cursor-pointer hover:text-blue-600 dark:hover:text-blue-400"
												>
													<span
														class="inline-block transform transition-transform duration-200"
														style="transform: rotate({expandedConfigs[
															`chunking_${transform.transform_id}`
														]
															? '90deg'
															: '0deg'})">▶</span
													>
													Chunking Config
												</button>
												{#if expandedConfigs[`chunking_${transform.transform_id}`]}
													<pre
														class="bg-gray-100 dark:bg-gray-900 rounded p-2 overflow-x-auto text-xs mt-2">
														{JSON.stringify(transform.job_config.chunking, null, 2)}
													</pre>
												{/if}
											</div>
										{/if}

										{#if transform.job_config?.extraction}
											<div class="mb-4">
												<button
													onclick={() => {
														const key = `extraction_${transform.transform_id}`;
														expandedConfigs[key] = !expandedConfigs[key];
													}}
													class="flex items-center gap-2 font-semibold mb-1 text-gray-900 dark:text-white cursor-pointer hover:text-blue-600 dark:hover:text-blue-400"
												>
													<span
														class="inline-block transform transition-transform duration-200"
														style="transform: rotate({expandedConfigs[
															`extraction_${transform.transform_id}`
														]
															? '90deg'
															: '0deg'})">▶</span
													>
													Extraction Config
												</button>
												{#if expandedConfigs[`extraction_${transform.transform_id}`]}
													<pre
														class="bg-gray-100 dark:bg-gray-900 rounded p-2 overflow-x-auto text-xs mt-2">
														{JSON.stringify(transform.job_config.extraction, null, 2)}
													</pre>
												{/if}
											</div>
										{/if}
									</div>
								</div>
								<div class="flex gap-2 shrink-0">
									{#if transform.job_type === 'collection_to_dataset'}
										<button
											onclick={() => triggerTransform(transform.transform_id)}
											disabled={triggering[transform.transform_id]}
											class="px-3 py-1 text-sm bg-green-600 text-white rounded hover:bg-green-700
												   disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
											title="Manually trigger a scan for new files"
										>
											{triggering[transform.transform_id] ? 'Triggering...' : 'Trigger Scan'}
										</button>
									{:else if transform.job_type === 'dataset_to_vector_storage'}
										<button
											onclick={() => triggerTransform(transform.transform_id)}
											disabled={triggering[transform.transform_id]}
											class="px-3 py-1 text-sm bg-green-600 text-white rounded hover:bg-green-700
												   disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
											title="Manually trigger embedding of all dataset items"
										>
											{triggering[transform.transform_id] ? 'Embedding...' : 'Trigger Embedding'}
										</button>
									{/if}
									<button
										onclick={() => toggleExpand(transform.transform_id)}
										class="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
									>
										{expandedTransformId === transform.transform_id
											? 'Hide Details'
											: 'Show Details'}
									</button>
									<button
										onclick={() => startEdit(transform)}
										class="px-3 py-1 text-sm bg-yellow-600 text-white rounded hover:bg-yellow-700 transition-colors"
									>
										Edit
									</button>
									<button
										onclick={() => requestDeleteTransform(transform)}
										disabled={deleting === transform.transform_id}
										class="px-3 py-1 text-sm bg-red-600 text-white rounded hover:bg-red-700
											   disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
									>
										{deleting === transform.transform_id ? 'Deleting...' : 'Delete'}
									</button>
								</div>
							</div>
						</div>

						{#if expandedTransformId === transform.transform_id}
							<div
								class="border-t border-gray-200 dark:border-gray-700 p-6 bg-gray-50 dark:bg-gray-900"
							>
								<div class="mb-6">
									<h4 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">
										Processing Statistics
									</h4>
									{#if loadingStats[transform.transform_id]}
										<div class="text-center py-4">
											<div
												class="inline-block animate-spin rounded-full h-6 w-6 border-4 border-gray-300 border-t-blue-600"
											></div>
										</div>
									{:else if transformStats[transform.transform_id]}
										{@const stats = transformStats[transform.transform_id]}
										{#if transform.job_type === 'dataset_to_vector_storage'}
											<div class="grid grid-cols-2 md:grid-cols-5 gap-3">
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-gray-900 dark:text-white">
														{transform.embedder_ids?.length || 0}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Embedders</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-purple-600 dark:text-purple-400">
														{formatCount(stats.total_items_processed)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Total Batches</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-green-600 dark:text-green-400">
														{formatCount(stats.successful_items)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Successful</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-red-600 dark:text-red-400">
														{formatCount(stats.failed_items)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Failed</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-blue-600 dark:text-blue-400">
														{formatCount(stats.total_chunks_embedded)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">
														Chunks Embedded
													</div>
												</div>
											</div>
											<div class="grid grid-cols-1 md:grid-cols-1 gap-3 mt-3">
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-orange-600 dark:text-orange-400">
														{formatCount(stats.total_chunks_failed)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Chunks Failed</div>
												</div>
											</div>
										{:else}
											<div class="grid grid-cols-2 md:grid-cols-5 gap-3">
												{#if stats.total_files_in_collection !== undefined}
													<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
														<div class="text-xl font-bold text-gray-900 dark:text-white">
															{formatCount(stats.total_files_in_collection)}
														</div>
														<div class="text-xs text-gray-600 dark:text-gray-400">
															Total Files in Collection
														</div>
													</div>
												{/if}
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-gray-900 dark:text-white">
														{formatCount(stats.total_files_processed)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">
														Files Processed
													</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-green-600 dark:text-green-400">
														{formatCount(stats.successful_files)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Successful</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-red-600 dark:text-red-400">
														{formatCount(stats.failed_files)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Failed</div>
												</div>
												<div class="bg-white dark:bg-gray-800 p-3 rounded-lg">
													<div class="text-xl font-bold text-blue-600 dark:text-blue-400">
														{formatCount(stats.total_items_created)}
													</div>
													<div class="text-xs text-gray-600 dark:text-gray-400">Items Created</div>
												</div>
											</div>
										{/if}
									{/if}
								</div>

								<div>
									<h4 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">
										Processed Files
									</h4>
									{#if processedFiles[transform.transform_id]?.length > 0}
										<div class="overflow-x-auto">
											<table
												class="min-w-full bg-white dark:bg-gray-800 rounded-lg overflow-hidden"
											>
												<thead class="bg-gray-100 dark:bg-gray-700">
													<tr>
														<th
															class="px-4 py-2 text-left text-xs font-medium text-gray-600 dark:text-gray-300 uppercase"
															>Batch ID</th
														>
														{#if transform.job_type === 'dataset_to_vector_storage'}
															<th
																class="px-4 py-2 text-left text-xs font-medium text-gray-600 dark:text-gray-300 uppercase"
																>Embedder</th
															>
														{/if}
														<th
															class="px-4 py-2 text-left text-xs font-medium text-gray-600 dark:text-gray-300 uppercase"
															>Status</th
														>
														<th
															class="px-4 py-2 text-left text-xs font-medium text-gray-600 dark:text-gray-300 uppercase"
															>Items</th
														>
														<th
															class="px-4 py-2 text-left text-xs font-medium text-gray-600 dark:text-gray-300 uppercase"
															>Duration</th
														>
														<th
															class="px-4 py-2 text-left text-xs font-medium text-gray-600 dark:text-gray-300 uppercase"
															>Processed</th
														>
													</tr>
												</thead>
												<tbody class="divide-y divide-gray-200 dark:divide-gray-700">
													{#each processedFiles[transform.transform_id] as file (file.file_key)}
														<tr class="hover:bg-gray-50 dark:hover:bg-gray-750">
															<td class="px-4 py-3 text-sm text-gray-900 dark:text-white">
																<span class="font-mono text-xs"
																	>{file.file_key.substring(
																		file.file_key.lastIndexOf('/') + 1,
																		file.file_key.lastIndexOf('.')
																	)}</span
																>
															</td>
															{#if transform.job_type === 'dataset_to_vector_storage'}
																<td class="px-4 py-3 text-sm text-gray-900 dark:text-white">
																	{#if getEmbedderFromFileKey(file.file_key, transform)}
																		<span class="font-medium"
																			>{getEmbedderFromFileKey(file.file_key, transform)
																				?.name}</span
																		>
																	{:else}
																		<span class="text-gray-500 dark:text-gray-400">-</span>
																	{/if}
																</td>
															{/if}
															<td class="px-4 py-3 text-sm">
																<span
																	class={`px-2 py-1 text-xs rounded-full ${
																		file.process_status === 'completed'
																			? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
																			: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'
																	}`}
																>
																	{file.process_status}
																</span>
																{#if file.process_error}
																	<div class="mt-1 text-xs text-red-600 dark:text-red-400">
																		{file.process_error}
																	</div>
																{/if}
															</td>
															<td class="px-4 py-3 text-sm text-gray-900 dark:text-white">
																{file.item_count}
															</td>
															<td class="px-4 py-3 text-sm text-gray-600 dark:text-gray-400">
																{#if file.processing_duration_ms !== null}
																	{(file.processing_duration_ms / 1000).toFixed(2)}s
																{:else}
																	<span class="text-gray-400 dark:text-gray-500">-</span>
																{/if}
															</td>
															<td class="px-4 py-3 text-sm text-gray-600 dark:text-gray-400">
																{formatDate(file.processed_at)}
															</td>
														</tr>
													{/each}
												</tbody>
											</table>
										</div>
									{:else}
										<p class="text-gray-600 dark:text-gray-400 text-sm">No files processed yet</p>
									{/if}
								</div>
							</div>
						{/if}
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<ConfirmDialog
	open={transformPendingDelete !== null}
	title="Delete transform?"
	message={transformPendingDelete
		? `Are you sure you want to delete "${transformPendingDelete.title}"? This will also delete all processing history.`
		: ''}
	confirmLabel="Delete transform"
	variant="danger"
	on:confirm={confirmDeleteTransform}
	on:cancel={cancelDeleteTransform}
/>
