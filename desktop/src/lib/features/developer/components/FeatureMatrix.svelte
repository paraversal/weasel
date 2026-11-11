<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import { Badge } from '$lib/components/ui/badge/index.js';
	import { createRegistry } from '$lib/features/registry.svelte';
	import { appQueries } from '$lib/ipc/queries';

	const registry = createRegistry();
	const backend = createQuery(() => appQueries.features());

	type Row = { id: string; frontend: boolean; backend: boolean; note: string };

	const rows = $derived.by<Row[]>(() => {
		const frontendIds = new Set(registry.current.features.map((feature) => feature.id));
		const backendIds = new Set((backend.data ?? []).map((feature) => feature.id));
		const ids = [...new Set([...frontendIds, ...backendIds])].sort();

		return ids.map((id) => {
			const frontend = frontendIds.has(id);
			const isBackend = backendIds.has(id);
			// A frontend-only feature is normal: the shell itself contributes some.
			const note = frontend && isBackend ? '' : frontend ? 'frontend only' : 'backend only';
			return { id, frontend, backend: isBackend, note };
		});
	});
</script>

<div class="flex flex-col gap-4">
	<h1 class="text-xl font-semibold">Features</h1>
	<p class="text-sm text-muted-foreground">
		Frontend manifests compared with the backend's `app_features`. A feature on one side only is
		highlighted; a frontend-only entry is not an error.
	</p>

	{#if backend.isPending}
		<p class="text-sm text-muted-foreground">Loading backend features…</p>
	{:else if backend.isError}
		<p class="text-sm text-danger">{backend.error.message}</p>
	{:else}
		<div class="border-border-subtle overflow-hidden rounded-lg border">
			<table class="w-full text-sm">
				<thead class="bg-muted/40 text-left text-xs text-muted-foreground">
					<tr>
						<th class="px-4 py-2 font-medium">Feature</th>
						<th class="px-4 py-2 font-medium">Frontend</th>
						<th class="px-4 py-2 font-medium">Backend</th>
						<th class="px-4 py-2 font-medium">Note</th>
					</tr>
				</thead>
				<tbody>
					{#each rows as row (row.id)}
						<tr class="border-border-subtle border-t">
							<td class="px-4 py-2 font-mono text-xs">{row.id}</td>
							<td class="px-4 py-2">
								<Badge variant={row.frontend ? 'secondary' : 'outline'}>
									{row.frontend ? 'yes' : 'no'}
								</Badge>
							</td>
							<td class="px-4 py-2">
								<Badge variant={row.backend ? 'secondary' : 'outline'}>
									{row.backend ? 'yes' : 'no'}
								</Badge>
							</td>
							<td class="px-4 py-2 text-xs text-muted-foreground">{row.note}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>
