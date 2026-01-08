import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';
import { defineConfig } from 'vite';

export default defineConfig({
	base: '/ui/',
	resolve: {
		alias: {
			$lib: path.resolve('./src/lib'),
		},
	},
	build: {
		// Increase chunk size warning limit for large visualization libraries
		chunkSizeWarningLimit: 750,
		rollupOptions: {
			output: {
				assetFileNames: 'assets/[name]-[hash][extname]',
				chunkFileNames: 'assets/[name]-[hash].js',
				entryFileNames: 'assets/[name]-[hash].js',
				manualChunks(id: string) {
					// Split deck.gl and its dependencies into a separate chunk
					if (id.includes('@deck.gl') || id.includes('deck.gl')) {
						return 'deck-gl';
					}
					// Split loaders.gl into a separate chunk
					if (id.includes('@loaders.gl')) {
						return 'loaders-gl';
					}
					// Split flowbite into a separate chunk
					if (id.includes('flowbite')) {
						return 'flowbite';
					}
				},
			},
		},
	},
	plugins: [tailwindcss(), svelte()],
});
