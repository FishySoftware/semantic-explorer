<script lang="ts">
	import type { ChatMessage, RetrievedDocument } from '../types/models';
	import { marked } from 'marked';
	import DOMPurify from 'dompurify';

	interface Props {
		message: ChatMessage;
		onRegenerate?: (_messageId: number) => void;
	}

	let { message, onRegenerate }: Props = $props();

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

<div class="flex {message.role === 'user' ? 'justify-end' : 'justify-start'} gap-4">
	<div
		class="max-w-2xl {message.role === 'user'
			? 'bg-blue-600 text-white'
			: 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white'} rounded-lg px-4 py-2"
	>
		{#if message.role === 'user'}
			<p class="text-sm">{message.content}</p>
		{:else if message.content}
			<div class="prose prose-sm dark:prose-invert max-w-none break-all">
				{#await renderMarkdown(message.content, message.retrieved_documents) then html}
					<!-- eslint-disable-next-line svelte/no-at-html-tags -- HTML is sanitized with DOMPurify -->
					{@html html}
				{/await}
			</div>
		{/if}
	</div>

	{#if message.role === 'assistant' && message.retrieved_documents && message.retrieved_documents.length > 0 && message.status === 'complete'}
		<div class="mt-3 pt-3 border-t border-gray-400 dark:border-gray-500">
			<div class="text-xs font-semibold mb-2 text-gray-600 dark:text-gray-300">References</div>
			<div class="space-y-1.5">
				{#each getDeduplicatedReferences(message.retrieved_documents) as ref (ref.title)}
					<div class="flex items-center gap-2">
						<a
							href="#/datasets/{message.embedded_dataset_id}/details?search={encodeURIComponent(
								ref.title
							)}"
							class="text-blue-600 dark:text-blue-400 hover:underline font-semibold text-xs"
						>
							{ref.title}
						</a>
						<span class="text-gray-500 dark:text-gray-400 text-xs">× {ref.count}</span>
					</div>
				{/each}
			</div>
		</div>
	{/if}

	{#if message.role === 'assistant' && message.status === 'complete' && onRegenerate}
		<div class="mt-3 pt-3 border-t border-current">
			<button
				onclick={() => onRegenerate(message.message_id)}
				class="text-xs px-3 py-1 bg-yellow-500 hover:bg-yellow-600 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
			>
				⚡ Regenerate
			</button>
		</div>
	{/if}
</div>
