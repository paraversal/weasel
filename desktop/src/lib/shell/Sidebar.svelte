<script lang="ts">
	import { page } from '$app/state';

	import type { NavItem } from '$lib/shell/nav';
	import { icons } from '$lib/shell/icons';
	import { resolvePath } from '$lib/utils/paths';

	let { items }: { items: NavItem[] } = $props();

	function isActive(href: string): boolean {
		const path = page.url.pathname;
		if (href === '/') return path === '/';
		return path === href || path.startsWith(`${href}/`);
	}
</script>

<!-- Nav hrefs are runtime strings; resolvePath applies the app base for us. -->
<!-- eslint-disable svelte/no-navigation-without-resolve -->
<aside
	class="border-border-subtle flex w-56 shrink-0 flex-col gap-1 border-r bg-surface-raised/40 p-2 max-[1100px]:w-14"
>
	<div class="mb-2 flex h-9 items-center gap-2 px-2 max-[1100px]:justify-center max-[1100px]:px-0">
		<span class="size-2 rounded-full bg-primary" aria-hidden="true"></span>
		<span class="text-sm font-semibold max-[1100px]:hidden">Weasel</span>
	</div>

	<nav class="flex flex-col gap-0.5" aria-label="Main">
		{#each items as item (item.href)}
			{@const Icon = icons[item.icon]}
			{@const active = isActive(item.href)}
			<a
				href={resolvePath(item.href)}
				title={item.label}
				aria-current={active ? 'page' : undefined}
				class={[
					'flex h-8 items-center gap-2 rounded-md px-2 text-sm max-[1100px]:justify-center max-[1100px]:px-0',
					'hover:bg-muted hover:text-foreground',
					active ? 'bg-muted text-foreground' : 'text-muted-foreground'
				]}
			>
				<Icon class="size-4 shrink-0" />
				<span class="truncate max-[1100px]:hidden">{item.label}</span>
			</a>
		{/each}
	</nav>
</aside>
