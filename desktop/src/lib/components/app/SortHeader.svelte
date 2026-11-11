<script lang="ts">
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import ArrowUpDownIcon from '@lucide/svelte/icons/arrow-up-down';

	import * as Table from '$lib/components/ui/table/index.js';

	/**
	 * A column header that sorts the list. `sorts` are the sort values this column cycles
	 * through (one for a fixed direction, two to toggle); `direction` maps each to an arrow.
	 */
	let {
		label,
		sorts,
		current,
		direction,
		hrefFor,
		class: className
	}: {
		label: string;
		sorts: readonly string[];
		current: string;
		direction: Record<string, 'asc' | 'desc'>;
		hrefFor: (sort: string) => string;
		class?: string;
	} = $props();

	const index = $derived(sorts.indexOf(current));
	const active = $derived(index !== -1);
	const next = $derived(sorts[(index + 1) % sorts.length] ?? sorts[0] ?? current);
	const dir = $derived(active ? direction[current] : undefined);
</script>

<Table.Head
	class={className}
	aria-sort={dir === 'asc' ? 'ascending' : dir === 'desc' ? 'descending' : undefined}
>
	<a
		href={hrefFor(next)}
		title="Sort by {label.toLowerCase()}"
		class={[
			'-mx-2 inline-flex h-7 cursor-pointer items-center gap-1 rounded-md px-2 hover:bg-muted hover:text-foreground',
			active ? 'bg-muted/60 text-foreground' : 'text-muted-foreground'
		]}
	>
		{label}
		{#if dir === 'asc'}
			<ArrowUpIcon class="size-3.5" />
		{:else if dir === 'desc'}
			<ArrowDownIcon class="size-3.5" />
		{:else}
			<ArrowUpDownIcon class="size-3.5 opacity-60" />
		{/if}
	</a>
</Table.Head>
