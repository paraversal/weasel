<script lang="ts">
	import { onMount } from 'svelte';
	import { Spring, prefersReducedMotion } from 'svelte/motion';
	import { fade } from 'svelte/transition';

	import Weasel, { type WeaselMood } from '$lib/components/app/Weasel.svelte';
	import { hasImportable, onFileDrag } from '$lib/ipc/dragdrop';
	import { ms } from '$lib/utils/motion';

	/**
	 * While a file is dragged over the window, the weasel chases the cursor with its
	 * mouth open, trailing a little behind it. Dropping a spreadsheet gets a chomp;
	 * anything else gets spat out. The actual import starts from the `sourceDropped`
	 * event, not from here.
	 */
	type Phase = 'idle' | 'hungry' | 'picky' | 'eating' | 'spitting';

	let phase = $state<Phase>('idle');
	let cursor = $state<{ x: number; y: number } | null>(null);
	let viewport = $state({ width: 1024, height: 768 });
	let size = $state(320);
	let timer: ReturnType<typeof setTimeout> | undefined;

	// The head's centre. A soft, slightly underdamped spring gives the lag and the
	// little overshoot when the cursor stops; there are no CSS transitions to fight it.
	const head = new Spring({ x: 512, y: 384 }, { stiffness: 0.045, damping: 0.42 });

	/** The mouth sits below the centre of the drawing (about y = 35 in a 200-unit box). */
	const mouthOffset = $derived(size * 0.18);

	function target(point: { x: number; y: number }) {
		// Put the open mouth just under the dragged file, kept inside the window.
		const half = size / 2;
		return {
			x: Math.min(viewport.width - half, Math.max(half, point.x)),
			y: Math.min(viewport.height - half, Math.max(half, point.y + 24 - mouthOffset))
		};
	}

	function follow(point: { x: number; y: number }) {
		cursor = point;
		void head.set(target(point), { instant: prefersReducedMotion.current });
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
		const resize = () => {
			viewport = { width: window.innerWidth, height: window.innerHeight };
		};
		resize();
		const stop = onFileDrag((drag) => {
			const point = drag.x !== null && drag.y !== null ? { x: drag.x, y: drag.y } : null;
			if (drag.phase === 'enter') {
				clearTimeout(timer);
				resize();
				// Pop up in the middle, then go after the file.
				void head.set({ x: viewport.width / 2, y: viewport.height / 2 }, { instant: true });
				phase = hasImportable(drag.paths) ? 'hungry' : 'picky';
				if (point) follow(point);
			} else if (drag.phase === 'over') {
				if (point && phase !== 'eating' && phase !== 'spitting') follow(point);
			} else if (drag.phase === 'leave') {
				phase = 'idle';
				cursor = null;
			} else if (drag.phase === 'drop') {
				if (point) follow(point);
				if (hasImportable(drag.paths)) settle('eating', ms(1000) || 300);
				else settle('spitting', ms(1000) || 300);
			}
		});
		window.addEventListener('resize', resize);
		return () => {
			stop();
			clearTimeout(timer);
			window.removeEventListener('resize', resize);
		};
	});

	const pos = $derived(head.current);

	// Everything below reads the smoothed position, so it moves as smoothly as the head.
	const look = $derived.by(() => {
		if (!cursor) return { x: 0, y: 0 };
		const dx = cursor.x - pos.x;
		const dy = cursor.y - pos.y;
		const distance = Math.hypot(dx, dy) || 1;
		const reach = Math.min(1, distance / (size * 0.6));
		return { x: (dx / distance) * reach, y: (dy / distance) * reach };
	});

	/** Wide open once it has nearly caught the file; smaller while it is still chasing. */
	const gape = $derived.by(() => {
		if (!cursor) return 0.7;
		const goal = target(cursor);
		const lag = Math.hypot(goal.x - pos.x, goal.y - pos.y);
		return 1 - 0.55 * Math.min(1, lag / 240);
	});

	/** Lean into the direction of travel. */
	const tilt = $derived.by(() => {
		if (prefersReducedMotion.current || !cursor) return 0;
		const goal = target(cursor);
		return Math.max(-16, Math.min(16, (goal.x - pos.x) / 14));
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
		class="pointer-events-none fixed inset-0 z-50 overflow-hidden bg-background/70 backdrop-blur-sm"
		transition:fade={{ duration: ms(150) }}
		aria-hidden="true"
	>
		<div
			class={[
				'absolute inset-6 rounded-2xl border-2 border-dashed transition-colors',
				phase === 'picky' || phase === 'spitting'
					? 'border-muted-foreground/40 bg-muted/40'
					: 'border-primary/60 bg-surface-raised/40'
			]}
		></div>

		<div
			class="absolute top-0 left-0 will-change-transform"
			style:width="{size}px"
			style:height="{size}px"
			style:transform="translate({pos.x - size / 2}px, {pos.y - size / 2}px) rotate({tilt}deg)"
		>
			<div
				bind:clientWidth={size}
				class={[
					'size-[min(52vmin,440px)] min-h-56 min-w-56',
					phase === 'eating' && 'animate-chomp',
					phase === 'spitting' && 'animate-shake'
				]}
			>
				<Weasel {mood} {look} {gape} class="size-full drop-shadow-2xl" />
			</div>
		</div>

		<div class="absolute inset-x-0 bottom-12 flex flex-col items-center gap-1">
			{#key phase}
				<p
					class={[
						'animate-pop font-semibold',
						phase === 'eating' ? 'text-5xl tracking-wide text-primary' : 'text-xl'
					]}
				>
					{caption}
				</p>
			{/key}
			<p class="text-sm text-muted-foreground">.xlsx, .xls, .ods or .csv</p>
		</div>
	</div>
{/if}
