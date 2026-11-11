<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';

	import BackLink from '$lib/components/app/BackLink.svelte';
	import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call, isIpcError } from '$lib/ipc/client';
	import { importQueries } from '$lib/ipc/queries';
	import { toast } from '$lib/stores/toasts.svelte';
	import { reportError } from '$lib/utils/errors';
	import { formatDate, formatDateTime } from '$lib/utils/format';
	import { PAGE_SIZE, offsetFor, pageParam } from '$lib/utils/list-params';
	import { href } from '$lib/utils/paths';

	const id = $derived(Number(page.params.id));
	const offset = $derived(offsetFor(pageParam(page.url)));
	const detail = createQuery(() => ({
		...importQueries.detail({ batchId: id, limit: PAGE_SIZE, offset }),
		placeholderData: keepPreviousData
	}));
	const batch = $derived(detail.data?.batch);

	let discardOpen = $state(false);

	async function discard() {
		try {
			await call(commands.importsDiscard({ batchId: id }));
			toast.success('Draft discarded');
		} catch (err) {
			reportError(err);
			throw err;
		}
	}
</script>

<div class="flex flex-col gap-4">
	<BackLink to="/imports" label="Imports" />

	{#if detail.isError}
		<p class="text-sm text-destructive">
			{isIpcError(detail.error) && detail.error.code === 'NOT_FOUND'
				? 'This import does not exist.'
				: `Could not load this import: ${detail.error.message}`}
		</p>
	{:else if batch && detail.data}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div class="flex items-center gap-3">
				<h1 class="text-xl font-semibold">{batch.sourceName}</h1>
				<StatusBadge status={batch.status} />
			</div>
			{#if batch.status === 'draft'}
				<Button variant="outline" onclick={() => (discardOpen = true)}>Discard draft</Button>
			{/if}
		</div>
		<p class="text-sm text-muted-foreground">
			{batch.sourceKind.toUpperCase()}{batch.sheetName ? ` · sheet “${batch.sheetName}”` : ''} ·
			{batch.scope === 'roster' ? 'complete list' : 'add and update only'} ·
			{batch.appliedAt
				? `applied ${formatDateTime(batch.appliedAt)}`
				: `created ${formatDateTime(batch.createdAt)}`}
		</p>
		{#if batch.status === 'draft'}
			<p class="text-sm text-muted-foreground">
				This draft was never applied. To import the file, discard it and start a new import.
			</p>
		{/if}
		{#if batch.error}
			<p class="text-sm text-destructive">{batch.error}</p>
		{/if}

		<dl class="grid grid-cols-3 gap-3 sm:grid-cols-6">
			{#each [['Rows', batch.summary.rowCount], ['Added', batch.summary.added], ['Updated', batch.summary.updated], ['Removed', batch.summary.removed], ['Unchanged', batch.summary.unchanged], ['Invalid', batch.summary.invalid]] as [label, value] (label)}
				<div class="border-border-subtle rounded-lg border bg-surface-raised p-3">
					<dt class="text-xs text-muted-foreground">{label}</dt>
					<dd class="text-lg font-semibold tabular-nums">{value}</dd>
				</div>
			{/each}
		</dl>

		{#if batch.status === 'applied'}
			<h2 class="text-sm font-semibold">Freshies added by this import</h2>
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head>Name</Table.Head>
						<Table.Head>Email</Table.Head>
						<Table.Head>Status</Table.Head>
						<Table.Head class="text-right">Added</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each detail.data.members.items as contact (contact.id)}
						<Table.Row>
							<Table.Cell class="font-medium">
								<a href={href(`/freshies/${contact.id}`)} class="hover:underline">
									{contact.lastName}, {contact.firstName}
								</a>
							</Table.Cell>
							<Table.Cell>{contact.email}</Table.Cell>
							<Table.Cell><StatusBadge status={contact.status} /></Table.Cell>
							<Table.Cell class="text-right text-muted-foreground">
								{formatDate(contact.dateAdded)}
							</Table.Cell>
						</Table.Row>
					{:else}
						<Table.Row>
							<Table.Cell colspan={4} class="py-8 text-center text-muted-foreground">
								No new freshies came from this import.
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
			<Pager total={detail.data.members.total} offset={detail.data.members.offset} />
		{/if}
	{:else}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{/if}
</div>

<ConfirmDialog
	bind:open={discardOpen}
	title="Discard this draft?"
	description="Nothing from it was applied, so no freshie changes."
	confirmLabel="Discard"
	destructive
	onConfirm={discard}
/>
