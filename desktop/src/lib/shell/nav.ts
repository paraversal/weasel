import type { Term } from '$lib/stores/term.svelte';

import type { IconName } from './icons';

/** Sidebar entries, in display order. Adding a screen means adding a route and a line here. */
export interface NavItem {
	label: string;
	href: string;
	icon: IconName;
	/** Two-key sequence such as `"g p"`; Help uses the shell's `?` binding instead. */
	shortcut?: string;
}

export function navItems(term: Term): NavItem[] {
	return [
		{ label: 'Dashboard', href: '/', icon: 'dashboard', shortcut: 'g d' },
		{ label: term.Many, href: '/contacts', icon: 'users', shortcut: 'g p' },
		{ label: 'Campaigns', href: '/campaigns', icon: 'mail', shortcut: 'g c' },
		{ label: 'History', href: '/history', icon: 'history', shortcut: 'g h' },
		{ label: 'Imports', href: '/imports', icon: 'upload', shortcut: 'g i' },
		{ label: 'Settings', href: '/settings', icon: 'settings', shortcut: 'g s' },
		{ label: 'Help', href: '/help', icon: 'help' }
	];
}

/** A palette command is a destination; screens react to query params such as `?new=1`. */
export interface PaletteEntry {
	id: string;
	title: string;
	group: string;
	href: string;
	keywords?: string[];
	shortcut?: string;
}

export function paletteEntries(term: Term): PaletteEntry[] {
	return [
		{ id: 'imports.new', title: 'Import list', group: 'Actions', href: '/imports/new' },
		{
			id: 'campaigns.new',
			title: 'New campaign',
			group: 'Actions',
			href: '/campaigns?new=1',
			keywords: ['create']
		},
		{
			id: 'contacts.new',
			title: `Add ${term.one}`,
			group: 'Actions',
			href: '/contacts?new=1',
			keywords: ['contact', 'create']
		},
		...navItems(term).map((item) => ({
			id: `nav:${item.href}`,
			title: `Go to ${item.label}`,
			group: 'Navigation',
			href: item.href,
			keywords: [item.label],
			shortcut: item.shortcut
		}))
	];
}
