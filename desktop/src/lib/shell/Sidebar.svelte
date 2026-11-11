<script lang="ts">
	import { page } from '$app/state';

	import Weasel from '$lib/components/app/Weasel.svelte';
	import { icons } from '$lib/shell/icons';
	import type { NavItem } from '$lib/shell/nav';
	import { resolvePath } from '$lib/utils/paths';

	let { items }: { items: NavItem[] } = $props();

	/** Item height (h-8 = 32px) plus the flex gap (gap-0.5 = 2px). */
	const STEP = 34;

	function isActive(href: string): boolean {
		const path = page.url.pathname;
		if (href === '/') return path === '/';
		return path === href || path.startsWith(`${href}/`);
	}

	const activeIndex = $derived(items.findIndex((item) => isActive(item.href)));

	let spins = $state(0);
</script>

<aside
	class="border-border-subtle flex w-56 shrink-0 flex-col gap-1 border-r bg-surface-raised/40 p-2 max-[1100px]:w-14"
>
	<div class="mb-2 flex h-9 items-center gap-2 px-2 max-[1100px]:justify-center max-[1100px]:px-0">
		<!-- Easter egg: the weasel does a flip when clicked. -->
		<button
			type="button"
			aria-label="Weasel"
			class="flex size-7 shrink-0 items-center justify-center transition-transform duration-700 ease-[cubic-bezier(0.34,1.56,0.64,1)]"
			style:transform="rotate({spins * 360}deg)"
			onclick={() => spins++}
		>
			<Weasel class="size-7" />
		</button>
		<span class="text-sm font-semibold max-[1100px]:hidden">Weasel</span>
	</div>

	<nav class="relative flex flex-col gap-0.5" aria-label="Main">
		{#if activeIndex >= 0}
			<!-- One highlight that slides to the active entry. -->
			<span
				class="pointer-events-none absolute inset-x-0 top-0 h-8 rounded-md bg-muted shadow-sm transition-transform duration-300 ease-[cubic-bezier(0.34,1.4,0.64,1)]"
				style:transform="translateY({activeIndex * STEP}px)"
				aria-hidden="true"
			></span>
		{/if}
		{#each items as item, index (item.href)}
			{@const Icon = icons[item.icon]}
			{@const active = index === activeIndex}
			<a
				href={resolvePath(item.href)}
				title={item.label}
				aria-current={active ? 'page' : undefined}
				class={[
					'group relative flex h-8 items-center gap-2 rounded-md px-2 text-sm transition-colors max-[1100px]:justify-center max-[1100px]:px-0',
					active
						? 'text-foreground'
						: 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'
				]}
			>
				<Icon class="size-4 shrink-0 group-hover:animate-wiggle" />
				<span
					class="truncate transition-transform duration-200 group-hover:translate-x-0.5 max-[1100px]:hidden"
				>
					{item.label}
				</span>
			</a>
		{/each}
	</nav>
</aside>
