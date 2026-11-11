<script lang="ts">
	/** URL-driven segmented control: each tab is a link, so filters survive reload. */
	let {
		items,
		label
	}: { items: { label: string; href: string; active: boolean }[]; label: string } = $props();
</script>

<nav aria-label={label} class="inline-flex rounded-lg bg-muted p-0.5">
	{#each items as item (item.href)}
		<a
			href={item.href}
			aria-current={item.active ? 'page' : undefined}
			class={[
				'rounded-md px-3 py-1 text-sm font-medium',
				item.active
					? 'bg-background text-foreground shadow-sm'
					: 'text-muted-foreground hover:text-foreground'
			]}
		>
			{item.label}
		</a>
	{/each}
</nav>
