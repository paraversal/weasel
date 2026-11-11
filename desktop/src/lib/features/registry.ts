import { canonicalShortcut, formatShortcut, parseShortcut } from '$lib/utils/shortcuts';

import type {
	DashboardWidget,
	FeatureManifest,
	NavItem,
	PaletteCommand,
	SettingsSection
} from './types';

export interface NavEntry extends NavItem {
	featureId: string;
	featureTitle: string;
}

export interface PaletteEntry extends PaletteCommand {
	featureId: string;
	group: string;
}

export interface DashboardEntry extends DashboardWidget {
	featureId: string;
}

export interface SettingsEntry extends SettingsSection {
	featureId: string;
}

export type WarningCode =
	'duplicate-feature' | 'duplicate-command' | 'shortcut-conflict' | 'invalid-shortcut';

export interface RegistryWarning {
	code: WarningCode;
	message: string;
}

export interface Registry {
	features: FeatureManifest[];
	navItems: NavEntry[];
	paletteCommands: PaletteEntry[];
	dashboardWidgets: DashboardEntry[];
	settingsSections: SettingsEntry[];
	warnings: RegistryWarning[];
}

export interface BuildOptions {
	/**
	 * Throw on a duplicate id instead of only warning. Enabled outside tests: a duplicate
	 * is a programming error, and failing at startup beats a silently shadowed command.
	 */
	strict?: boolean;
	platform?: 'mac' | 'other';
}

const DEFAULT_ORDER = 100;

function byOrder<T extends { order?: number }>(a: T, b: T): number {
	return (a.order ?? DEFAULT_ORDER) - (b.order ?? DEFAULT_ORDER);
}

/** A nav item also becomes a palette command, so the palette covers all navigation. */
function navCommand(item: NavEntry): PaletteEntry {
	return {
		id: `nav:${item.href}`,
		title: `Go to ${item.label}`,
		keywords: [item.label, item.featureTitle],
		shortcut: item.shortcut,
		group: 'Navigation',
		run: (context) => context.goto(item.href),
		featureId: item.featureId
	};
}

/**
 * Pure composition step: manifests + feature flags in, everything the shell renders out.
 * Discovery lives below so tests can feed synthetic manifests.
 */
export function buildRegistry(
	manifests: FeatureManifest[],
	flags: Record<string, boolean>,
	options: BuildOptions = {}
): Registry {
	const warnings: RegistryWarning[] = [];
	const strict = options.strict ?? false;

	const features = manifests
		.filter((manifest) => !manifest.flag || flags[manifest.flag] === true)
		.slice()
		.sort((a, b) => byOrder(a, b) || a.id.localeCompare(b.id));

	const seenFeatures = new Set<string>();
	for (const feature of features) {
		if (seenFeatures.has(feature.id)) {
			warnings.push({
				code: 'duplicate-feature',
				message: `feature id "${feature.id}" is registered twice`
			});
		}
		seenFeatures.add(feature.id);
	}

	const navItems: NavEntry[] = features
		.filter((feature) => !feature.hidden)
		.flatMap((feature) =>
			(feature.nav ?? []).map((item) => ({
				...item,
				featureId: feature.id,
				featureTitle: feature.title
			}))
		)
		.sort((a, b) => byOrder(a, b) || a.label.localeCompare(b.label));

	const declaredCommands: PaletteEntry[] = features.flatMap((feature) =>
		(feature.commands ?? []).map((command) => ({
			...command,
			featureId: feature.id,
			group: command.group ?? feature.title
		}))
	);

	const paletteCommands = [...navItems.map(navCommand), ...declaredCommands];
	const seenCommands = new Set<string>();
	for (const command of paletteCommands) {
		if (seenCommands.has(command.id)) {
			warnings.push({
				code: 'duplicate-command',
				message: `palette command id "${command.id}" is registered twice`
			});
		}
		seenCommands.add(command.id);
	}

	const shortcuts = new Map<string, string>();
	for (const command of paletteCommands) {
		if (!command.shortcut) continue;
		if (!parseShortcut(command.shortcut)) {
			warnings.push({
				code: 'invalid-shortcut',
				message: `command "${command.id}" has an unparseable shortcut "${command.shortcut}"`
			});
			continue;
		}
		const canonical = canonicalShortcut(command.shortcut);
		const owner = shortcuts.get(canonical);
		if (owner) {
			const formatted = options.platform
				? formatShortcut(command.shortcut, options.platform)
				: command.shortcut;
			warnings.push({
				code: 'shortcut-conflict',
				message: `shortcut ${formatted} is claimed by "${owner}" and "${command.id}"`
			});
			continue;
		}
		shortcuts.set(canonical, command.id);
	}

	if (strict) {
		const fatal = warnings.find(
			(warning) => warning.code === 'duplicate-feature' || warning.code === 'duplicate-command'
		);
		if (fatal) throw new Error(`feature registry: ${fatal.message}`);
	}

	const dashboardWidgets: DashboardEntry[] = features
		.flatMap((feature) =>
			(feature.dashboard ?? []).map((widget) => ({ ...widget, featureId: feature.id }))
		)
		.sort((a, b) => byOrder(a, b) || a.id.localeCompare(b.id));

	const settingsSections: SettingsEntry[] = features
		.flatMap((feature) =>
			(feature.settings ?? []).map((section) => ({ ...section, featureId: feature.id }))
		)
		.sort((a, b) => byOrder(a, b) || a.id.localeCompare(b.id));

	return {
		features,
		navItems,
		paletteCommands,
		dashboardWidgets,
		settingsSections,
		warnings
	};
}

const discovered = import.meta.glob<{ default: FeatureManifest }>('./*/manifest.ts', {
	eager: true
});

export const manifests: FeatureManifest[] = Object.values(discovered).map(
	(module) => module.default
);
