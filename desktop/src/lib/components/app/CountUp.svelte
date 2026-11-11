<script lang="ts">
	import { cubicOut } from 'svelte/easing';
	import { Tween, prefersReducedMotion } from 'svelte/motion';

	import { formatNumber } from '$lib/utils/format';

	/** A number that rolls up to its value on first render and whenever it changes. */
	let { value, duration = 900 }: { value: number; duration?: number } = $props();

	const tween = new Tween(0, { easing: cubicOut });

	$effect(() => {
		void tween.set(value, { duration: prefersReducedMotion.current ? 0 : duration });
	});
</script>

<span class="tabular-nums">{formatNumber(Math.round(tween.current))}</span>
