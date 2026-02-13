<script lang="ts">
	import DOMPurify from 'dompurify';
	import { marked } from 'marked';
	import type { ChatMessage, RetrievedDocument } from '../types/models';

	interface Props {
		message: ChatMessage;
		sourceDatasetId?: number;
		onRegenerate?: (_messageId: number) => void;
	}

	let { message, sourceDatasetId, onRegenerate }: Props = $props();

	// State for collapsible chunks
	let chunksExpanded = $state(false);

	// Lazy load highlight.js
	let hljsModule: typeof import('highlight.js').default | null = null;
	let hljsLoaded = false;

	async function loadHighlightJS() {
		if (!hljsLoaded) {
			const [hljs] = await Promise.all([
				import('highlight.js'),
				import('highlight.js/styles/github-dark.css'),
			]);
			hljsModule = hljs.default;
			hljsLoaded = true;
		}
		return hljsModule!;
	}

	// Configure marked options
	marked.setOptions({
		breaks: true,
		gfm: true,
	});

	async function renderMarkdown(
		content: string,
		retrievedDocs?: RetrievedDocument[]
	): Promise<string> {
		let processedContent = content;

		if (retrievedDocs && retrievedDocs.length > 0) {
			// Get all unique document titles
			const titles = new Set(
				retrievedDocs.map((doc) => doc.item_title).filter((title): title is string => !!title)
			);

			// Apply blue styling to each title when it appears in content
			titles.forEach((title) => {
				// Escape special regex characters in title
				const escapedTitle = title.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
				// Replace title with styled version, but only in text (not in URLs or markdown links)
				const regex = new RegExp(`(?<!\\[)\\b${escapedTitle}\\b(?![\\]\\(])`, 'g');
				processedContent = processedContent.replace(
					regex,
					`**<span style="color: rgb(37 99 235)">${title}</span>**`
				);
			});
		}

		const html = marked.parse(processedContent) as string;
		// Sanitize HTML to prevent XSS attacks
		const sanitizedHtml = DOMPurify.sanitize(html, {
			ALLOWED_TAGS: [
				'h1',
				'h2',
				'h3',
				'h4',
				'h5',
				'h6',
				'p',
				'br',
				'hr',
				'ul',
				'ol',
				'li',
				'blockquote',
				'pre',
				'code',
				'span',
				'strong',
				'em',
				'a',
				'table',
				'head',
				'body',
				'tr',
				'th',
				'td',
				'img',
			],
			ALLOWED_ATTR: ['href', 'target', 'rel', 'class', 'src', 'alt', 'title'],
			ALLOW_DATA_ATTR: false,
		});
		// Apply syntax highlighting to code blocks after rendering
		const div = document.createElement('div');
		div.innerHTML = sanitizedHtml;

		const codeBlocks = div.querySelectorAll('pre code');
		if (codeBlocks.length > 0) {
			const hljs = await loadHighlightJS();
			codeBlocks.forEach((block) => {
				hljs.highlightElement(block as HTMLElement);
			});
		}

		return div.innerHTML;
	}

	function getDeduplicatedReferences(
		retrievedDocs?: RetrievedDocument[]
	): Array<{ title: string; count: number }> {
		if (!retrievedDocs) return [];

		const refMap: Record<string, number> = {};
		retrievedDocs.forEach((doc) => {
			const title = doc.item_title || 'Unknown';
			refMap[title] = (refMap[title] || 0) + 1;
		});
		return Object.entries(refMap)
			.map(([title, count]) => ({ title, count }))
			.sort((a, b) => b.count - a.count);
	}
</script>

