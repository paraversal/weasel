import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query';
import { getContext, setContext } from 'svelte';

import { commands } from '$lib/ipc/bindings';
import { call } from '$lib/ipc/client';
import { keys, settingsQuery } from '$lib/ipc/queries';
import { toast } from '$lib/stores/toasts.svelte';

export type ThemeMode = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

const STORAGE_KEY = 'weasel.theme';

function readCachedMode(): ThemeMode {
	if (typeof window === 'undefined') return 'system';
	const stored = localStorage.getItem(STORAGE_KEY);
	return stored === 'light' || stored === 'dark' ? stored : 'system';
}

export function resolveTheme(mode: ThemeMode, systemDark: boolean): ResolvedTheme {
	if (mode === 'system') return systemDark ? 'dark' : 'light';
	return mode;
}

export interface ThemeController {
	readonly mode: ThemeMode;
	readonly resolved: ResolvedTheme;
	setMode: (next: ThemeMode) => void;
}

/**
 * Applies the theme class before first paint from the localStorage cache, then reconciles
 * with the backend value (the backend wins, because it is what other windows read).
 */
export function createThemeController(): ThemeController {
	let mode = $state<ThemeMode>(readCachedMode());
	let systemDark = $state(false);
	const query = createQuery(() => settingsQuery());
	const queryClient = useQueryClient();

	const resolved = $derived(resolveTheme(mode, systemDark));

	const mutation = createMutation(() => ({
		mutationFn: (next: ThemeMode) => call(commands.settingsUpdate({ patch: { theme: next } })),
		onMutate: (next: ThemeMode) => {
			const previous = mode;
			mode = next;
			return { previous };
		},
		onError: (error, _next, context) => {
			if (context?.previous) mode = context.previous;
			toast.ipcError(error);
		},
		onSettled: () => queryClient.invalidateQueries({ queryKey: keys.settings })
	}));

	$effect(() => {
		if (typeof window === 'undefined') return;
		const root = document.documentElement;
		root.classList.toggle('dark', resolved === 'dark');
		root.dataset.theme = resolved;
		localStorage.setItem(STORAGE_KEY, mode);
	});

	$effect(() => {
		if (typeof window === 'undefined') return;
		const media = window.matchMedia('(prefers-color-scheme: dark)');
		systemDark = media.matches;
		const onChange = (event: MediaQueryListEvent) => {
			systemDark = event.matches;
		};
		media.addEventListener('change', onChange);
		return () => media.removeEventListener('change', onChange);
	});

	$effect(() => {
		const persisted = query.data?.theme;
		if (!persisted) return;
		// While a write is in flight the local value is the newer intent.
		if (mutation.isPending) return;
		if (persisted !== mode) mode = persisted;
	});

	return {
		get mode() {
			return mode;
		},
		get resolved() {
			return resolved;
		},
		setMode(next: ThemeMode) {
			if (next === mode) return;
			mutation.mutate(next);
		}
	};
}

const THEME_CONTEXT = Symbol('weasel.theme');

/** One controller per window: every theme control must share the same optimistic state. */
export function provideTheme(theme: ThemeController): void {
	setContext(THEME_CONTEXT, theme);
}

export function useTheme(): ThemeController {
	return getContext<ThemeController>(THEME_CONTEXT);
}
