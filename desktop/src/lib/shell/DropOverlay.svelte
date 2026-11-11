<script lang="ts">
	import FileSpreadsheetIcon from '@lucide/svelte/icons/file-spreadsheet';
	import { onMount } from 'svelte';
	import { fade, scale } from 'svelte/transition';

	import { onFileDrag } from '$lib/ipc/dragdrop';
	import { ms } from '$lib/utils/motion';

	let dragging = $state(false);

	onMount(() =>
		onFileDrag((phase) => {
			dragging = phase === 'enter';
		})
	);
</script>

{#if dragging}
	<div
		class="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-background/70 p-6 backdrop-blur-sm"
		transition:fade={{ duration: ms(150) }}
		aria-hidden="true"
	>
		<div
			class="flex size-full flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed border-primary/60 bg-surface-raised/60"
			transition:scale={{ start: 0.96, duration: ms(220) }}
		>
			<FileSpreadsheetIcon class="size-12 animate-float text-primary" />
			<p class="text-lg font-semibold">Drop to import</p>
			<p class="text-sm text-muted-foreground">.xlsx, .xls, .ods or .csv</p>
		</div>
	</div>
{/if}
