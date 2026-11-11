<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, actions, children }: { title: string; actions?: Snippet; children: Snippet } =
		$props();
</script>

<section class="border-border-subtle rounded-lg border bg-surface-raised">
	<div class="border-border-subtle flex items-center justify-between gap-2 border-b px-4 py-2.5">
		<h2 class="text-sm font-semibold">{title}</h2>
		{@render actions?.()}
	</div>
	<div class="p-4">
		{@render children()}
	</div>
</section>
