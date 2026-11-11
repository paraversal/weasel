import type { FeatureManifest } from '$lib/features/types';

export default {
	id: 'help',
	title: 'Help',
	description: 'Keyboard shortcuts and getting started.',
	order: 95,
	nav: [{ label: 'Help', href: '/help', icon: 'help', order: 95, shortcut: 'g h' }],
	commands: [
		{
			id: 'help.shortcuts',
			title: 'Keyboard shortcuts',
			keywords: ['keys', 'hotkeys', 'reference'],
			group: 'Help',
			run: (context) => context.goto('/help')
		}
	]
} satisfies FeatureManifest;
