import type { FeatureManifest } from '$lib/features/types';

export default {
	id: 'settings',
	title: 'Settings',
	description: 'Theme, data and feature-flag preferences.',
	order: 90,
	nav: [{ label: 'Settings', href: '/settings', icon: 'settings', order: 90, shortcut: 'g s' }],
	settings: [
		{
			id: 'settings.general',
			title: 'General',
			order: 10,
			component: () => import('./components/GeneralSettings.svelte')
		}
	]
} satisfies FeatureManifest;
