<script lang="ts" module>
	export type WeaselMood = 'calm' | 'hungry' | 'chomp' | 'picky';
</script>

<script lang="ts">
	/**
	 * The mascot's head. `look` moves the eyes (each axis -1…1); `gape` opens the hungry
	 * mouth (0…1). Colours are fixed: the weasel is brown in both themes. The same
	 * drawing is the favicon and the app icon (`src-tauri/icons/source/weasel.svg`).
	 */
	let {
		mood = 'calm',
		look = { x: 0, y: 0 },
		gape = 1,
		class: className = '',
		title
	}: {
		mood?: WeaselMood;
		look?: { x: number; y: number };
		gape?: number;
		class?: string;
		title?: string;
	} = $props();

	const clamp = (value: number) => Math.max(-1, Math.min(1, value));
	const eyeX = $derived(clamp(look.x) * 3);
	const eyeY = $derived(clamp(look.y) * 3);
	const g = $derived(Math.max(0, Math.min(1, gape)));
	// Hungry mouth: a wide open smile (flat top under the nose, round bottom).
	const mouthW = $derived(10 + 8 * g);
	const mouthH = $derived(10 + 16 * g);
	const mouth = $derived(
		`M${-mouthW} 25 C${-mouthW} ${25 + mouthH * 1.1} ${mouthW} ${25 + mouthH * 1.1} ${mouthW} 25 Q0 29 ${-mouthW} 25Z`
	);
	const clipId = $props.id();

	const brown = '#9a5b2e';
	const ink = '#3b2418';
	const eye = '#24150d';
</script>

<svg
	viewBox="-100 -100 200 200"
	class={className}
	role={title ? 'img' : undefined}
	aria-hidden={title ? undefined : 'true'}
	aria-label={title}
>
	<circle cx="-50" cy="-50" r="16" fill={brown} />
	<circle cx="-50" cy="-50" r="8" fill="#e8b4a0" />
	<circle cx="50" cy="-50" r="16" fill={brown} />
	<circle cx="50" cy="-50" r="8" fill="#e8b4a0" />
	<path
		d="M0 -70 C45 -70 70 -45 66 -18 C62 12 30 50 0 62 C-30 50 -62 12 -66 -18 C-70 -45 -45 -70 0 -70Z"
		fill={brown}
	/>
	<path
		d="M-56 -2 C-38 12 -14 10 0 8 C14 10 38 12 56 -2 C48 24 25 49 0 56 C-25 49 -48 24 -56 -2Z"
		fill="#f6e7cf"
	/>

	{#if mood === 'chomp'}
		<!-- Happy, squeezed-shut eyes and blushing cheeks. -->
		<path
			d="M-34 -20 Q-25 -31 -16 -20 M16 -20 Q25 -31 34 -20"
			stroke={eye}
			stroke-width="4.5"
			fill="none"
			stroke-linecap="round"
		/>
		<circle cx="-34" cy="16" r="8" fill="#f2b8a6" />
		<circle cx="34" cy="16" r="8" fill="#f2b8a6" />
	{:else}
		{#if mood === 'hungry'}
			<ellipse cx="-36" cy="14" rx="9" ry="6" fill="#f0a593" opacity="0.7" />
			<ellipse cx="36" cy="14" rx="9" ry="6" fill="#f0a593" opacity="0.7" />
		{/if}
		<g style:transform="translate({eyeX}px, {eyeY}px)">
			{#if mood === 'hungry'}
				<!-- Bigger, sparkly eyes: excited, not menacing. -->
				<ellipse cx="-25" cy="-22" rx="9.5" ry="10.5" fill={eye} />
				<circle cx="-21.5" cy="-26" r="3.6" fill="#fff" />
				<circle cx="-28" cy="-17.5" r="1.6" fill="#fff" />
				<ellipse cx="25" cy="-22" rx="9.5" ry="10.5" fill={eye} />
				<circle cx="28.5" cy="-26" r="3.6" fill="#fff" />
				<circle cx="22" cy="-17.5" r="1.6" fill="#fff" />
			{:else}
				<ellipse cx="-25" cy="-22" rx="8.5" ry="9.5" fill={eye} />
				<circle cx="-22" cy="-25.5" r="2.7" fill="#fff" />
				<ellipse cx="25" cy="-22" rx="8.5" ry="9.5" fill={eye} />
				<circle cx="28" cy="-25.5" r="2.7" fill="#fff" />
			{/if}
		</g>
		{#if mood === 'picky'}
			<!-- Lowered lids: unimpressed. -->
			<path d="M-36 -34 H-14 V-23 H-36Z M14 -34 H36 V-23 H14Z" fill={brown} />
		{/if}
	{/if}

	{#if mood === 'hungry'}
		<clipPath id={clipId}><path d={mouth} /></clipPath>
		<path d={mouth} fill="#7a2f3b" />
		<ellipse
			cx="0"
			cy={25 + mouthH * 0.85}
			rx={mouthW * 0.7}
			ry={mouthH * 0.35}
			fill="#f08c9a"
			clip-path="url(#{clipId})"
		/>
	{:else if mood === 'picky'}
		<path
			d="M0 25 L0 31 M-9 35 L9 33"
			stroke={ink}
			stroke-width="3"
			fill="none"
			stroke-linecap="round"
		/>
	{:else}
		<path
			d="M0 25 L0 31 M-11 30 Q-5 37 0 31 Q5 37 11 30"
			stroke={ink}
			stroke-width="3"
			fill="none"
			stroke-linecap="round"
		/>
	{/if}

	<path d="M-9 14 Q0 9 9 14 Q4 24 0 25 Q-4 24 -9 14Z" fill={ink} />
	<path
		d="M-22 21 L-64 12 M-22 27 L-64 33 M22 21 L64 12 M22 27 L64 33"
		stroke={ink}
		stroke-width="1.6"
		stroke-linecap="round"
		opacity="0.55"
	/>
</svg>
