import type { FeatureManifest } from '$lib/features/types';

export default {
	id: 'dashboard',
	title: 'Dashboard',
	description: 'Status of the app and its database.',
	order: 0,
	nav: [{ label: 'Dashboard', href: '/', icon: 'dashboard', order: 0, shortcut: 'g d' }],
	dashboard: [
		{
			id: 'dashboard.status',
			order: 0,
			component: () => import('./components/AppStatusWidget.svelte')
		}
	]
} satisfies FeatureManifest;
