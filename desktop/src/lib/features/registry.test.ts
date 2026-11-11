import { describe, expect, it, vi } from 'vitest';

import type { QueryClient } from '@tanstack/svelte-query';
import type { Component } from 'svelte';

import { buildRegistry } from './registry';
import type { CommandContext, FeatureManifest } from './types';

const queryClient = {} as QueryClient;

function context(goto = vi.fn()): CommandContext {
	return {
		goto,
		queryClient,
		toast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), ipcError: vi.fn() }
	};
}

function manifest(
	overrides: Partial<FeatureManifest> & Pick<FeatureManifest, 'id' | 'title'>
): FeatureManifest {
	return { ...overrides };
}

describe('buildRegistry', () => {
	it('orders features and nav items by order then id', () => {
		const registry = buildRegistry(
			[
				manifest({
					id: 'b',
					title: 'B',
					order: 20,
					nav: [{ label: 'B', href: '/b', icon: 'users' }]
				}),
				manifest({
					id: 'a',
					title: 'A',
					order: 10,
					nav: [{ label: 'A', href: '/a', icon: 'mail', order: 5 }]
				}),
				manifest({ id: 'c', title: 'C', nav: [{ label: 'C', href: '/c', icon: 'help' }] })
			],
			{}
		);

		expect(registry.features.map((feature) => feature.id)).toEqual(['a', 'b', 'c']);
		expect(registry.navItems.map((item) => item.href)).toEqual(['/a', '/b', '/c']);
	});

	it('warns on duplicate feature ids and throws in strict mode', () => {
		const manifests = [
			manifest({ id: 'same', title: 'One' }),
			manifest({ id: 'same', title: 'Two' })
		];

		const registry = buildRegistry(manifests, {});
		expect(registry.warnings.map((warning) => warning.code)).toContain('duplicate-feature');
		expect(() => buildRegistry(manifests, {}, { strict: true })).toThrow(/registered twice/);
	});

	it('warns on duplicate palette command ids, including auto navigation commands', () => {
		const registry = buildRegistry(
			[
				manifest({
					id: 'one',
					title: 'One',
					commands: [{ id: 'one.go', title: 'Go', run: () => {} }],
					nav: [{ label: 'Dashboard', href: '/', icon: 'dashboard' }]
				}),
				manifest({
					id: 'two',
					title: 'Two',
					commands: [{ id: 'one.go', title: 'Duplicate', run: () => {} }]
				})
			],
			{}
		);

		expect(
			registry.warnings.filter((warning) => warning.code === 'duplicate-command')
		).toHaveLength(1);
		expect(() =>
			buildRegistry(
				[manifest({ id: 'x', title: 'X', nav: [{ label: 'D', href: '/', icon: 'dashboard' }] })],
				{}
			)
		).not.toThrow();
	});

	it('reports shortcut conflicts and unparseable shortcuts without failing', () => {
		const registry = buildRegistry(
			[
				manifest({
					id: 'one',
					title: 'One',
					commands: [
						{ id: 'one.a', title: 'A', shortcut: 'mod+k', run: () => {} },
						{ id: 'one.b', title: 'B', shortcut: 'mod+k', run: () => {} },
						{ id: 'one.c', title: 'C', shortcut: 'nonsense+', run: () => {} }
					]
				})
			],
			{}
		);

		const codes = registry.warnings.map((warning) => warning.code);
		expect(codes).toContain('shortcut-conflict');
		expect(codes).toContain('invalid-shortcut');
	});

	it('hides flagged-off features entirely and shows them when enabled', () => {
		const manifests = [
			manifest({
				id: 'beta',
				title: 'Beta',
				flag: 'beta-tools',
				nav: [{ label: 'Beta', href: '/beta', icon: 'code' }],
				commands: [{ id: 'beta.run', title: 'Run', run: () => {} }]
			})
		];

		expect(buildRegistry(manifests, {}).features).toHaveLength(0);
		const enabled = buildRegistry(manifests, { 'beta-tools': true });
		expect(enabled.features).toHaveLength(1);
		expect(enabled.paletteCommands.map((command) => command.id)).toContain('beta.run');
	});

	it('derives a navigation command from every nav item that navigates', () => {
		const registry = buildRegistry(
			[
				manifest({
					id: 'one',
					title: 'One',
					nav: [{ label: 'Imports', href: '/imports', icon: 'upload' }]
				})
			],
			{}
		);
		const navCommand = registry.paletteCommands.find((command) => command.id === 'nav:/imports');
		expect(navCommand?.title).toBe('Go to Imports');
		expect(navCommand?.group).toBe('Navigation');

		const goto = vi.fn();
		navCommand?.run(context(goto));
		expect(goto).toHaveBeenCalledWith('/imports');
	});

	it('keeps commands of hidden features but drops their nav entries', () => {
		const registry = buildRegistry(
			[
				manifest({
					id: 'dev',
					title: 'Developer',
					hidden: true,
					nav: [{ label: 'Developer', href: '/developer', icon: 'code' }],
					commands: [{ id: 'dev.copy', title: 'Copy diagnostics', run: () => {} }]
				})
			],
			{}
		);

		expect(registry.navItems).toHaveLength(0);
		expect(registry.paletteCommands.map((command) => command.id)).toEqual(['dev.copy']);
	});

	it('sorts dashboard widgets and settings sections', () => {
		const Widget = (() => {}) as unknown as Component;
		const registry = buildRegistry(
			[
				manifest({
					id: 'one',
					title: 'One',
					order: 20,
					dashboard: [{ id: 'one.z', order: 2, component: async () => ({ default: Widget }) }],
					settings: [
						{ id: 'one.s', title: 'S', order: 2, component: async () => ({ default: Widget }) }
					]
				}),
				manifest({
					id: 'two',
					title: 'Two',
					order: 10,
					dashboard: [{ id: 'two.a', order: 1, component: async () => ({ default: Widget }) }],
					settings: [
						{ id: 'two.s', title: 'S', order: 1, component: async () => ({ default: Widget }) }
					]
				})
			],
			{}
		);

		expect(registry.dashboardWidgets.map((widget) => widget.id)).toEqual(['two.a', 'one.z']);
		expect(registry.settingsSections.map((section) => section.id)).toEqual(['two.s', 'one.s']);
	});
});
