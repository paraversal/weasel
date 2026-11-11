<script lang="ts">
	import MonitorIcon from '@lucide/svelte/icons/monitor';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SunIcon from '@lucide/svelte/icons/sun';

	import { Button } from '$lib/components/ui/button/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import type { ThemeController, ThemeMode } from '$lib/stores/theme.svelte';
	import { formatShortcut, type Platform } from '$lib/utils/shortcuts';

	let {
		onOpenPalette,
		theme,
		platform
	}: {
		onOpenPalette: () => void;
		theme: ThemeController;
		platform: Platform;
	} = $props();

	const themeOptions: Array<{ mode: ThemeMode; label: string; icon: typeof SunIcon }> = [
		{ mode: 'system', label: 'System', icon: MonitorIcon },
		{ mode: 'light', label: 'Light', icon: SunIcon },
		{ mode: 'dark', label: 'Dark', icon: MoonIcon }
	];

	const paletteHint = $derived(formatShortcut('mod+k', platform));
</script>

<header class="border-border-subtle flex h-12 shrink-0 items-center gap-3 border-b px-3">
	<Button
		variant="outline"
		class="h-8 w-full max-w-md justify-between text-muted-foreground"
		onclick={onOpenPalette}
	>
		<span class="flex items-center gap-2">
			<SearchIcon class="size-4" />
			<span class="text-sm">Search or run a command</span>
		</span>
		<kbd class="border-border-subtle rounded border bg-muted px-1.5 py-0.5 font-mono text-[0.7rem]">
			{paletteHint}
		</kbd>
	</Button>

	<div class="ml-auto flex items-center gap-2">
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<Button {...props} variant="ghost" size="icon-sm" aria-label="Theme">
						{#if theme.mode === 'light'}
							<SunIcon class="size-4" />
						{:else if theme.mode === 'dark'}
							<MoonIcon class="size-4" />
						{:else}
							<MonitorIcon class="size-4" />
						{/if}
					</Button>
				{/snippet}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end">
				<DropdownMenu.Label>Theme</DropdownMenu.Label>
				{#each themeOptions as option (option.mode)}
					<DropdownMenu.Item onSelect={() => theme.setMode(option.mode)}>
						<option.icon class="size-4" />
						{option.label}
						{#if theme.mode === option.mode}
							<span class="ml-auto text-xs text-muted-foreground">Active</span>
						{/if}
					</DropdownMenu.Item>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</div>
</header>
