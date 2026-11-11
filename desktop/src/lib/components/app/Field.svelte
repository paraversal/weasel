<script lang="ts">
	import type { Snippet } from 'svelte';

	import { Label } from '$lib/components/ui/label/index.js';

	let {
		id,
		label,
		error,
		children
	}: { id: string; label: string; error?: string; children: Snippet } = $props();
</script>

<div class="flex flex-col gap-1.5">
	<Label for={id}>{label}</Label>
	{@render children()}
	{#if error}
		<p class="text-xs text-destructive" id="{id}-error">{error}</p>
	{/if}
</div>
