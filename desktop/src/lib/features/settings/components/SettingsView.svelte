<script lang="ts">
	import { createRegistry } from '$lib/features/registry.svelte';

	const registry = createRegistry();
</script>

<div class="flex flex-col gap-6">
	<h1 class="text-xl font-semibold">Settings</h1>

	{#each registry.current.settingsSections as section (section.id)}
		<section class="border-border-subtle rounded-lg border bg-surface-raised p-4">
			<h2 class="mb-3 text-sm font-semibold">{section.title}</h2>
			{#await section.component() then module}
				{@const Section = module.default}
				<Section />
			{/await}
		</section>
	{/each}
</div>
