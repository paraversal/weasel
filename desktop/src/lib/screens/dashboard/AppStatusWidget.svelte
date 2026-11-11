<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call } from '$lib/ipc/client';
	import { appQueries } from '$lib/ipc/queries';
	import { toast } from '$lib/stores/toasts.svelte';

	const info = createQuery(() => appQueries.info());
	const health = createQuery(() => appQueries.health());

	const rows = $derived([
		{ label: 'Version', value: info.data?.version ?? '…' },
		{ label: 'API version', value: info.data?.apiVersion ?? '…' },
		{ label: 'Schema version', value: health.data?.schemaVersion?.toString() ?? '…' },
		{ label: 'Contacts', value: health.data?.contactCount?.toString() ?? '…' },
		{ label: 'Pending imports', value: health.data?.pendingImports?.toString() ?? '…' },
		{ label: 'Database', value: info.data?.dbPath ?? '…', mono: true }
	]);

	async function openDataFolder() {
		try {
			await call(commands.appOpenDataDir());
		} catch (error) {
			toast.ipcError(error);
		}
	}
</script>

<section class="border-border-subtle rounded-lg border bg-surface-raised p-4">
	<header class="mb-3 flex items-center justify-between gap-2">
		<h2 class="text-sm font-semibold">Application status</h2>
		{#if health.data}
			<Badge variant={health.data.dbOk ? 'secondary' : 'destructive'}>
				{health.data.dbOk ? 'Database ready' : 'Database unavailable'}
			</Badge>
		{:else}
			<Badge variant="outline">Checking…</Badge>
		{/if}
	</header>

	<dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm">
		{#each rows as row (row.label)}
			<dt class="text-muted-foreground">{row.label}</dt>
			<dd class={row.mono ? 'truncate font-mono text-xs' : ''} title={row.value}>{row.value}</dd>
		{/each}
	</dl>

	<div class="mt-3 flex justify-end">
		<Button variant="outline" size="sm" onclick={openDataFolder}>Open data folder</Button>
	</div>
</section>
