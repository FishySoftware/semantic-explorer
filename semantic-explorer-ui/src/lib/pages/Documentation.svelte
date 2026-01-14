<script lang="ts">
	import { ArrowRightOutline, BookOpenSolid } from 'flowbite-svelte-icons';
</script>

<div class="p-6 max-w-6xl mx-auto">
	<div class="mb-8">
		<div class="flex items-center gap-3 mb-4">
			<BookOpenSolid class="w-8 h-8 text-blue-600 dark:text-blue-400" />
			<h1 class="text-3xl font-bold text-gray-900 dark:text-white">User Documentation</h1>
		</div>
		<p class="text-lg text-gray-600 dark:text-gray-400">
			Learn how to use Semantic Explorer to build, test, and compare embedding-based search systems,
			perform RAG-based chat, and visualize embedding spaces.
		</p>
	</div>

	<!-- Quick Start with Marketplace -->
	<section class="mb-8 rounded-lg border-l-4 border-green-500 bg-green-50 p-6 dark:bg-gray-800">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">
			Quick Start: Marketplace
		</h3>
		<p class="text-sm text-gray-700 dark:text-gray-300">
			Want to get started quickly? Visit the <strong>Marketplace</strong> to browse and grab public collections,
			datasets, embedders, and LLMs shared by other users. This is a great way to explore the platform
			without setting up everything from scratch.
		</p>
	</section>

	<!-- API-First Design -->
	<section class="mb-8 rounded-lg border-l-4 border-purple-500 bg-purple-50 p-6 dark:bg-gray-800">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">API-First Design</h3>
		<p class="text-sm text-gray-700 dark:text-gray-300 mb-3">
			While the UI provides a convenient interface, Semantic Explorer is <strong>API-first</strong>.
			You can programmatically push data directly via the REST API, bypassing the UI entirely:
		</p>
		<ul class="space-y-2 text-sm text-gray-700 dark:text-gray-300 list-inside list-disc">
			<li>
				<strong>Collections</strong> - Upload files directly to collections via multipart form-data
			</li>
			<li>
				<strong>Datasets</strong> - Push pre-processed chunks as JSON, bringing your own chunking strategy
			</li>
			<li>
				<strong>Embedders & LLMs</strong> - Configure providers programmatically for automation
			</li>
		</ul>
		<p class="text-sm text-gray-700 dark:text-gray-300 mt-3">
			This flexibility allows you to integrate Semantic Explorer into existing pipelines, use custom
			preprocessing, or build automated workflows. API documentation is available at <code
				class="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">/api/openapi.json</code
			>
		</p>
	</section>

	<!-- Workflow Overview -->
	<section class="mb-12">
		<h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-4">Workflow Overview</h2>
		<p class="text-gray-700 dark:text-gray-300 mb-6">
			Semantic Explorer provides a complete pipeline for evaluating embedding models, search
			quality, and RAG-based chat. Follow this typical workflow:
		</p>

		<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
			<!-- Step 1 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>1</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Collections</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Start by creating a collection and uploading your documents. Organize your content for
					processing.
				</p>
			</div>

			<!-- Step 2 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>2</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Embedders</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Configure your embedding providers (OpenAI, Cohere, etc.) that will be used to generate
					vector embeddings.
				</p>
			</div>

			<!-- Step 3 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>3</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Collection Transform</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Extract text from documents and generate chunks to populate a dataset.
				</p>
			</div>

			<!-- Step 4 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>4</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Datasets</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Review the generated dataset containing your processed text chunks ready for embedding.
				</p>
			</div>

			<!-- Step 5 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>5</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Dataset Transform</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Generate embeddings using configured embedders (creates one Embedded Dataset per
					embedder).
				</p>
			</div>

			<!-- Step 6 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>6</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Embedded Datasets</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Track embedding progress and view vector statistics of the generated embedded datasets.
				</p>
			</div>

			<!-- Step 7 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>7</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Visualization</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Create visualizations with UMAP dimensionality reduction and HDBSCAN clustering.
				</p>
			</div>

			<!-- Step 8 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>8</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Search</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Execute semantic searches and compare results across embedded datasets.
				</p>
			</div>

			<!-- Step 9 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>9</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">LLMs</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Configure your language model providers (OpenAI, Anthropic, Cohere, etc.) for RAG-based
					chat.
				</p>
			</div>

			<!-- Step 10 -->
			<div
				class="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
			>
				<div class="flex items-center gap-2 mb-3">
					<span
						class="flex h-8 w-8 items-center justify-center rounded-full bg-blue-100 text-sm font-bold text-blue-600 dark:bg-blue-900 dark:text-blue-300"
						>10</span
					>
					<h3 class="text-lg font-semibold text-gray-900 dark:text-white">Chat</h3>
				</div>
				<p class="text-sm text-gray-600 dark:text-gray-400">
					Interact with your embedded datasets using RAG-based chat powered by your configured LLMs.
				</p>
			</div>
		</div>
	</section>

	<!-- Detailed Component Descriptions -->
	<section class="space-y-8">
		<h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-6">Component Details</h2>

		<!-- Collections -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-blue-100 px-2 py-1 text-sm font-medium text-blue-800 dark:bg-blue-900 dark:text-blue-300"
					>Collections</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Organize collections of documents of interest. You can add as many files as you want, up to
				100MB per file with a total upload limit of 1GB per request. Most common content types are
				supported including Office documents (Word, Excel, PowerPoint), HTML, XML, and raw text
				files.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900 mb-3">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Create and manage document collections</li>
					<li>Upload files via web interface</li>
					<li>S3 integration for bulk imports</li>
					<li>Support for multiple document formats</li>
					<li>Individual file size limit: 100MB (configurable), 1GB per upload request</li>
					<li>Organize related documents together for processing</li>
				</ul>
			</div>
			<div
				class="rounded bg-purple-50 dark:bg-purple-900/20 p-3 border border-purple-200 dark:border-purple-800"
			>
				<p class="text-xs text-gray-700 dark:text-gray-300">
					<strong>💡 API Usage:</strong> Push files directly to collections via
					<code class="px-1 py-0.5 bg-white dark:bg-gray-800 rounded text-xs"
						>POST /api/collections/&#123;id&#125;/upload</code
					>
					with multipart form-data. Great for automation and CI/CD pipelines.
				</p>
			</div>
		</div>

		<!-- Datasets -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-green-100 px-2 py-1 text-sm font-medium text-green-800 dark:bg-green-900 dark:text-green-300"
					>Datasets</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Contains processed texts as JSON with name and chunks, to be used for embedding transforms.
				Datasets can be generated automatically from collections using Collection Transforms, or you
				can push pre-processed chunks directly via the API. This flexibility allows you to bring
				your own chunking strategies or integrate with external processing pipelines.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900 mb-3">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>View processed text chunks with metadata</li>
					<li>JSON format with structured data</li>
					<li>Generated from Collection Transforms automatically</li>
					<li>Can be embedded using multiple embedders for comparison</li>
					<li>Browse and search dataset items</li>
				</ul>
			</div>
			<div
				class="rounded bg-purple-50 dark:bg-purple-900/20 p-3 border border-purple-200 dark:border-purple-800"
			>
				<p class="text-xs text-gray-700 dark:text-gray-300">
					<strong>💡 API Usage:</strong> Skip Collection Transforms entirely! Push pre-processed
					chunks directly via
					<code class="px-1 py-0.5 bg-white dark:bg-gray-800 rounded text-xs"
						>POST /api/datasets/&#123;id&#125;/items</code
					>
					as JSON. Bring your own chunking strategy from external tools like LangChain, LlamaIndex, or
					custom pipelines.
				</p>
			</div>
		</div>

		<!-- Embedders -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-purple-100 px-2 py-1 text-sm font-medium text-purple-800 dark:bg-purple-900 dark:text-purple-300"
					>Embedders</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Provides embedding provider instances that are user-managed. Define OpenAI or Cohere
				compatible embedders that can be used on dataset transforms to produce vector embeddings for
				semantic search.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Configure OpenAI-compatible embedding APIs</li>
					<li>Configure Cohere embedding APIs</li>
					<li>Test connection and validate credentials</li>
					<li>Store API keys securely</li>
					<li>Reuse embedder configurations across multiple datasets</li>
				</ul>
			</div>
		</div>

		<!-- Transforms -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-orange-100 px-2 py-1 text-sm font-medium text-orange-800 dark:bg-orange-900 dark:text-orange-300"
					>Transforms</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Provides configurable pipelines for each stage of data processing. Collection transforms
				extract text from documents and generate chunks. Dataset transforms generate embeddings
				using configured embedders, creating one Embedded Dataset per embedder for comparison.
				Visualization transforms reduce embeddings with UMAP and identify topics with HDBSCAN.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Transform Types:</h4>
				<div class="space-y-3 text-sm">
					<div>
						<div class="font-semibold text-gray-900 dark:text-white">Collection Transforms</div>
						<p class="text-gray-600 dark:text-gray-400">
							Extract text from documents and generate chunks to create datasets
						</p>
					</div>
					<div>
						<div class="font-semibold text-gray-900 dark:text-white">Dataset Transforms</div>
						<p class="text-gray-600 dark:text-gray-400">
							Generate embeddings for dataset chunks using configured embedders (creates one
							Embedded Dataset per embedder)
						</p>
					</div>
					<div>
						<div class="font-semibold text-gray-900 dark:text-white">Visualization Transforms</div>
						<p class="text-gray-600 dark:text-gray-400">
							Reduce embeddings using UMAP and identify topic clusters with HDBSCAN (requires GPU)
						</p>
					</div>
				</div>
				<h4 class="font-semibold text-gray-900 dark:text-white mt-4 mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Monitor transform job status and progress</li>
					<li>View processing history and statistics</li>
					<li>Trigger transforms manually or via API</li>
					<li>Configure chunking strategies and parameters</li>
					<li>Support for multiple embedders per dataset</li>
					<li>GPU-accelerated visualization generation (CUDA 12.4)</li>
				</ul>
			</div>
		</div>

		<!-- Embedded Datasets -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-amber-100 px-2 py-1 text-sm font-medium text-amber-800 dark:bg-amber-900 dark:text-amber-300"
					>Embedded Datasets</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Result entities created automatically when you create a Dataset Transform with multiple
				embedders. Each Embedded Dataset represents embeddings from one embedder model, allowing you
				to compare results side-by-side. Track embedding progress, view vector statistics, and
				trigger visualizations for each model independently.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Automatically created when Dataset Transform runs</li>
					<li>One per configured embedder model</li>
					<li>Track embedding generation progress per model</li>
					<li>View vector statistics and embedding metadata</li>
					<li>Search across each embedded dataset independently</li>
					<li>Trigger visualization transforms for visualization</li>
					<li>Compare embedding quality across models</li>
				</ul>
			</div>
		</div>

		<!-- Search -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-red-100 px-2 py-1 text-sm font-medium text-red-800 dark:bg-red-900 dark:text-red-300"
					>Search</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Provides the ability to execute semantic searches against multiple embedded datasets for
				comparison purposes. Test and evaluate different embedding models side-by-side to determine
				which performs best for your use case.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Search across multiple embedded datasets simultaneously</li>
					<li>Compare results from different embedding models</li>
					<li>Configure search parameters (top K, filters)</li>
					<li>Apply metadata filters to refine results</li>
					<li>View similarity scores and ranking differences</li>
				</ul>
			</div>
		</div>

		<!-- Visualizations -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-indigo-100 px-2 py-1 text-sm font-medium text-indigo-800 dark:bg-indigo-900 dark:text-indigo-300"
					>Visualizations</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				View and interact with 2D/3D visualizations of your embedding spaces. After running
				Visualization Transforms, explore how your documents cluster together in visual space.
				Compare different embedding models side-by-side to understand how each organizes your data
				and identify distinct topic clusters.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Interactive 2D scatter plots of embedding spaces</li>
					<li>Color-coded clusters identified by HDBSCAN</li>
					<li>Hover to view document text and metadata</li>
					<li>Compare visualizations across different embedding models</li>
					<li>Explore topic distributions and cluster quality</li>
					<li>Pan, zoom, and interact with visualization data</li>
				</ul>
			</div>
		</div>

		<!-- LLMs -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-cyan-100 px-2 py-1 text-sm font-medium text-cyan-800 dark:bg-cyan-900 dark:text-cyan-300"
					>LLMs</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Configure and manage Large Language Model (LLM) providers for RAG-based chat functionality.
				Similar to embedders, LLMs are user-managed configurations that connect to chat completion
				APIs from providers like OpenAI, Anthropic, Cohere, and other OpenAI-compatible services.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Configure OpenAI-compatible chat APIs (GPT-4, GPT-3.5, etc.)</li>
					<li>Support for Anthropic Claude, Cohere, and other providers</li>
					<li>Test connection and validate credentials</li>
					<li>Store API keys securely with encryption</li>
					<li>Configure model parameters (temperature, max tokens, etc.)</li>
					<li>Make LLMs public to share with other users via marketplace</li>
				</ul>
			</div>
		</div>

		<!-- Chat -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-teal-100 px-2 py-1 text-sm font-medium text-teal-800 dark:bg-teal-900 dark:text-teal-300"
					>Chat (RAG)</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Interact with your embedded datasets using Retrieval-Augmented Generation (RAG). Create chat
				sessions that combine semantic search with LLM-powered responses. The system automatically
				retrieves relevant chunks from your embedded datasets and uses them to provide contextual,
				grounded answers to your questions.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>RAG-based chat with automatic context retrieval</li>
					<li>Create and manage multiple chat sessions</li>
					<li>Select from configured embedded datasets and LLMs</li>
					<li>View retrieved documents and similarity scores</li>
					<li>Streaming responses with markdown rendering</li>
					<li>Code syntax highlighting for technical content</li>
					<li>Session history and persistence</li>
				</ul>
			</div>
		</div>

		<!-- Marketplace -->
		<div
			class="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800"
		>
			<h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
				<span
					class="rounded bg-pink-100 px-2 py-1 text-sm font-medium text-pink-800 dark:bg-pink-900 dark:text-pink-300"
					>Marketplace</span
				>
			</h3>
			<p class="text-gray-700 dark:text-gray-300 mb-4">
				Browse and grab public resources shared by other users. The marketplace allows you to
				discover collections, datasets, embedders, and LLMs that others have made public. This is a
				great way to get started quickly without setting up everything from scratch, or to explore
				example configurations and datasets.
			</p>
			<div class="rounded bg-gray-50 p-4 dark:bg-gray-900">
				<h4 class="font-semibold text-gray-900 dark:text-white mb-2">Key Features:</h4>
				<ul class="list-inside list-disc space-y-1 text-sm text-gray-600 dark:text-gray-400">
					<li>Browse public collections from other users</li>
					<li>Discover pre-configured embedders and LLMs</li>
					<li>Grab (copy) public datasets to your account</li>
					<li>Search and filter marketplace resources</li>
					<li>View resource details before grabbing</li>
					<li>Share your own resources by marking them as public</li>
				</ul>
			</div>
		</div>
	</section>

	<!-- Tips Section -->
	<section class="mt-12 rounded-lg border-l-4 border-blue-500 bg-blue-50 p-6 dark:bg-gray-800">
		<h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">Tips for Best Results</h3>
		<ul class="space-y-2 text-sm text-gray-700 dark:text-gray-300">
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span>Check the Marketplace first - grab public resources to get started quickly</span>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span>Start with a small collection to test your pipeline before scaling up</span>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span
					>Configure multiple embedders to compare performance across different models using
					Embedded Datasets</span
				>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span
					>Use meaningful names for collections, datasets, embedders, and LLMs to track experiments</span
				>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span>Monitor transform job status to track processing progress</span>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span>Experiment with different chunking strategies to optimize search quality</span>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span
					>Generate visualizations to visually compare how different embedding models organize your
					data</span
				>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span
					>Use Chat to interact with your embedded datasets - great for exploring and validating
					search quality</span
				>
			</li>
			<li class="flex items-start gap-2">
				<ArrowRightOutline class="w-4 h-4 mt-0.5 shrink-0 text-blue-600 dark:text-blue-400" />
				<span
					>Make your best resources public to share them with the community via the Marketplace</span
				>
			</li>
		</ul>
	</section>
</div>
