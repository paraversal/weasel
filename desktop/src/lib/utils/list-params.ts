/** Kit's `page.url` is a read-only URL; accept anything with a path and readable query. */
export interface UrlLike {
	readonly pathname: string;
	readonly searchParams: { get(key: string): string | null; toString(): string };
}

/** Every paginated list shows this many rows. */
export const PAGE_SIZE = 50;

/** A trimmed query param, or `undefined` when missing or blank. */
export function param(url: UrlLike, key: string): string | undefined {
	const value = url.searchParams.get(key)?.trim();
	return value ? value : undefined;
}

/** Positive integer param (ids, page numbers); anything else → `undefined`. */
export function intParam(url: UrlLike, key: string): number | undefined {
	const raw = param(url, key);
	if (!raw || !/^\d+$/.test(raw)) return undefined;
	const value = Number(raw);
	return value > 0 ? value : undefined;
}

/** 1-based page from `?page=`, defaulting to 1. */
export function pageParam(url: UrlLike): number {
	return intParam(url, 'page') ?? 1;
}

export function offsetFor(page: number): number {
	return (page - 1) * PAGE_SIZE;
}

export type ParamPatch = Record<string, string | number | null | undefined>;

/**
 * The current path with `patch` applied to its query. `null`, `undefined` and `''` remove
 * a key. Changing anything but `page` returns to page 1, so a new filter never lands on
 * an empty page; `page=1` itself is omitted to keep URLs short.
 */
export function listHref(url: UrlLike, patch: ParamPatch): string {
	const params = new URLSearchParams(url.searchParams.toString());
	if (!('page' in patch)) params.delete('page');
	for (const [key, value] of Object.entries(patch)) {
		if (value === null || value === undefined || value === '') params.delete(key);
		else params.set(key, String(value));
	}
	if (params.get('page') === '1') params.delete('page');
	const query = params.toString();
	return query ? `${url.pathname}?${query}` : url.pathname;
}
