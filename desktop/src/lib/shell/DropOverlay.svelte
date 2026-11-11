<script lang="ts">
	import { onMount } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, scale } from 'svelte/transition';

	import Weasel, { type WeaselMood } from '$lib/components/app/Weasel.svelte';
	import { hasImportable, onFileDrag } from '$lib/ipc/dragdrop';
	import { ms } from '$lib/utils/motion';

	/**
	 * While a file is dragged over the window, the weasel opens wide and follows the
	 * cursor. Dropping a spreadsheet gets a chomp; anything else gets spat out. The
	 * actual import starts from the `sourceDropped` event, not from here.
	 */
	type Phase = 'idle' | 'hungry' | 'picky' | 'eating' | 'spitting';

	let phase = $state<Phase>('idle');
	let cursor = $state<{ x: number; y: number } | null>(null);
	let head = $state<HTMLDivElement | null>(null);
	let center = $state({ x: 0, y: 0 });
	let timer: ReturnType<typeof setTimeout> | undefined;

	function measure() {
		if (!head) return;
		const rect = head.getBoundingClientRect();
		center = { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
	}

	function settle(next: Phase, after: number) {
		clearTimeout(timer);
		phase = next;
		timer = setTimeout(() => {
			phase = 'idle';
			cursor = null;
		}, after);
	}

	onMount(() => {
		const stop = onFileDrag((drag) => {
			if (drag.x !== null && drag.y !== null) cursor = { x: drag.x, y: drag.y };
			if (drag.phase === 'enter') {
				clearTimeout(timer);
				phase = hasImportable(drag.paths) ? 'hungry' : 'picky';
				requestAnimationFrame(measure);
			} else if (drag.phase === 'leave') {
				phase = 'idle';
				cursor = null;
			} else if (drag.phase === 'drop') {
				if (hasImportable(drag.paths)) settle('eating', ms(900) || 300);
				else settle('spitting', ms(900) || 300);
			}
		});
		window.addEventListener('resize', measure);
		return () => {
			stop();
			clearTimeout(timer);
			window.removeEventListener('resize', measure);
		};
	});

	// Direction and distance from the weasel's nose to the cursor.
	const aim = $derived.by(() => {
		if (!cursor) return { x: 0, y: 0, near: 0, dx: 0 };
		const dx = cursor.x - center.x;
		const dy = cursor.y - center.y;
		const distance = Math.hypot(dx, dy) || 1;
		const reach = Math.min(1, distance / 220);
		return {
			x: (dx / distance) * reach,
			y: (dy / distance) * reach,
			near: 1 - Math.min(1, distance / 520),
			dx
		};
	});

	const mood = $derived<WeaselMood>(
		phase === 'eating'
			? 'chomp'
			: phase === 'hungry'
				? 'hungry'
				: phase === 'idle'
					? 'calm'
					: 'picky'
	);
	const lean = $derived.by(() => {
		if (prefersReducedMotion.current || phase !== 'hungry') return '';
		const tilt = Math.max(-12, Math.min(12, aim.dx / 24));
		return `translate(${aim.x * 18}px, ${aim.y * 14}px) rotate(${tilt}deg)`;
	});

	const caption = $derived(
		{
			idle: '',
			hungry: 'Feed me a spreadsheet',
			picky: 'I only eat spreadsheets',
			eating: 'YUM!',
			spitting: 'Bleh.'
		}[phase]
	);
</script>

{#if phase !== 'idle'}
	<div
		class="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-background/70 p-6 backdrop-blur-sm"
		transition:fade={{ duration: ms(150) }}
		aria-hidden="true"
	>
		<div
			class={[
				'flex size-full flex-col items-center justify-center gap-4 rounded-2xl border-2 border-dashed transition-colors',
				phase === 'picky' || phase === 'spitting'
					? 'border-muted-foreground/40 bg-muted/40'
					: 'border-primary/60 bg-surface-raised/60'
			]}
			transition:scale={{ start: 0.96, duration: ms(220) }}
		>
			<div
				bind:this={head}
				class={[
					'size-56 transition-transform duration-150 ease-out',
					phase === 'eating' && 'animate-chomp',
					phase === 'spitting' && 'animate-shake',
					phase === 'hungry' && !cursor && 'animate-bob'
				]}
				style:transform={lean}
			>
				<Weasel
					{mood}
					look={{ x: aim.x, y: aim.y }}
					gape={0.45 + 0.55 * aim.near}
					class="size-full drop-shadow-xl"
				/>
			</div>
			{#key phase}
				<p
					class={[
						'animate-pop font-semibold',
						phase === 'eating' ? 'text-4xl tracking-wide text-primary' : 'text-lg'
					]}
				>
					{caption}
				</p>
			{/key}
			<p class="text-sm text-muted-foreground">.xlsx, .xls, .ods or .csv</p>
		</div>
	</div>
{/if}
