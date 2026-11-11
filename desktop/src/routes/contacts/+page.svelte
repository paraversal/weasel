<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import { flip } from 'svelte/animate';
	import PlusIcon from '@lucide/svelte/icons/plus';

	import EmptyState from '$lib/components/app/EmptyState.svelte';
	import FilterTabs from '$lib/components/app/FilterTabs.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import SearchInput from '$lib/components/app/SearchInput.svelte';
	import SortHeader from '$lib/components/app/SortHeader.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { ContactSortDto, ContactStatusDto } from '$lib/ipc/bindings';
	import { contactQueries, importQueries } from '$lib/ipc/queries';
	import AddContactDialog from '$lib/screens/contacts/AddContactDialog.svelte';
	import { formatDate } from '$lib/utils/format';
	import { flipParams } from '$lib/utils/motion';
	import {
		PAGE_SIZE,
		intParam,
		listHref,
		offsetFor,
		pageParam,
		param
	} from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';
	import { useTerm } from '$lib/stores/term.svelte';

	const tabs = [
		{ value: 'active', label: 'Active' },
		{ value: 'removed', label: 'Removed' },
		{ value: 'archived', label: 'Archived' },
		{ value: 'all', label: 'All' }
	] as const;
	type Tab = (typeof tabs)[number]['value'];

	const search = $derived(param(page.url, 'search'));
	const term = useTerm();
	const tab = $derived<Tab>(
		tabs.find((t) => t.value === param(page.url, 'status'))?.value ?? 'active'
	);
	const offset = $derived(offsetFor(pageParam(page.url)));
	const statuses = $derived<ContactStatusDto[]>(
		tab === 'all' ? ['active', 'removed', 'archived'] : [tab]
	);

	const sorts: Record<ContactSortDto, 'asc' | 'desc'> = {
		lastNameAsc: 'asc',
		lastNameDesc: 'desc',
		emailAsc: 'asc',
		emailDesc: 'desc',
		addedDesc: 'desc',
		addedAsc: 'asc'
	};
	const sort = $derived.by((): ContactSortDto => {
		const raw = param(page.url, 'sort');
		return raw && raw in sorts ? (raw as ContactSortDto) : 'lastNameAsc';
	});
	const sortHref = (value: string) =>
		href(listHref(page.url, { sort: value === 'lastNameAsc' ? null : value }));

	const importBatchId = $derived(intParam(page.url, 'import'));

	const list = createQuery(() => ({
		...contactQueries.list({ search, statuses, importBatchId, sort, limit: PAGE_SIZE, offset }),
		placeholderData: keepPreviousData
	}));

	// Applied imports, newest first: names for the Import column and the import filter.
	const imports = createQuery(() => importQueries.list({ status: 'applied', limit: 500 }));
	const importNames = $derived(
		new Map((imports.data?.items ?? []).map((batch) => [batch.id, batch.sourceName]))
	);

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
		<h1 class="text-xl font-semibold">{term.Many}</h1>
		<Button onclick={() => (addOpen = true)}><PlusIcon />Add {term.one}</Button>
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
		{#if imports.data && imports.data.items.length > 0}
			<select
				aria-label="Import"
				class="h-8 max-w-56 rounded-lg border border-input bg-transparent px-2 text-sm dark:bg-input/30"
				value={importBatchId ?? ''}
				onchange={(event) =>
					go(listHref(page.url, { import: event.currentTarget.value }), { reset: false })}
			>
				<option value="">All imports</option>
				{#each imports.data.items as batch (batch.id)}
					<option value={batch.id}>{batch.sourceName} · {formatDate(batch.appliedAt)}</option>
				{/each}
			</select>
		{/if}
	</div>

	{#if list.isError}
		<p class="text-sm text-destructive">Could not load {term.many}: {list.error.message}</p>
	{:else if list.data && list.data.total === 0 && !search && !importBatchId && tab === 'active'}
		<EmptyState title="No {term.many} yet">
			<p>Import a spreadsheet or add someone by hand.</p>
			<div class="flex gap-2">
				<Button href={href('/imports/new')}>Import list</Button>
				<Button variant="outline" onclick={() => (addOpen = true)}>Add {term.one}</Button>
			</div>
		</EmptyState>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<SortHeader
						label="Name"
						sorts={['lastNameAsc', 'lastNameDesc']}
						current={sort}
						direction={sorts}
						hrefFor={sortHref}
					/>
					<SortHeader
						label="Email"
						sorts={['emailAsc', 'emailDesc']}
						current={sort}
						direction={sorts}
						hrefFor={sortHref}
					/>
					<Table.Head>Status</Table.Head>
					<Table.Head>Import</Table.Head>
					<SortHeader
						label="Added"
						sorts={['addedDesc', 'addedAsc']}
						current={sort}
						direction={sorts}
						hrefFor={sortHref}
						class="text-right"
					/>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each list.data?.items ?? [] as contact, index (contact.id)}
					<tr
						animate:flip={flipParams()}
						class="stagger animate-row-in cursor-pointer border-b transition-colors hover:bg-muted/50"
						style:--i={index}
						onclick={() => go(`/contacts/${contact.id}`)}
					>
						<Table.Cell class="font-medium">
							<a href={href(`/contacts/${contact.id}`)} class="hover:underline">
								{contact.lastName}, {contact.firstName}
							</a>
						</Table.Cell>
						<Table.Cell>{contact.email}</Table.Cell>
						<Table.Cell><StatusBadge status={contact.status} /></Table.Cell>
						<Table.Cell class="max-w-48 truncate text-muted-foreground">
							{#if contact.importBatchId}
								<a
									href={href(`/imports/${contact.importBatchId}`)}
									class="hover:underline"
									onclick={(event: MouseEvent) => event.stopPropagation()}
								>
									{importNames.get(contact.importBatchId) ?? `Import #${contact.importBatchId}`}
								</a>
							{:else}
								Added by hand
							{/if}
						</Table.Cell>
						<Table.Cell class="text-right text-muted-foreground">
							{formatDate(contact.dateAdded)}
						</Table.Cell>
					</tr>
				{:else}
					<Table.Row>
						<Table.Cell colspan={5} class="py-8 text-center text-muted-foreground">
							{list.isPending ? 'Loading…' : `No ${term.many} match.`}
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

<AddContactDialog bind:open={addOpen} />
