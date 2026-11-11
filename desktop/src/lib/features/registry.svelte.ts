import { createQuery } from '@tanstack/svelte-query';

import { settingsQuery } from '$lib/ipc/queries';
import { detectPlatform } from '$lib/utils/shortcuts';

import { buildRegistry, manifests, type Registry } from './registry';

export interface RegistryState {
	readonly current: Registry;
}

/**
 * The registry the shell renders. Recomputes when settings arrive or feature flags change;
 * a plain value cannot be returned from a function and stay reactive, hence the accessor.
 */
export function createRegistry(): RegistryState {
	const settings = createQuery(() => settingsQuery());
	const current = $derived(
		buildRegistry(manifests, settings.data?.featureFlags ?? {}, {
			strict: import.meta.env.DEV,
			platform: detectPlatform()
		})
	);
	return {
		get current() {
			return current;
		}
	};
}
