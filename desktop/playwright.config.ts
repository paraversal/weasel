import { defineConfig } from '@playwright/test';

// The e2e suite drives the built SPA in Chromium against the mock backend
// (`VITE_IPC_MOCK=1` in the webServer command), not the Tauri window: the webview cannot
// be scripted from Playwright.
export default defineConfig({
	testDir: 'tests/e2e',
	testMatch: '**/*.spec.ts',
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	reporter: process.env.CI ? 'github' : 'list',
	use: {
		baseURL: 'http://localhost:4173',
		trace: 'on-first-retry'
	},
	webServer: {
		command: 'VITE_IPC_MOCK=1 pnpm build && VITE_IPC_MOCK=1 pnpm preview --port 4173 --strictPort',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI,
		timeout: 120_000
	}
});
