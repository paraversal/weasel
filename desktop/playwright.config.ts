import { defineConfig } from '@playwright/test';

// The e2e suite drives the built SPA in Chromium (`VITE_IPC_MOCK=1` is injected by the
// specs that need a fake backend), not the Tauri window: the webview cannot be scripted
// from Playwright on CI.
export default defineConfig({
	testDir: 'tests/e2e',
	testMatch: '**/*.e2e.ts',
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	reporter: process.env.CI ? 'github' : 'list',
	use: {
		baseURL: 'http://localhost:4173',
		trace: 'on-first-retry'
	},
	webServer: {
		command: 'pnpm build && pnpm preview --port 4173 --strictPort',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI,
		timeout: 120_000
	}
});
