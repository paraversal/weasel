import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

// Tauri recommends a fixed dev port and keeps Rust build output out of the watcher.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			adapter: adapter({ fallback: 'index.html' }),
			// Also emitted into the generated tsconfig so tools (shadcn-svelte,
			// svelte-check) resolve the same aliases Vite uses.
			alias: { $lib: 'src/lib' }
		})
	],
	clearScreen: false,
	envPrefix: ['VITE_', 'TAURI_ENV_'],
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
		watch: { ignored: ['**/src-tauri/**'] }
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'unit',
					environment: 'node',
					include: ['src/**/*.test.ts', 'tests/**/*.test.ts'],
					exclude: ['**/*.svelte.test.ts']
				}
			},
			{
				extends: './vite.config.ts',
				test: {
					name: 'client',
					environment: 'happy-dom',
					include: ['src/**/*.svelte.test.ts', 'tests/**/*.svelte.test.ts'],
					setupFiles: ['./tests/setup-client.ts']
				}
			}
		]
	}
});
