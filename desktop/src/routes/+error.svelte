<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';

	import { Button } from '$lib/components/ui/button/index.js';

	const message = $derived(
		page.error?.message ??
			(page.status === 404 ? 'This page does not exist.' : 'Something went wrong.')
	);
</script>

<div class="flex h-full flex-col items-center justify-center gap-3 text-center">
	<p class="font-mono text-xs text-muted-foreground">{page.status}</p>
	<h1 class="text-lg font-semibold">{message}</h1>
	<Button variant="outline" onclick={() => goto(resolve('/'))}>Go to dashboard</Button>
</div>
