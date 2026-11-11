<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import PlusIcon from '@lucide/svelte/icons/plus';

	import EmptyState from '$lib/components/app/EmptyState.svelte';
	import FilterTabs from '$lib/components/app/FilterTabs.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import Progress from '$lib/components/app/Progress.svelte';
	import SearchInput from '$lib/components/app/SearchInput.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { campaignQueries } from '$lib/ipc/queries';
	import CampaignFormDialog from '$lib/screens/campaigns/CampaignFormDialog.svelte';
	import { formatDate } from '$lib/utils/format';
	import { PAGE_SIZE, listHref, offsetFor, pageParam, param } from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';

	const tabs = [
		{ value: 'all', label: 'All' },
		{ value: 'active', label: 'Active' },
		{ value: 'draft', label: 'Draft' },
		{ value: 'archived', label: 'Archived' }
	] as const;

	const search = $derived(param(page.url, 'search'));
	const tab = $derived(tabs.find((t) => t.value === param(page.url, 'status'))?.value ?? 'all');
	const offset = $derived(offsetFor(pageParam(page.url)));

	const list = createQuery(() => ({
		...campaignQueries.list({
			search,
			status: tab === 'all' ? null : tab,
			limit: PAGE_SIZE,
			offset
		}),
		placeholderData: keepPreviousData
	}));

	let createOpen = $state(false);

	$effect(() => {
		if (param(page.url, 'new') === '1') {
			createOpen = true;
			void go(listHref(page.url, { new: null }), { replace: true, reset: false });
		}
	});

	function onSearch(value: string) {
		void go(listHref(page.url, { search: value }), { replace: true, reset: false });
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between gap-3">
		<h1 class="text-xl font-semibold">Campaigns</h1>
		<Button onclick={() => (createOpen = true)}><PlusIcon />New campaign</Button>
	</div>

	<div class="flex flex-wrap items-center gap-3">
		<SearchInput value={search ?? ''} placeholder="Search campaigns" {onSearch} />
		<FilterTabs
			label="Status"
			items={tabs.map((t) => ({
				label: t.label,
				href: href(listHref(page.url, { status: t.value === 'all' ? null : t.value })),
				active: t.value === tab
			}))}
		/>
	</div>

	{#if list.isError}
		<p class="text-sm text-destructive">Could not load campaigns: {list.error.message}</p>
	{:else if list.data && list.data.total === 0 && !search && tab === 'all'}
		<EmptyState title="No campaigns yet">
			<p>A campaign tracks who has received a mailing.</p>
			<Button onclick={() => (createOpen = true)}>New campaign</Button>
		</EmptyState>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>Name</Table.Head>
					<Table.Head>Status</Table.Head>
					<Table.Head>Sent</Table.Head>
					<Table.Head>Last send</Table.Head>
					<Table.Head class="text-right">Updated</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each list.data?.items ?? [] as { campaign, stats } (campaign.id)}
					<Table.Row class="cursor-pointer" onclick={() => go(`/campaigns/${campaign.id}`)}>
						<Table.Cell class="font-medium">
							<a href={href(`/campaigns/${campaign.id}`)} class="hover:underline">
								{campaign.name}
							</a>
						</Table.Cell>
						<Table.Cell><StatusBadge status={campaign.status} /></Table.Cell>
						<Table.Cell>
							<Progress value={stats.sent} max={stats.total} label="Sent of total" />
						</Table.Cell>
						<Table.Cell class="text-muted-foreground">{formatDate(stats.lastBatchAt)}</Table.Cell>
						<Table.Cell class="text-right text-muted-foreground">
							{formatDate(campaign.updatedAt)}
						</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={5} class="py-8 text-center text-muted-foreground">
							{list.isPending ? 'Loading…' : 'No campaigns match.'}
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

<CampaignFormDialog bind:open={createOpen} />
