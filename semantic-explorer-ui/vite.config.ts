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
	css: {
		transformer: 'postcss', // Use PostCSS instead of lightningcss for Svelte compatibility
	},
	build: {
		// Increase chunk size warning limit for large visualization and syntax highlighting libraries
		// These are lazy-loaded so the large size is acceptable
		chunkSizeWarningLimit: 1024,
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
					// Split highlight.js into a separate chunk
					if (id.includes('highlight.js')) {
						return 'highlight';
					}
					// Split marked into a separate chunk
					if (id.includes('marked')) {
						return 'marked';
					}
					// Split node_modules into vendor chunk (excluding the above)
					if (id.includes('node_modules')) {
						return 'vendor';
					}
				},
			},
		},
	},
	plugins: [
		tailwindcss({
			// Disable lightningcss minification and let Vite handle it with proper Svelte support
			optimize: {
				minify: false,
			},
		}),
		svelte(),
	],
});
