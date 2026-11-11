import { commands } from './bindings';

/**
 * Browser-only stand-in for the Rust backend, used by `pnpm dev:web` and Playwright.
 * Installed from the root layout's `load` when `VITE_IPC_MOCK=1`; the dynamic import keeps
 * this module (and `@tauri-apps/api/mocks`) out of production bundles.
 */
type Handler = (args: Record<string, unknown>) => unknown;

type SettingsState = {
	theme: 'system' | 'light' | 'dark';
	featureFlags: Record<string, boolean>;
};

const initialTheme = 'system' as const;

const features = [
	{
		id: 'app',
		title: 'Application',
		commands: ['app_info', 'app_health', 'app_features', 'app_open_data_dir'],
		events: []
	},
	{
		id: 'settings',
		title: 'Settings',
		commands: ['settings_get_all', 'settings_update'],
		events: ['settingsChanged']
	}
];

export function mockHandlers(): Record<string, Handler> {
	let settings: SettingsState = { theme: initialTheme, featureFlags: {} };

	return {
		app_info: () => ({
			name: 'Weasel',
			version: '0.0.0-mock',
			tauriVersion: 'mock',
			apiVersion: '0.1.0',
			dbPath: '/mock/weasel.sqlite3',
			dataDir: '/mock'
		}),
		app_health: () => ({ dbOk: true, schemaVersion: 1, bootError: null }),
		app_features: () => features,
		app_open_data_dir: () => null,
		settings_get_all: () => settings,
		settings_update: (args) => {
			const request = args.request as
				| { patch?: { theme?: SettingsState['theme']; featureFlags?: Record<string, boolean> } }
				| undefined;
			const patch = request?.patch ?? {};
			settings = {
				theme: patch.theme ?? settings.theme,
				featureFlags: { ...settings.featureFlags, ...patch.featureFlags }
			};
			return settings;
		},
		// The event plugin is invoked from `listen`; no payloads are delivered in mock mode.
		'plugin:event|listen': () => 1,
		'plugin:event|unlisten': () => null,
		'plugin:event|emit': () => null,
		'plugin:event|emit_to': () => null,
		// `lib/ipc/log.ts` still writes to the plugin so errors survive in dev:web.
		'plugin:log|log': () => null
	};
}

/** `appInfo` → `app_info`, matching the Rust command names. */
export function toCommandName(bindingKey: string): string {
	return bindingKey.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/** Fails loudly at startup when a command has no mock, instead of at first click. */
export function missingHandlers(handlers: Record<string, Handler>): string[] {
	return Object.keys(commands)
		.map(toCommandName)
		.filter((name) => !(name in handlers));
}

let installed = false;

export async function installMockIpc(): Promise<void> {
	if (installed) return;
	const handlers = mockHandlers();
	const missing = missingHandlers(handlers);
	if (missing.length > 0) {
		throw new Error(`mock ipc: no handler for ${missing.join(', ')}`);
	}

	const { mockIPC } = await import('@tauri-apps/api/mocks');
	mockIPC((command, args) => {
		const handler = handlers[command];
		if (!handler) throw new Error(`mock ipc: unexpected command ${command}`);
		return handler((args ?? {}) as Record<string, unknown>);
	});
	installed = true;
}