<div class="flex {message.role === 'user' ? 'justify-end' : 'justify-start'}">
	<div
		class="max-w-3xl {message.role === 'user'
			? 'bg-blue-600 text-white'
			: 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white'} rounded-lg px-4 py-3"
	>
		<!-- Message Content -->
		{#if message.role === 'user'}
			<p class="text-sm whitespace-pre-wrap">{message.content}</p>
		{:else if message.status === 'incomplete' && !message.content}
			<div class="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
				<svg class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
					<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
					></circle>
					<path
						class="opacity-75"
						fill="currentColor"
						d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
					></path>
				</svg>
				<span>Generating response...</span>
			</div>
		{:else if message.content}
			<div class="prose prose-sm dark:prose-invert max-w-none">
				{#await renderMarkdown(message.content, message.retrieved_documents) then html}
					<!-- eslint-disable-next-line svelte/no-at-html-tags -- HTML is sanitized with DOMPurify -->
					{@html html}
				{/await}
			</div>
		{/if}

		<!-- References Section (inside message bubble, at bottom) -->
		{#if message.role === 'assistant' && message.retrieved_documents && message.retrieved_documents.length > 0 && message.status === 'complete'}
			<div class="mt-3 pt-3 border-t border-gray-300 dark:border-gray-600">
				<div class="text-xs font-semibold mb-2 text-gray-600 dark:text-gray-400">References</div>
				<div class="flex flex-wrap gap-2">
					{#each getDeduplicatedReferences(message.retrieved_documents) as ref (ref.title)}
						{#if sourceDatasetId}
							<a
								href="#/datasets/{sourceDatasetId}/details?search={encodeURIComponent(ref.title)}"
								class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs hover:bg-blue-200 dark:hover:bg-blue-900/50 transition-colors"
							>
								<span class="font-medium">{ref.title}</span>
								<span class="text-blue-500 dark:text-blue-400">×{ref.count}</span>
							</a>
						{:else}
							<span
								class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded text-xs"
							>
								<span class="font-medium">{ref.title}</span>
								<span class="text-blue-500 dark:text-blue-400">×{ref.count}</span>
							</span>
						{/if}
					{/each}
				</div>

				<!-- Collapsible Retrieved Chunks -->
				<div class="mt-3">
					<button
						onclick={() => (chunksExpanded = !chunksExpanded)}
						class="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 transition-colors"
					>
						<svg
							class="w-3 h-3 transition-transform {chunksExpanded ? 'rotate-90' : ''}"
							fill="none"
							stroke="currentColor"
							viewBox="0 0 24 24"
						>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M9 5l7 7-7 7"
							/>
						</svg>
						{chunksExpanded ? 'Hide' : 'Show'} Retrieved Chunks ({message.retrieved_documents
							.length})
					</button>

					{#if chunksExpanded}
						<div class="mt-2 space-y-2 max-h-64 overflow-y-auto">
							{#each message.retrieved_documents as doc, i (doc.document_id || i)}
								<div
									class="p-2 bg-gray-100 dark:bg-gray-600 rounded text-xs border-l-2 border-blue-400"
								>
									<div class="flex justify-between items-start mb-1">
										<span class="font-medium text-gray-700 dark:text-gray-300">
											{doc.item_title || `Chunk ${i + 1}`}
										</span>
										<span class="text-gray-500 dark:text-gray-400 ml-2">
											{(doc.similarity_score * 100).toFixed(1)}%
										</span>
									</div>
									<p class="text-gray-600 dark:text-gray-300 whitespace-pre-wrap line-clamp-4">
										{doc.text}
									</p>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}

		<!-- Regenerate Button (inside message bubble, at bottom) -->
		{#if message.role === 'assistant' && message.status === 'complete' && onRegenerate}
			<div class="mt-3 pt-2">
				<button
					onclick={() => onRegenerate(message.message_id)}
					class="text-xs px-3 py-1 bg-gray-300 dark:bg-gray-600 hover:bg-gray-400 dark:hover:bg-gray-500 text-gray-700 dark:text-gray-200 rounded transition-colors"
				>
					↻ Regenerate
				</button>
			</div>
		{/if}
	</div>
</div>
