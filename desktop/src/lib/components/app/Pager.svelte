<script lang="ts">
	import { page } from '$app/state';

	import { Button } from '$lib/components/ui/button/index.js';
	import { formatNumber } from '$lib/utils/format';
	import { PAGE_SIZE, listHref } from '$lib/utils/list-params';
	import { href } from '$lib/utils/paths';

	let { total, offset }: { total: number; offset: number } = $props();

	const current = $derived(Math.floor(offset / PAGE_SIZE) + 1);
	const last = $derived(Math.max(1, Math.ceil(total / PAGE_SIZE)));
	const first = $derived(total === 0 ? 0 : offset + 1);
	const end = $derived(Math.min(offset + PAGE_SIZE, total));
</script>

{#if total > 0}
	<div class="flex items-center justify-between gap-2 text-sm text-muted-foreground">
		<span>{formatNumber(first)}–{formatNumber(end)} of {formatNumber(total)}</span>
		{#if last > 1}
			<div class="flex items-center gap-1">
				<Button
					variant="outline"
					size="sm"
					href={current > 1 ? href(listHref(page.url, { page: current - 1 })) : undefined}
					disabled={current <= 1}
				>
					Previous
				</Button>
				<span class="px-2">Page {current} of {last}</span>
				<Button
					variant="outline"
					size="sm"
					href={current < last ? href(listHref(page.url, { page: current + 1 })) : undefined}
					disabled={current >= last}
				>
					Next
				</Button>
			</div>
		{/if}
	</div>
{/if}
