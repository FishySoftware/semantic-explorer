import js from '@eslint/js';
import tseslint from '@typescript-eslint/eslint-plugin';
import tsparser from '@typescript-eslint/parser';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import svelteParser from 'svelte-eslint-parser';

export default [
	js.configs.recommended,
	{
		languageOptions: {
			globals: globals.browser,
		},
	},
	{
		files: ['**/*.ts'],
		languageOptions: {
			parser: tsparser,
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
			},
			globals: globals.browser,
		},
		plugins: {
			'@typescript-eslint': tseslint,
		},
		rules: {
			...tseslint.configs.recommended.rules,
			'@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
			'@typescript-eslint/no-explicit-any': 'warn',
		},
	},
	...svelte.configs['flat/recommended'],
	{
		files: ['**/*.svelte'],
		languageOptions: {
			parser: svelteParser,
			parserOptions: {
				parser: tsparser,
				projectService: true,
				extraFileExtensions: ['.svelte'],
			},
			globals: globals.browser,
		},
		rules: {
			'svelte/no-at-html-tags': 'warn',
			'svelte/valid-compile': 'error',
			'no-empty': 'warn',
			'no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
			'svelte/require-each-key': 'warn',
		},
	},
	{
		files: ['**/CollectionTransforms.svelte'],
		rules: {
			'svelte/no-at-html-tags': 'off',
		},
	},
	{
		files: ['**/VisualizationTransforms.svelte'],
		rules: {
			'svelte/no-at-html-tags': 'off',
		},
	},
	{
		files: ['**/VisualizationDetail.svelte'],
		rules: {
			'svelte/prefer-svelte-reactivity': 'off',
		},
	},
	prettier,
	{
		ignores: ['dist/', 'node_modules/', 'build/'],
	},
];
