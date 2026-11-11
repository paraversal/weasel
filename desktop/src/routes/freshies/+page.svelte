<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import PlusIcon from '@lucide/svelte/icons/plus';

	import EmptyState from '$lib/components/app/EmptyState.svelte';
	import FilterTabs from '$lib/components/app/FilterTabs.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import SearchInput from '$lib/components/app/SearchInput.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { ContactStatusDto } from '$lib/ipc/bindings';
	import { contactQueries } from '$lib/ipc/queries';
	import AddFreshieDialog from '$lib/screens/freshies/AddFreshieDialog.svelte';
	import { formatDate } from '$lib/utils/format';
	import { PAGE_SIZE, listHref, offsetFor, pageParam, param } from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';

	const tabs = [
		{ value: 'active', label: 'Active' },
		{ value: 'removed', label: 'Removed' },
		{ value: 'archived', label: 'Archived' },
		{ value: 'all', label: 'All' }
	] as const;
	type Tab = (typeof tabs)[number]['value'];

	const search = $derived(param(page.url, 'search'));
	const tab = $derived<Tab>(
		tabs.find((t) => t.value === param(page.url, 'status'))?.value ?? 'active'
	);
	const offset = $derived(offsetFor(pageParam(page.url)));
	const statuses = $derived<ContactStatusDto[]>(
		tab === 'all' ? ['active', 'removed', 'archived'] : [tab]
	);

	const list = createQuery(() => ({
		...contactQueries.list({ search, statuses, limit: PAGE_SIZE, offset }),
		placeholderData: keepPreviousData
	}));

	let addOpen = $state(false);

	// The palette opens the dialog with `?new=1`; consume the flag so reload does not reopen it.
	$effect(() => {
		if (param(page.url, 'new') === '1') {
			addOpen = true;
			void go(listHref(page.url, { new: null }), { replace: true, reset: false });
		}
	});

	function onSearch(value: string) {
		void go(listHref(page.url, { search: value }), { replace: true, reset: false });
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between gap-3">
		<h1 class="text-xl font-semibold">Freshies</h1>
		<Button onclick={() => (addOpen = true)}><PlusIcon />Add freshie</Button>
	</div>

	<div class="flex flex-wrap items-center gap-3">
		<SearchInput value={search ?? ''} placeholder="Search name or email" {onSearch} />
		<FilterTabs
			label="Status"
			items={tabs.map((t) => ({
				label: t.label,
				href: href(listHref(page.url, { status: t.value === 'active' ? null : t.value })),
				active: t.value === tab
			}))}
		/>
	</div>

	{#if list.isError}
		<p class="text-sm text-destructive">Could not load freshies: {list.error.message}</p>
	{:else if list.data && list.data.total === 0 && !search && tab === 'active'}
		<EmptyState title="No freshies yet">
			<p>Import a spreadsheet or add someone by hand.</p>
			<div class="flex gap-2">
				<Button href={href('/imports/new')}>Import list</Button>
				<Button variant="outline" onclick={() => (addOpen = true)}>Add freshie</Button>
			</div>
		</EmptyState>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>Name</Table.Head>
					<Table.Head>Email</Table.Head>
					<Table.Head>Status</Table.Head>
					<Table.Head>Source</Table.Head>
					<Table.Head class="text-right">Added</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each list.data?.items ?? [] as contact (contact.id)}
					<Table.Row class="cursor-pointer" onclick={() => go(`/freshies/${contact.id}`)}>
						<Table.Cell class="font-medium">
							<a href={href(`/freshies/${contact.id}`)} class="hover:underline">
								{contact.lastName}, {contact.firstName}
							</a>
						</Table.Cell>
						<Table.Cell>{contact.email}</Table.Cell>
						<Table.Cell><StatusBadge status={contact.status} /></Table.Cell>
						<Table.Cell class="text-muted-foreground capitalize">{contact.source}</Table.Cell>
						<Table.Cell class="text-right text-muted-foreground">
							{formatDate(contact.dateAdded)}
						</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={5} class="py-8 text-center text-muted-foreground">
							{list.isPending ? 'Loading…' : 'No freshies match.'}
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

<AddFreshieDialog bind:open={addOpen} />
