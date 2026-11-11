<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { useQueryClient } from '@tanstack/svelte-query';

	import { createRegistry } from '$lib/features/registry.svelte';
	import type { CommandContext } from '$lib/features/types';
	import { toast } from '$lib/stores/toasts.svelte';
	import { ShortcutMatcher, detectPlatform, type ShortcutBinding } from '$lib/utils/shortcuts';

	import CommandPalette from './CommandPalette.svelte';
	import ErrorBoundary from './ErrorBoundary.svelte';
	import Sidebar from './Sidebar.svelte';
	import Toasts from './Toasts.svelte';
	import Topbar from './Topbar.svelte';
	import type { ThemeController } from '$lib/stores/theme.svelte';

	let {
		children,
		theme
	}: {
		children: import('svelte').Snippet;
		theme: ThemeController;
	} = $props();

	const registry = createRegistry();
	const queryClient = useQueryClient();
	const platform = detectPlatform();
	let paletteOpen = $state(false);

	const context: CommandContext = {
		goto: (path) => goto(resolve(path as Parameters<typeof resolve>[0])),
		queryClient,
		toast
	};

	function run(entry: {
		id: string;
		when?: () => boolean;
		run: (context: CommandContext) => unknown;
	}) {
		if (entry.when && !entry.when()) return;
		try {
			const result = entry.run(context);
			if (result instanceof Promise) result.catch((error: unknown) => toast.ipcError(error));
		} catch (error) {
			toast.ipcError(error);
		}
	}

	const bindings = $derived<ShortcutBinding[]>([
		{ spec: 'mod+k', run: () => (paletteOpen = !paletteOpen) },
		{
			spec: '?',
			run: () => {
				if (!paletteOpen) void goto(resolve('/help' as Parameters<typeof resolve>[0]));
			}
		},
		...registry.current.paletteCommands
			.filter((entry) => entry.shortcut)
			.map((entry) => ({ spec: entry.shortcut ?? '', run: () => run(entry) }))
	]);

	let reportedWarnings = '';

	// One global listener; re-created when the registry's shortcut set changes.
	$effect(() => {
		const matcher = new ShortcutMatcher(bindings, { platform });
		const onKeydown = (event: KeyboardEvent) => {
			if (matcher.handleKeydown(event)) event.preventDefault();
		};
		window.addEventListener('keydown', onKeydown);
		return () => window.removeEventListener('keydown', onKeydown);
	});

	// Startup diagnostics belong in the log, not in a toast the user cannot act on.
	$effect(() => {
		const signature = registry.current.warnings.map((w) => `${w.code}:${w.message}`).join('|');
		if (!signature || signature === reportedWarnings) return;
		reportedWarnings = signature;
		for (const warning of registry.current.warnings) {
			console.warn(`feature registry: ${warning.message}`);
		}
	});
</script>

<div class="grid h-dvh grid-cols-[auto_minmax(0,1fr)] overflow-hidden bg-background">
	<Sidebar items={registry.current.navItems} />
	<div class="flex min-h-0 min-w-0 flex-col">
		<Topbar onOpenPalette={() => (paletteOpen = true)} {theme} {platform} />
		<main class="min-h-0 flex-1 overflow-auto">
			<div class="mx-auto h-full max-w-6xl p-4">
				<ErrorBoundary>
					{@render children()}
				</ErrorBoundary>
			</div>
		</main>
	</div>
</div>

<CommandPalette
	bind:open={paletteOpen}
	commands={registry.current.paletteCommands}
	{platform}
	onRun={run}
/>
<Toasts theme={theme.resolved} />
