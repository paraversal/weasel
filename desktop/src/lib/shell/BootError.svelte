<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call } from '$lib/ipc/client';
	import { toast } from '$lib/stores/toasts.svelte';

	let { message, dataDir }: { message: string; dataDir?: string } = $props();

	async function openDataFolder() {
		try {
			await call(commands.appOpenDataDir());
		} catch (error) {
			toast.ipcError(error);
		}
	}
</script>

<main class="flex h-dvh flex-col items-center justify-center gap-4 p-8 text-center">
	<h1 class="text-xl font-semibold">Weasel could not open its database</h1>
	<p class="max-w-lg text-sm break-words text-muted-foreground">{message}</p>
	{#if dataDir}
		<p class="max-w-lg font-mono text-xs break-all text-muted-foreground">Data folder: {dataDir}</p>
	{/if}
	<div class="flex gap-2">
		<Button onclick={openDataFolder}>Open data folder</Button>
		<Button variant="outline" onclick={() => location.reload()}>Retry</Button>
	</div>
</main>
