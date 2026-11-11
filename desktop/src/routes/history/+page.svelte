<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import XIcon from '@lucide/svelte/icons/x';

	import EmptyState from '$lib/components/app/EmptyState.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import SearchInput from '$lib/components/app/SearchInput.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { campaignQueries, sendBatchQueries } from '$lib/ipc/queries';
	import { formatDateTime, formatNumber } from '$lib/utils/format';
	import {
		PAGE_SIZE,
		intParam,
		listHref,
		offsetFor,
		pageParam,
		param
	} from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';

	const search = $derived(param(page.url, 'search'));
	const campaignId = $derived(intParam(page.url, 'campaign'));
	const offset = $derived(offsetFor(pageParam(page.url)));

	const list = createQuery(() => ({
		...sendBatchQueries.list({ search, campaignId, limit: PAGE_SIZE, offset }),
		placeholderData: keepPreviousData
	}));
	const campaign = createQuery(() => ({
		...campaignQueries.detail(campaignId ?? 0),
		enabled: campaignId !== undefined
	}));

	function onSearch(value: string) {
		void go(listHref(page.url, { search: value }), { replace: true, reset: false });
	}
</script>

<div class="flex flex-col gap-4">
	<h1 class="text-xl font-semibold">Send history</h1>

	<div class="flex flex-wrap items-center gap-3">
		<SearchInput value={search ?? ''} placeholder="Search campaign, recipient or note" {onSearch} />
		{#if campaignId}
			<Badge variant="outline" href={href(listHref(page.url, { campaign: null }))}>
				{campaign.data?.name ?? `Campaign #${campaignId}`}
				<XIcon data-icon="inline-end" />
			</Badge>
		{/if}
	</div>

	{#if list.isError}
		<p class="text-sm text-destructive">Could not load the history: {list.error.message}</p>
	{:else if list.data && list.data.total === 0 && !search && !campaignId}
		<EmptyState title="Nothing sent yet">
			<p>Open a campaign and use “Prepare send” to copy its recipients.</p>
		</EmptyState>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>Sent</Table.Head>
					<Table.Head>Campaign</Table.Head>
					<Table.Head>Recipients</Table.Head>
					<Table.Head>Note</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each list.data?.items ?? [] as batch, index (batch.id)}
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
						<Table.Cell class="font-medium">{batch.campaignName}</Table.Cell>
						<Table.Cell>{formatNumber(batch.recipientCount)}</Table.Cell>
						<Table.Cell class="max-w-sm truncate text-muted-foreground">{batch.note}</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={4} class="py-8 text-center text-muted-foreground">
							{list.isPending ? 'Loading…' : 'No batches match.'}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
		{#if list.data}
			<Pager total={list.data.total} offset={list.data.offset} />
		{/if}
	{/if}
</div>
