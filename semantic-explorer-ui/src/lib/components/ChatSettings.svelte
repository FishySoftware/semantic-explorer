<script lang="ts">
	const DEFAULT_SYSTEM_PROMPT = `You are a helpful assistant that answers questions based on the provided context.

Context:
{{chunks}}

When answering, always cite the specific chunk number (e.g., "According to Chunk 1" or "As mentioned in Chunk 2 and Chunk 3") to reference where your information comes from. If the context doesn't contain relevant information to answer the question, say so explicitly.`;

	interface Props {
		maxChunks: number;
		minSimilarityScore: number;
		temperature: number;
		maxTokens: number;
		systemPrompt: string;
		onMaxChunksChange: (_value: number) => void;
		onMinSimilarityScoreChange: (_value: number) => void;
		onTemperatureChange: (_value: number) => void;
		onMaxTokensChange: (_value: number) => void;
		onSystemPromptChange: (_value: string) => void;
	}

	let {
		maxChunks,
		minSimilarityScore,
		temperature,
		maxTokens,
		systemPrompt,
		onMaxChunksChange,
		onMinSimilarityScoreChange,
		onTemperatureChange,
		onMaxTokensChange,
		onSystemPromptChange,
	}: Props = $props();

	let showSystemPrompt = $state(false);

	// Derive the effective prompt (what will actually be used)
	let effectivePrompt = $derived(systemPrompt || DEFAULT_SYSTEM_PROMPT);

	function handleSystemPromptInput(e: Event) {
		const target = e.currentTarget as HTMLTextAreaElement;
		onSystemPromptChange(target.value);
	}
</script>

<div class="space-y-4">
	<!-- RAG Settings Section -->
	<div>
		<h4 class="text-sm font-semibold text-gray-800 dark:text-gray-200 mb-2">RAG Settings</h4>
		<p class="text-xs text-gray-500 dark:text-gray-400 mb-3">
			Control how many chunks are retrieved and minimum similarity score threshold.
		</p>
		<div class="grid grid-cols-2 gap-4">
			<div>
				<label
					for="max-chunks"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
				>
					Max Chunks: <span class="font-bold">{maxChunks}</span>
				</label>
				<input
					id="max-chunks"
					type="range"
					min="1"
					max="100"
					step="1"
					bind:value={maxChunks}
					oninput={(e) => onMaxChunksChange(Number(e.currentTarget.value))}
					class="slider w-full"
				/>
				<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
					<span>1</span>
					<span>100</span>
				</div>
			</div>

			<div>
				<label
					for="min-score"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
				>
					Min Similarity: <span class="font-bold">{minSimilarityScore.toFixed(2)}</span>
				</label>
				<input
					id="min-score"
					type="range"
					min="0"
					max="1"
					step="0.05"
					bind:value={minSimilarityScore}
					oninput={(e) => onMinSimilarityScoreChange(Number(e.currentTarget.value))}
					class="slider w-full"
				/>
				<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
					<span>0.00</span>
					<span>1.00</span>
				</div>
			</div>
		</div>
	</div>

	<hr class="border-gray-200 dark:border-gray-600" />

	<!-- LLM Settings Section -->
	<div>
		<h4 class="text-sm font-semibold text-gray-800 dark:text-gray-200 mb-2">LLM Settings</h4>
		<p class="text-xs text-gray-500 dark:text-gray-400 mb-3">
			Control LLM response creativity and maximum length.
		</p>
		<div class="grid grid-cols-2 gap-4">
			<div>
				<label
					for="temperature"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
				>
					Temperature: <span class="font-bold">{temperature.toFixed(2)}</span>
				</label>
				<input
					id="temperature"
					type="range"
					min="0"
					max="2"
					step="0.1"
					bind:value={temperature}
					oninput={(e) => onTemperatureChange(Number(e.currentTarget.value))}
					class="slider w-full"
				/>
				<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
					<span>0.0 (Precise)</span>
					<span>2.0 (Creative)</span>
				</div>
			</div>

			<div>
				<label
					for="max-tokens"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
				>
					Max Tokens: <span class="font-bold">{maxTokens}</span>
				</label>
				<input
					id="max-tokens"
					type="number"
					min="100"
					max="8000"
					step="100"
					bind:value={maxTokens}
					oninput={(e) => onMaxTokensChange(Number(e.currentTarget.value))}
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
				/>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">100-8000 tokens</div>
			</div>
		</div>
	</div>

	<hr class="border-gray-200 dark:border-gray-600" />

	<!-- System Prompt Section -->
	<div>
		<button
			type="button"
			class="flex items-center gap-2 text-sm font-semibold text-gray-800 dark:text-gray-200 mb-2 hover:text-gray-600 dark:hover:text-gray-400 transition-colors"
			onclick={() => (showSystemPrompt = !showSystemPrompt)}
		>
			<svg
				class="w-4 h-4 transition-transform {showSystemPrompt ? 'rotate-90' : ''}"
				fill="none"
				stroke="currentColor"
				viewBox="0 0 24 24"
			>
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
			</svg>
			System Prompt
			{#if systemPrompt}
				<span class="text-xs text-blue-500 dark:text-blue-400">(custom)</span>
			{/if}
		</button>

		{#if showSystemPrompt}
			<div class="space-y-2">
				<p class="text-xs text-gray-500 dark:text-gray-400">
					Customize the system prompt sent to the LLM. Use <code
						class="bg-gray-100 dark:bg-gray-700 px-1 rounded">{`{{chunks}}`}</code
					> as a placeholder for the retrieved document chunks. Leave empty to use the default prompt.
				</p>
				<textarea
					id="system-prompt"
					rows="6"
					value={systemPrompt}
					oninput={handleSystemPromptInput}
					placeholder={DEFAULT_SYSTEM_PROMPT}
					class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white text-sm font-mono resize-y"
				></textarea>
				<div class="flex justify-between items-center">
					<span class="text-xs text-gray-500 dark:text-gray-400">
						{effectivePrompt.length} characters {#if !systemPrompt}<span class="italic"
								>(default)</span
							>{/if}
					</span>
					<div class="flex gap-2">
						{#if !systemPrompt}
							<button
								type="button"
								class="text-xs text-blue-500 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300"
								onclick={() => onSystemPromptChange(DEFAULT_SYSTEM_PROMPT)}
							>
								Load default to edit
							</button>
						{:else}
							<button
								type="button"
								class="text-xs text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300"
								onclick={() => onSystemPromptChange('')}
							>
								Reset to default
							</button>
						{/if}
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>
