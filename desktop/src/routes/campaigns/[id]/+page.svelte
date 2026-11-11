<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import CopyIcon from '@lucide/svelte/icons/copy';

	import BackLink from '$lib/components/app/BackLink.svelte';
	import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte';
	import FilterTabs from '$lib/components/app/FilterTabs.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import SearchInput from '$lib/components/app/SearchInput.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { CampaignStatusDto } from '$lib/ipc/bindings';
	import { commands } from '$lib/ipc/bindings';
	import { call, isIpcError } from '$lib/ipc/client';
	import { campaignQueries, sendBatchQueries } from '$lib/ipc/queries';
	import CampaignFormDialog from '$lib/screens/campaigns/CampaignFormDialog.svelte';
	import PrepareSendDialog from '$lib/screens/campaigns/PrepareSendDialog.svelte';
	import { toast } from '$lib/stores/toasts.svelte';
	import { reportError } from '$lib/utils/errors';
	import { formatDate, formatDateTime, formatNumber } from '$lib/utils/format';
	import { PAGE_SIZE, listHref, offsetFor, pageParam, param } from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';

	const id = $derived(Number(page.params.id));
	const detail = createQuery(() => campaignQueries.detail(id));
	const stats = createQuery(() => campaignQueries.stats(id));
	const campaign = $derived(detail.data);

	const tabs = [
		{ value: 'unsent', label: 'Not sent' },
		{ value: 'sent', label: 'Sent' },
		{ value: 'all', label: 'All' },
		{ value: 'history', label: 'History' }
	] as const;
	const tab = $derived(tabs.find((t) => t.value === param(page.url, 'tab'))?.value ?? 'unsent');
	const search = $derived(param(page.url, 'search'));
	const offset = $derived(offsetFor(pageParam(page.url)));

	const audience = createQuery(() => ({
		...campaignQueries.audience(id, {
			filter: tab === 'history' ? 'all' : tab,
			search,
			limit: PAGE_SIZE,
			offset
		}),
		enabled: tab !== 'history',
		placeholderData: keepPreviousData
	}));
	const batches = createQuery(() => ({
		...sendBatchQueries.list({ campaignId: id, limit: PAGE_SIZE, offset }),
		enabled: tab === 'history'
	}));

	let editOpen = $state(false);
	let sendOpen = $state(false);
	let archiveOpen = $state(false);

	async function setStatus(status: CampaignStatusDto) {
		if (!campaign) return;
		try {
			await call(
				commands.campaignsSetStatus({ id: campaign.id, version: campaign.version, status })
			);
			toast.success(status === 'archived' ? 'Archived' : 'Activated');
		} catch (err) {
			reportError(err);
			throw err;
		}
	}

	function onSearch(value: string) {
		void go(listHref(page.url, { search: value }), { replace: true, reset: false });
	}
</script>

