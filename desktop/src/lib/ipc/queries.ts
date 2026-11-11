import { commands } from './bindings';
import { call } from './client';

/**
 * Every query key in the app. Events invalidate by key, so the two files that mention a
 * key are this one and `events.ts`.
 */
export const keys = {
	app: {
		info: ['app', 'info'] as const,
		health: ['app', 'health'] as const,
		features: ['app', 'features'] as const
	},
	settings: ['settings'] as const
};

export const appQueries = {
	info: () => ({ queryKey: keys.app.info, queryFn: () => call(commands.appInfo()) }),
	health: () => ({ queryKey: keys.app.health, queryFn: () => call(commands.appHealth()) }),
	features: () => ({
		queryKey: keys.app.features,
		queryFn: () => call(commands.appFeatures())
	})
};

export const settingsQuery = () => ({
	queryKey: keys.settings,
	queryFn: () => call(commands.settingsGetAll())
});
