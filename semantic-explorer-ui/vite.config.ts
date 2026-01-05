import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
	base: '/ui/',
	resolve: {
		alias: {
			$lib: path.resolve('./src/lib'),
		},
	},
	build: {
		rollupOptions: {
			output: {
				assetFileNames: 'assets/[name]-[hash][extname]',
				chunkFileNames: 'assets/[name]-[hash].js',
				entryFileNames: 'assets/[name]-[hash].js',
			},
		},
	},
	plugins: [tailwindcss(), svelte()],
});