<div class="flex flex-col gap-4">
	<BackLink to="/campaigns" label="Campaigns" />

	{#if detail.isError}
		<p class="text-sm text-destructive">
			{isIpcError(detail.error) && detail.error.code === 'NOT_FOUND'
				? 'This campaign no longer exists.'
				: `Could not load this campaign: ${detail.error.message}`}
		</p>
	{:else if campaign}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div class="flex items-center gap-3">
				<h1 class="text-xl font-semibold">{campaign.name}</h1>
				<StatusBadge status={campaign.status} />
			</div>
			<div class="flex gap-2">
				<Button variant="outline" onclick={() => (editOpen = true)}>Edit</Button>
				{#if campaign.status === 'active'}
					<Button variant="outline" onclick={() => (archiveOpen = true)}>Archive</Button>
				{:else}
					<Button variant="outline" onclick={() => setStatus('active').catch(() => {})}>
						{campaign.status === 'archived' ? 'Unarchive' : 'Activate'}
					</Button>
				{/if}
				<Button onclick={() => (sendOpen = true)}><CopyIcon />Prepare send</Button>
			</div>
		</div>
		{#if campaign.description}
			<p class="text-sm whitespace-pre-line text-muted-foreground">{campaign.description}</p>
		{/if}

		<dl class="grid grid-cols-2 gap-3 sm:grid-cols-4">
			{#each [{ label: 'Audience', value: stats.data ? formatNumber(stats.data.total) : '…' }, { label: 'Sent', value: stats.data ? formatNumber(stats.data.sent) : '…' }, { label: 'Not sent', value: stats.data ? formatNumber(stats.data.unsent) : '…' }, { label: 'Last send', value: formatDate(stats.data?.lastBatchAt) }] as stat (stat.label)}
				<div class="border-border-subtle rounded-lg border bg-surface-raised p-3">
					<dt class="text-xs text-muted-foreground">{stat.label}</dt>
					<dd class="text-lg font-semibold tabular-nums">{stat.value}</dd>
				</div>
			{/each}
		</dl>

		<div class="flex flex-wrap items-center gap-3">
			<FilterTabs
				label="Audience"
				items={tabs.map((t) => ({
					label: t.label,
					href: href(
						listHref(page.url, { tab: t.value === 'unsent' ? null : t.value, search: null })
					),
					active: t.value === tab
				}))}
			/>
			{#if tab !== 'history'}
				<SearchInput value={search ?? ''} placeholder="Search name or email" {onSearch} />
			{/if}
		</div>

		{#if tab === 'history'}
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head>Sent</Table.Head>
						<Table.Head>Recipients</Table.Head>
						<Table.Head>Note</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each batches.data?.items ?? [] as batch (batch.id)}
						<Table.Row class="cursor-pointer" onclick={() => go(`/history/${batch.id}`)}>
							<Table.Cell>
								<a href={href(`/history/${batch.id}`)} class="hover:underline">
									{formatDateTime(batch.createdAt)}
								</a>
							</Table.Cell>
							<Table.Cell>{formatNumber(batch.recipientCount)}</Table.Cell>
							<Table.Cell class="max-w-sm truncate text-muted-foreground">{batch.note}</Table.Cell>
						</Table.Row>
					{:else}
						<Table.Row>
							<Table.Cell colspan={3} class="py-8 text-center text-muted-foreground">
								{batches.isPending ? 'Loading…' : 'Nothing has been sent yet.'}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
			{#if batches.data}
				<Pager total={batches.data.total} offset={batches.data.offset} />
			{/if}
		{:else}
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head>Name</Table.Head>
						<Table.Head>Email</Table.Head>
						<Table.Head class="text-right">Sent</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each audience.data?.items ?? [] as member, index (`${member.contactId}:${member.batchId}:${index}`)}
						<Table.Row>
							<Table.Cell class="font-medium">
								{#if member.contactId}
									<a href={href(`/freshies/${member.contactId}`)} class="hover:underline">
										{member.lastName}, {member.firstName}
									</a>
								{:else}
									{member.lastName}, {member.firstName}
								{/if}
							</Table.Cell>
							<Table.Cell>{member.email}</Table.Cell>
							<Table.Cell class="text-right text-muted-foreground">
								{#if member.batchId}
									<a href={href(`/history/${member.batchId}`)} class="hover:underline">
										{formatDate(member.sentAt)}
									</a>
								{:else}
									Not sent
								{/if}
							</Table.Cell>
						</Table.Row>
					{:else}
						<Table.Row>
							<Table.Cell colspan={3} class="py-8 text-center text-muted-foreground">
								{audience.isPending
									? 'Loading…'
									: tab === 'unsent' && !search
										? 'Everyone active has received this campaign.'
										: 'Nobody matches.'}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
			{#if audience.data}
				<Pager total={audience.data.total} offset={audience.data.offset} />
			{/if}
		{/if}

		<CampaignFormDialog bind:open={editOpen} {campaign} />
		<PrepareSendDialog bind:open={sendOpen} {campaign} />
		<ConfirmDialog
			bind:open={archiveOpen}
			title="Archive {campaign.name}?"
			description="Archived campaigns cannot be sent until you unarchive them. Their history is kept."
			confirmLabel="Archive"
			onConfirm={() => setStatus('archived')}
		/>
	{:else}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{/if}
</div>
