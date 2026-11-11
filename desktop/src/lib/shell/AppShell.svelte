<script lang="ts">
	/* Palette targets are runtime strings; resolvePath applies the app base for us. */
	/* eslint-disable svelte/no-navigation-without-resolve */
	import { goto } from '$app/navigation';

	import { resolvePath } from '$lib/utils/paths';
	import { ShortcutMatcher, detectPlatform, type ShortcutBinding } from '$lib/utils/shortcuts';

	import CommandPalette from './CommandPalette.svelte';
	import ErrorBoundary from './ErrorBoundary.svelte';
	import Sidebar from './Sidebar.svelte';
	import Toasts from './Toasts.svelte';
	import Topbar from './Topbar.svelte';
	import { NAV, PALETTE, type PaletteEntry } from './nav';
	import type { ThemeController } from '$lib/stores/theme.svelte';

	let {
		children,
		theme
	}: {
		children: import('svelte').Snippet;
		theme: ThemeController;
	} = $props();

	const platform = detectPlatform();
	let paletteOpen = $state(false);

	function run(entry: PaletteEntry) {
		void goto(resolvePath(entry.href));
	}

	const bindings = $derived<ShortcutBinding[]>([
		{ spec: 'mod+k', run: () => (paletteOpen = !paletteOpen) },
		{
			spec: '?',
			run: () => {
				if (!paletteOpen) void goto(resolvePath('/help'));
			}
		},
		...PALETTE.filter((entry) => entry.shortcut).map((entry) => ({
			spec: entry.shortcut ?? '',
			run: () => run(entry)
		}))
	]);

	// One global listener; re-created if the bindings change.
	$effect(() => {
		const matcher = new ShortcutMatcher(bindings, { platform });
		const onKeydown = (event: KeyboardEvent) => {
			if (matcher.handleKeydown(event)) event.preventDefault();
		};
		window.addEventListener('keydown', onKeydown);
		return () => window.removeEventListener('keydown', onKeydown);
	});
</script>

<div class="grid h-dvh grid-cols-[auto_minmax(0,1fr)] overflow-hidden bg-background">
	<Sidebar items={NAV} />
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

<CommandPalette bind:open={paletteOpen} commands={PALETTE} {platform} onRun={run} />
<Toasts theme={theme.resolved} />
