<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call } from '$lib/ipc/client';
	import { logError } from '$lib/ipc/log';
	import { toast } from '$lib/stores/toasts.svelte';

	let { children } = $props();

	async function openDataFolder() {
		try {
			await call(commands.appOpenDataDir());
		} catch (error) {
			toast.ipcError(error);
		}
	}
</script>

<!-- A render error in one pane must not take down the sidebar, palette or toasts. -->
<svelte:boundary
	onerror={(error) => {
		logError('render boundary caught an error', error);
	}}
>
	{@render children()}

	{#snippet failed(error, reset)}
		<div class="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
			<h2 class="text-lg font-semibold">This screen failed to render</h2>
			<p class="max-w-md text-sm text-muted-foreground">
				{error instanceof Error ? error.message : String(error)}
			</p>
			<div class="flex gap-2">
				<Button onclick={reset}>Try again</Button>
				<Button variant="outline" onclick={openDataFolder}>Open data folder</Button>
			</div>
		</div>
	{/snippet}
</svelte:boundary>
