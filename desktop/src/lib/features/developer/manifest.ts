import type { FeatureManifest } from '$lib/features/types';

export default {
	id: 'developer',
	title: 'Developer',
	description: 'Diagnostics for the feature registry.',
	order: 1000,
	hidden: true,
	commands: [
		{
			id: 'developer.features',
			title: 'Developer: features',
			keywords: ['diagnostics', 'registry', 'debug'],
			group: 'Developer',
			when: () => import.meta.env.DEV,
			run: (context) => context.goto('/dev/features')
		}
	]
} satisfies FeatureManifest;
