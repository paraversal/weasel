<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import CopyIcon from '@lucide/svelte/icons/copy';

	import BackLink from '$lib/components/app/BackLink.svelte';
	import CountUp from '$lib/components/app/CountUp.svelte';
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
	import { setBounced } from '$lib/screens/email-checks/actions';
	import EmailHoldBadge from '$lib/screens/email-checks/EmailHoldBadge.svelte';
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
			{#each [{ label: 'Audience', value: stats.data?.total, extra: null }, { label: 'Sent', value: stats.data?.sent, extra: stats.data?.bounced ? `${formatNumber(stats.data.bounced)} bounced` : null }, { label: 'Not sent', value: stats.data?.unsent, extra: stats.data?.onHold ? `${formatNumber(stats.data.onHold)} more on hold` : null }] as stat, index (stat.label)}
				<div
					class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-3"
					style="--i: {index * 2}"
				>
					<dt class="text-xs text-muted-foreground">{stat.label}</dt>
					<dd class="text-lg font-semibold">
						{#if stat.value === undefined}…{:else}<CountUp value={stat.value} />{/if}
					</dd>
					{#if stat.extra}
						<dd class="animate-pop text-xs text-amber-700 dark:text-amber-300">{stat.extra}</dd>
					{/if}
				</div>
			{/each}
			<div
				class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-3"
				style="--i: 6"
			>
				<dt class="text-xs text-muted-foreground">Last send</dt>
				<dd class="text-lg font-semibold">{formatDate(stats.data?.lastBatchAt)}</dd>
			</div>
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
					{#each batches.data?.items ?? [] as batch, index (batch.id)}
						<Table.Row
							class="stagger animate-row-in cursor-pointer"
							style="--i: {index}"
							onclick={() => go(`/history/${batch.id}`)}
						>
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
						{#if tab !== 'unsent'}<Table.Head class="w-0"
								><span class="sr-only">Bounce</span></Table.Head
							>{/if}
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each audience.data?.items ?? [] as member, index (`${member.contactId}:${member.batchId}:${index}`)}
						<Table.Row class="stagger animate-row-in" style="--i: {index}">
							<Table.Cell class="font-medium">
								{#if member.contactId}
									<a href={href(`/contacts/${member.contactId}`)} class="hover:underline">
										{member.lastName}, {member.firstName}
									</a>
								{:else}
									{member.lastName}, {member.firstName}
								{/if}
							</Table.Cell>
							<Table.Cell>
								<span class="inline-flex flex-wrap items-center gap-2">
									<span
										class={member.bounced || member.onHold
											? 'text-muted-foreground line-through'
											: ''}>{member.email}</span
									>
									{#if member.bounced}
										<EmailHoldBadge hold={{ kind: 'bounced' }} label="Bounced" />
									{:else if member.onHold}
										<EmailHoldBadge label="On hold" />
									{/if}
								</span>
							</Table.Cell>
							<Table.Cell class="text-right text-muted-foreground">
								{#if member.batchId}
									<a href={href(`/history/${member.batchId}`)} class="hover:underline">
										{formatDate(member.sentAt)}
									</a>
								{:else}
									Not sent
								{/if}
							</Table.Cell>
							{#if tab !== 'unsent'}
								<Table.Cell class="text-right">
									{#if member.recordId !== null}
										{@const recordId = member.recordId}
										<Button
											size="sm"
											variant="ghost"
											class="text-muted-foreground"
											onclick={() => setBounced(recordId, !member.bounced)}
										>
											{member.bounced ? 'Undo bounce' : 'Bounced?'}
										</Button>
									{/if}
								</Table.Cell>
							{/if}
						</Table.Row>
					{:else}
						<Table.Row>
							<Table.Cell
								colspan={tab === 'unsent' ? 3 : 4}
								class="py-8 text-center text-muted-foreground"
							>
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
