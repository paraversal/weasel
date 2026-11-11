<script lang="ts">
	import MailXIcon from '@lucide/svelte/icons/mail-x';

	import { Button } from '$lib/components/ui/button/index.js';
	import type { EmailHoldDto } from '$lib/ipc/bindings';
	import { useTerm } from '$lib/stores/term.svelte';
	import { href } from '$lib/utils/paths';

	import { markEmail } from './actions';
	import { holdLabel } from './labels';

	let { email, hold }: { email: string; hold: EmailHoldDto } = $props();
	const term = useTerm();
	let busy = $state(false);

	async function release() {
		busy = true;
		await markEmail(email, 'valid');
		busy = false;
	}
</script>

<div
	class="flex animate-rise flex-wrap items-start gap-3 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-sm"
	role="status"
>
	<MailXIcon class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400" aria-hidden="true" />
	<div class="flex min-w-0 flex-1 flex-col gap-1">
		<p class="font-medium">{email} is on hold</p>
		<p class="text-muted-foreground">
			{holdLabel(hold)}. This {term.one} is left out of every send until the address changes. Fix it here,
			or import a list with the corrected address.
			{#if hold.kind === 'rule'}
				Rules live in <a class="underline" href={href('/settings')}>Settings</a>.
			{/if}
		</p>
	</div>
	<Button size="sm" variant="outline" disabled={busy} onclick={release}>Mark valid</Button>
</div>
