<script lang="ts">
	import { createRegistry } from '$lib/features/registry.svelte';
	import { detectPlatform, formatShortcut } from '$lib/utils/shortcuts';

	const registry = createRegistry();
	const platform = detectPlatform();

	/** Shell bindings are not palette commands, so they are listed explicitly. */
	const builtIns: Array<{ label: string; shortcut: string }> = [
		{ label: 'Command palette', shortcut: 'mod+k' },
		{ label: 'Keyboard shortcuts (this page)', shortcut: '?' },
		{ label: 'Close dialog or palette', shortcut: 'Escape' },
		{ label: 'Primary action in a dialog', shortcut: 'mod+enter' }
	];

	const rows = $derived([
		...builtIns,
		...registry.current.paletteCommands
			.filter((command) => command.shortcut)
			.map((command) => ({ label: command.title, shortcut: command.shortcut ?? '' }))
	]);
</script>

<div class="flex max-w-3xl flex-col gap-4">
	<h1 class="text-xl font-semibold">Help</h1>
	<p class="text-sm text-muted-foreground">
		Press a key sequence anywhere outside a text field. Typing in an input never triggers
		bare-letter navigation.
	</p>

	<section class="border-border-subtle rounded-lg border bg-surface-raised">
		<h2 class="border-border-subtle border-b px-4 py-2.5 text-sm font-semibold">Shortcuts</h2>
		<dl class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-4">
			{#each rows as row (`${row.label}:${row.shortcut}`)}
				<div class="contents">
					<dt class="border-border-subtle border-b px-4 py-2 text-sm">{row.label}</dt>
					<dd class="border-border-subtle border-b px-4 py-2 text-right">
						<kbd
							class="border-border-subtle rounded border bg-muted px-1.5 py-0.5 font-mono text-xs"
						>
							{formatShortcut(row.shortcut, platform)}
						</kbd>
					</dd>
				</div>
			{/each}
		</dl>
	</section>
</div>
