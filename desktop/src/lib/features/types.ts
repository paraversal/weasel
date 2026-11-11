import type { QueryClient } from '@tanstack/svelte-query';
import type { Component } from 'svelte';

import type { IconName } from '$lib/shell/icons';
import type { ToastApi } from '$lib/stores/toasts.svelte';

/**
 * Everything a feature may contribute to the shell. A feature is a manifest plus its own
 * routes and components; the shell never imports a feature directly.
 */
export interface FeatureManifest {
	id: string;
	title: string;
	description?: string;
	order?: number;
	/** Feature flag id; when set the feature is hidden unless the flag is enabled. */
	flag?: string;
	/** Registered but never linked (developer tools): routes exist, no nav entry. */
	hidden?: boolean;
	nav?: NavItem[];
	commands?: PaletteCommand[];
	dashboard?: DashboardWidget[];
	settings?: SettingsSection[];
}

export interface NavItem {
	label: string;
	href: string;
	icon: IconName;
	order?: number;
	/** Two-key navigation sequence, e.g. `"g d"`. */
	shortcut?: string;
	badge?: () => number | undefined;
}

export interface PaletteCommand {
	/** Stable, namespaced: `"campaigns.create"`. */
	id: string;
	title: string;
	/** Extra fuzzy-match terms; the title is always matched. */
	keywords?: string[];
	shortcut?: string;
	/** Group heading in the palette. Defaults to the owning feature's title. */
	group?: string;
	when?: () => boolean;
	run: (context: CommandContext) => void | Promise<void>;
}

export interface DashboardWidget {
	id: string;
	order?: number;
	component: () => Promise<{ default: Component }>;
}

export interface SettingsSection {
	id: string;
	title: string;
	order?: number;
	component: () => Promise<{ default: Component }>;
}

export interface CommandContext {
	goto: (href: string) => void | Promise<void>;
	queryClient: QueryClient;
	toast: ToastApi;
}

export type FeatureId = string;
