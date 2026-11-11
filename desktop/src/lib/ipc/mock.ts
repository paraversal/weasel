import { emit } from '@tauri-apps/api/event';
import { commands } from './bindings';
import type { DomainEvent_Serialize, FeatureDescriptorDto } from './bindings';
import { createDomainHandlers } from './mock-domain';
import { MOCK_NOW, createMockData } from './mock-data';

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

const STORAGE_KEY = 'weasel.mock.settings';
const initialTheme = 'system' as const;

/** Persist across reloads so e2e can assert that a write survives; the real store uses SQLite. */
function readSettings(): SettingsState {
	if (typeof localStorage === 'undefined') return { theme: initialTheme, featureFlags: {} };
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return { theme: initialTheme, featureFlags: {} };
		const parsed = JSON.parse(raw) as Partial<SettingsState>;
		const theme = parsed.theme;
		return {
			theme: theme === 'light' || theme === 'dark' ? theme : initialTheme,
			featureFlags: parsed.featureFlags ?? {}
		};
	} catch {
		return { theme: initialTheme, featureFlags: {} };
	}
}

function writeSettings(settings: SettingsState): void {
	if (typeof localStorage === 'undefined') return;
	localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

const features: FeatureDescriptorDto[] = [
	{
		id: 'app',
		title: 'Application',
		commands: ['app_info', 'app_health', 'app_features', 'app_open_data_dir', 'app_backup_create'],
		events: []
	},
	{
		id: 'contacts',
		title: 'Contacts',
		commands: [
			'contacts_list',
			'contacts_get',
			'contacts_create',
			'contacts_update',
			'contacts_set_status',
			'contacts_delete',
			'contacts_stats'
		],
		events: ['contactsChanged']
	},
	{
		id: 'campaigns',
		title: 'Campaigns',
		commands: [
			'campaigns_list',
			'campaigns_get',
			'campaigns_create',
			'campaigns_update',
			'campaigns_set_status',
			'campaigns_delete',
			'campaigns_stats',
			'campaigns_audience',
			'campaigns_prepare_send'
		],
		events: ['campaignsChanged', 'sendBatchesChanged']
	},
	{
		id: 'send_batches',
		title: 'Send batches',
		commands: [
			'send_batches_list',
			'send_batches_get',
			'send_batches_recipients',
			'send_batches_recopy',
			'send_batches_set_note'
		],
		events: ['sendBatchesChanged']
	},
	{
		id: 'imports',
		title: 'Imports',
		commands: [
			'imports_pick_source',
			'imports_inspect',
			'imports_preview',
			'imports_apply',
			'imports_discard',
			'imports_list',
			'imports_get',
			'imports_plan_rows'
		],
		events: ['importsChanged', 'importProgress', 'importFinished']
	},
	{
		id: 'search',
		title: 'Search',
		commands: ['search_global'],
		events: []
	},
	{
		id: 'activity',
		title: 'Activity',
		commands: ['activity_recent', 'activity_list'],
		events: []
	},
	{
		id: 'settings',
		title: 'Settings',
		commands: ['settings_get_all', 'settings_update'],
		events: ['settingsChanged']
	}
	// @gen:features
];

export function mockHandlers(): Record<string, Handler> {
	let settings: SettingsState = readSettings();
	const data = createMockData();
	const emitEvent = (event: DomainEvent_Serialize) => {
		data.revision += 1;
		void emit('weasel-event', { revision: data.revision, event });
	};

	return {
		...createDomainHandlers(data, emitEvent),
		app_info: () => ({
			name: 'Weasel',
			version: '0.0.0-mock',
			tauriVersion: 'mock',
			apiVersion: '0.2.0',
			dbPath: '/mock/weasel.sqlite3',
			dataDir: '/mock'
		}),
		app_health: () => ({
			dbOk: true,
			schemaVersion: 2,
			contactCount: data.contacts.filter((contact) => contact.status === 'active').length,
			pendingImports: data.imports.filter((entry) => entry.status === 'draft').length,
			bootError: null
		}),
		app_features: () => features,
		app_open_data_dir: () => null,
		app_backup_create: () => ({
			path: `/mock/backups/weasel-${MOCK_NOW}.sqlite3`,
			bytes: 1_048_576,
			createdAt: MOCK_NOW
		}),
		settings_get_all: () => settings,
		settings_update: (args) => {
			const request = args.request as
				| { patch?: { theme?: SettingsState['theme']; featureFlags?: Record<string, boolean> } }
				| undefined;
			const patch = request?.patch ?? {};
			const keys = [
				...(patch.theme === undefined ? [] : ['theme']),
				...(patch.featureFlags === undefined ? [] : ['featureFlags'])
			];
			settings = {
				theme: patch.theme ?? settings.theme,
				featureFlags: { ...settings.featureFlags, ...patch.featureFlags }
			};
			writeSettings(settings);
			if (keys.length > 0) emitEvent({ type: 'settingsChanged', keys });
			return settings;
		},
		// @gen:handlers
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
	mockIPC(
		(command, args) => {
			const handler = handlers[command];
			if (!handler) throw new Error(`mock ipc: unexpected command ${command}`);
			return handler((args ?? {}) as Record<string, unknown>);
		},
		{ shouldMockEvents: true }
	);
	installed = true;
}
