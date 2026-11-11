<script lang="ts">
	import { createRegistry } from '$lib/features/registry.svelte';

	const registry = createRegistry();
</script>

<div class="flex flex-col gap-4">
	<h1 class="text-xl font-semibold">Dashboard</h1>

	<div class="grid gap-3 lg:grid-cols-2">
		{#each registry.current.dashboardWidgets as widget (widget.id)}
			{#await widget.component() then module}
				{@const Widget = module.default}
				<Widget />
			{/await}
		{:else}
			<p class="text-sm text-muted-foreground">No dashboard widgets are registered yet.</p>
		{/each}
	</div>
</div>
