<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';

	import { href } from '$lib/utils/paths';

	/**
	 * Back to a list, keeping its search/filter/page when we came from it. The Tauri
	 * window has no browser back button, so this is the user's way back.
	 */
	let { to, label }: { to: string; label: string } = $props();

	let target = $state<string | null>(null);

	afterNavigate(({ from }) => {
		if (from?.url.pathname === to) target = from.url.pathname + from.url.search;
	});
</script>

<a
	href={href(target ?? to)}
	class="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
>
	<ArrowLeftIcon class="size-4" />{label}
</a>
