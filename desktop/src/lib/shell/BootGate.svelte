<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import { Skeleton } from '$lib/components/ui/skeleton/index.js';
	import { appQueries } from '$lib/ipc/queries';
	import { createThemeController, provideTheme } from '$lib/stores/theme.svelte';

	import AppShell from './AppShell.svelte';
	import BootError from './BootError.svelte';

	let { children }: { children: import('svelte').Snippet } = $props();

	const theme = createThemeController();
	provideTheme(theme);
	const health = createQuery(() => appQueries.health());
	const info = createQuery(() => appQueries.info());

	const broken = $derived(health.isError || health.data?.dbOk === false);
	const failure = $derived(
		health.isError
			? health.error instanceof Error
				? health.error.message
				: 'The backend is unavailable.'
			: health.data?.bootError
	);
</script>

{#if health.isPending}
	<div class="flex h-dvh">
		<div class="border-border-subtle w-56 border-r p-2">
			<Skeleton class="mb-3 h-8 w-full" />
			<Skeleton class="mb-1 h-8 w-full" />
			<Skeleton class="mb-1 h-8 w-full" />
		</div>
		<div class="flex-1 p-6">
			<Skeleton class="mb-4 h-9 w-72" />
			<Skeleton class="h-40 w-full" />
		</div>
	</div>
{:else if broken}
	<BootError message={failure ?? 'The database is unavailable.'} dataDir={info.data?.dataDir} />
{:else}
	<AppShell {theme}>{@render children()}</AppShell>
{/if}
