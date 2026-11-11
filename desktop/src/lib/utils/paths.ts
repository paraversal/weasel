import { goto } from '$app/navigation';
import { resolve } from '$app/paths';

/**
 * Resolve a path that only exists at runtime (a nav href, a list URL with query params)
 * against the app base. Kit's typed `resolve` only accepts literal route ids, so the
 * cast has to live here instead of at every call site.
 */
export function resolvePath(path: string): string {
	return (resolve as unknown as (path: string) => string)(path);
}

/** `href` for links built at runtime, e.g. `href(`/contacts/${id}`)`. */
export const href = resolvePath;

type GotoOptions = Parameters<typeof goto>[1];

/** Navigate to a runtime path; the only place screens call `goto`. */
export function go(path: string, options?: GotoOptions): Promise<void> {
	// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved above
	return goto(resolvePath(path), options);
}
