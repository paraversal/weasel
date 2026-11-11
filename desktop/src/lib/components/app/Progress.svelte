<script lang="ts">
	let { value, max, label }: { value: number; max: number; label: string } = $props();
	const percent = $derived(max > 0 ? Math.round((value / max) * 100) : 0);
</script>

<div class="flex items-center gap-2">
	<div
		class="h-1.5 w-24 overflow-hidden rounded-full bg-muted"
		role="progressbar"
		aria-label={label}
		aria-valuemin={0}
		aria-valuemax={max}
		aria-valuenow={value}
	>
		<div class="h-full rounded-full bg-primary" style:width="{percent}%"></div>
	</div>
	<span class="text-xs text-muted-foreground tabular-nums">{value}/{max}</span>
</div>
