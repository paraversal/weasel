<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import type { CampaignDto } from '$lib/ipc/bindings';
	import { commands } from '$lib/ipc/bindings';
	import { call, isIpcError } from '$lib/ipc/client';
	import { campaignQueries } from '$lib/ipc/queries';
	import { toast } from '$lib/stores/toasts.svelte';
	import { fullName, plural } from '$lib/utils/format';
	import { go } from '$lib/utils/paths';

	let { open = $bindable(false), campaign }: { open?: boolean; campaign: CampaignDto } = $props();

	const stats = createQuery(() => campaignQueries.stats(campaign.id));
	const sample = createQuery(() => ({
		...campaignQueries.audience(campaign.id, { filter: 'unsent', limit: 10, offset: 0 }),
		enabled: open
	}));

	let pending = $state(false);
	let changed = $state(false);

	// The count we send as `expectedCount` must be the one on screen, so refresh on open.
	$effect(() => {
		if (open) {
			changed = false;
			void stats.refetch();
		}
	});

	const unsent = $derived(stats.data?.unsent ?? 0);
	const blocked = $derived(
		campaign.status !== 'active'
			? 'Only active campaigns can be sent. Activate it first.'
			: stats.data && unsent === 0
				? 'Everyone active has already received this campaign.'
				: null
	);

	async function send() {
		if (!stats.data || blocked) return;
		pending = true;
		try {
			const result = await call(
				commands.campaignsPrepareSend({ campaignId: campaign.id, expectedCount: unsent })
			);
			open = false;
			toast.success(`Copied ${plural(result.recipientCount, 'address', 'addresses')}`, {
				action: { label: 'View batch', onClick: () => void go(`/history/${result.batchId}`) }
			});
		} catch (err) {
			if (isIpcError(err) && err.code === 'CONFLICT') {
				changed = true;
				await Promise.all([stats.refetch(), sample.refetch()]);
			} else if (isIpcError(err) && err.retryable) {
				toast.ipcError(err, { action: { label: 'Retry', onClick: () => void send() } });
			} else {
				toast.ipcError(err);
			}
		} finally {
			pending = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Prepare send: {campaign.name}</Dialog.Title>
			<Dialog.Description>
				Copies the addresses of everyone who has not received this campaign yet and records them as
				a new batch in the send history.
			</Dialog.Description>
		</Dialog.Header>

		{#if changed}
			<p class="rounded-md bg-warning/10 px-3 py-2 text-sm" role="alert">
				The audience changed since you opened this. Check the new count and confirm again.
			</p>
		{/if}

		{#if stats.isPending}
			<p class="text-sm text-muted-foreground">Counting recipients…</p>
		{:else if blocked}
			<p class="text-sm text-muted-foreground">{blocked}</p>
		{:else}
			<p class="text-sm">
				<span class="font-semibold">{plural(unsent, 'recipient')}</span> will be copied.
			</p>
			{#if sample.data && sample.data.items.length > 0}
				<ul class="flex flex-col gap-1 text-sm text-muted-foreground">
					{#each sample.data.items as member (member.email)}
						<li>{fullName(member)} &lt;{member.email}&gt;</li>
					{/each}
					{#if unsent > sample.data.items.length}
						<li>…and {plural(unsent - sample.data.items.length, 'more', 'more')}</li>
					{/if}
				</ul>
			{/if}
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
			<Button onclick={send} disabled={pending || !!blocked || stats.isPending}>
				Copy {plural(unsent, 'address', 'addresses')}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
