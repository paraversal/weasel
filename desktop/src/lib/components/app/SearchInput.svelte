<script lang="ts">
	import SearchIcon from '@lucide/svelte/icons/search';
	import { onDestroy } from 'svelte';

	import { Input } from '$lib/components/ui/input/index.js';

	let {
		value = '',
		placeholder = 'Search…',
		onSearch
	}: { value?: string; placeholder?: string; onSearch: (value: string) => void } = $props();

	let text = $state('');
	let input = $state<HTMLInputElement | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	// Follow external changes (back/forward) unless the user is mid-typing.
	$effect(() => {
		if (document.activeElement !== input) text = value;
	});

	function schedule() {
		clearTimeout(timer);
		timer = setTimeout(() => onSearch(text.trim()), 250);
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && text) {
			event.stopPropagation();
			text = '';
			clearTimeout(timer);
			onSearch('');
		}
	}

	function onWindowKeydown(event: KeyboardEvent) {
		if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
			event.preventDefault();
			input?.focus();
			input?.select();
		}
	}

	onDestroy(() => clearTimeout(timer));
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="relative w-full max-w-xs">
	<SearchIcon
		class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
	/>
	<Input
		bind:ref={input}
		bind:value={text}
		oninput={schedule}
		onkeydown={onKeydown}
		type="search"
		{placeholder}
		aria-label={placeholder}
		class="pl-8"
	/>
</div>
