import prettier from 'eslint-config-prettier';
import path from 'node:path';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, includeIgnoreFile } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';

const gitignorePath = path.resolve(import.meta.dirname, '.gitignore');

export default defineConfig(
	includeIgnoreFile(gitignorePath),
	js.configs.recommended,
	ts.configs.recommended,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		languageOptions: { globals: { ...globals.browser, ...globals.node } },
		rules: {
			// typescript-eslint strongly recommend that you do not use the no-undef lint rule on TypeScript projects.
			// see: https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
			'no-undef': 'off'
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser
			}
		}
	},
	{
		// Generated IPC bindings are tauri-specta output: never edited, never linted.
		ignores: ['src/lib/ipc/bindings.ts']
	},
	{
		// The IPC layer is the only place allowed to touch Tauri directly, and only the
		// toast renderer may mount sonner. Those exemptions share one rule object, so a
		// file listed in `ignores` is exempt from both bans.
		ignores: [
			'src/lib/ipc/**',
			'src/lib/stores/toasts.svelte.ts',
			'src/lib/shell/Toasts.svelte',
			'src/lib/components/ui/sonner/**'
		],
		rules: {
			'no-restricted-imports': [
				'error',
				{
					paths: [
						{
							name: '@tauri-apps/api/core',
							message: 'Import through $lib/ipc instead.'
						},
						{
							name: '@tauri-apps/api/event',
							message: 'Import through $lib/ipc instead.'
						},
						{
							name: 'svelte-sonner',
							message: 'Use $lib/stores/toasts.svelte.'
						}
					]
				}
			]
		}
	}
);
