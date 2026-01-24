<script lang="ts">
	import type { ChatSession, EmbeddedDataset, LLM } from '../types/models';

	interface Props {
		sessions: ChatSession[];
		embeddedDatasets: EmbeddedDataset[];
		llms: LLM[];
		currentSession: ChatSession | null;
		onSelectSession: (_session: ChatSession) => void;
		onNewSession?: () => void;
	}

	let { sessions, embeddedDatasets, llms, currentSession, onSelectSession, onNewSession }: Props =
		$props();

	function getEmbeddedDatasetTitle(id: number): string {
		const dataset = embeddedDatasets.find((d) => d.embedded_dataset_id === id);
		return dataset ? `${dataset.title} (${dataset.embedder_name})` : `Dataset ${id}`;
	}

	function getLLMTitle(id: number): string {
		const llm = llms.find((l) => l.llm_id === id);
		return llm ? `${llm.name} (${llm.provider})` : `LLM ${id}`;
	}
</script>

<div
	class="w-72 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col overflow-hidden"
>
	<div class="p-6 border-b border-gray-200 dark:border-gray-700">
		<h1 class="text-2xl font-bold text-gray-900 dark:text-white">Chat</h1>
		<p class="text-sm text-gray-600 dark:text-gray-400 mt-1">RAG-powered conversations</p>
		{#if onNewSession}
			<button
				onclick={onNewSession}
				class="mt-3 w-full px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
			>
				New Session
			</button>
		{/if}
	</div>

	<div class="flex-1 overflow-y-auto">
		{#each sessions as session (session.session_id)}
			<button
				onclick={() => onSelectSession(session)}
				class="w-full text-left px-4 py-3 border-l-4 transition-colors {currentSession?.session_id ===
				session.session_id
					? 'border-blue-600 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400'
					: 'border-transparent hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300'}"
			>
				<div class="font-medium text-sm">{session.title || 'Untitled'}</div>
				<div class="text-xs text-gray-500 dark:text-gray-400 mt-1">
					{getEmbeddedDatasetTitle(session.embedded_dataset_id)} •
					{getLLMTitle(session.llm_id)}
				</div>
			</button>
		{/each}
	</div>
</div>
