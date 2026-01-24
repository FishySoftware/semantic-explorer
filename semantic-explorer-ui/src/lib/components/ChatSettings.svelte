<script lang="ts">
	interface Props {
		maxChunks: number;
		minSimilarityScore: number;
		temperature: number;
		maxTokens: number;
		onMaxChunksChange: (_value: number) => void;
		onMinSimilarityScoreChange: (_value: number) => void;
		onTemperatureChange: (_value: number) => void;
		onMaxTokensChange: (_value: number) => void;
	}

	let {
		maxChunks,
		minSimilarityScore,
		temperature,
		maxTokens,
		onMaxChunksChange,
		onMinSimilarityScoreChange,
		onTemperatureChange,
		onMaxTokensChange,
	}: Props = $props();
</script>

<div class="space-y-4">
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

	<p class="text-xs text-gray-500 dark:text-gray-400 mt-3">
		Control how many chunks are retrieved and minimum similarity score threshold.
	</p>

	<hr class="my-4 border-gray-200 dark:border-gray-600" />

	<p class="text-xs text-gray-500 dark:text-gray-400 mt-3">
		Control LLM response creativity and maximum length.
	</p>
</div>
