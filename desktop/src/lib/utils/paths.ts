import { resolve } from '$app/paths';

/**
 * Resolve a path that only exists at runtime (a manifest nav href or palette target)
 * against the app base. Kit's typed `resolve` only accepts literal route ids, so the
 * cast has to live here instead of at every call site.
 */
export function resolvePath(path: string): string {
	return (resolve as unknown as (path: string) => string)(path);
}
