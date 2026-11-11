<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';

	import * as Command from '$lib/components/ui/command/index.js';
	import type { PaletteEntry } from '$lib/features/registry';
	import { formatShortcut, type Platform } from '$lib/utils/shortcuts';

	let {
		open = $bindable(false),
		commands,
		platform,
		onRun
	}: {
		open?: boolean;
		commands: PaletteEntry[];
		platform: Platform;
		onRun: (entry: PaletteEntry) => void;
	} = $props();

	let query = $state('');

	const available = $derived(commands.filter((entry) => !entry.when || entry.when()));

	const groups = $derived.by(() => {
		const byGroup = new SvelteMap<string, PaletteEntry[]>();
		for (const entry of available) {
			const list = byGroup.get(entry.group);
			if (list) list.push(entry);
			else byGroup.set(entry.group, [entry]);
		}
		return [...byGroup].map(([heading, entries]) => ({ heading, entries }));
	});

	function select(entry: PaletteEntry) {
		open = false;
		onRun(entry);
	}

	// A fresh palette starts with an empty query; stale queries hide the item you expect.
	$effect(() => {
		if (!open) query = '';
	});
</script>

<Command.Dialog
	bind:open
	bind:value={query}
	title="Command palette"
	description="Search for a command to run."
	class="max-w-xl"
>
	<Command.Input placeholder="Type a command or page…" />
	<Command.List>
		<Command.Empty>No matching commands.</Command.Empty>
		{#each groups as group (group.heading)}
			<Command.Group heading={group.heading}>
				{#each group.entries as entry (entry.id)}
					<Command.Item
						value={[entry.title, ...(entry.keywords ?? [])].join(' ')}
						onSelect={() => select(entry)}
					>
						<span class="truncate">{entry.title}</span>
						{#if entry.shortcut}
							<Command.Shortcut>{formatShortcut(entry.shortcut, platform)}</Command.Shortcut>
						{/if}
					</Command.Item>
				{/each}
			</Command.Group>
		{/each}
	</Command.List>
</Command.Dialog>
