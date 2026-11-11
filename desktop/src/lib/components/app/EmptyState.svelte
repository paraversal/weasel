<script lang="ts">
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import type { Snippet } from 'svelte';

	let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<div
	class="border-border-subtle flex animate-rise flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center"
>
	<SparklesIcon class="size-6 animate-float text-muted-foreground" aria-hidden="true" />
	<p class="text-sm font-medium">{title}</p>
	{#if children}
		<div class="flex flex-col items-center gap-3 text-sm text-muted-foreground">
			{@render children()}
		</div>
	{/if}
</div>
