<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import UploadIcon from '@lucide/svelte/icons/upload';

	import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte';
	import EmptyState from '$lib/components/app/EmptyState.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { ImportBatchDto_Serialize } from '$lib/ipc/bindings';
	import { commands } from '$lib/ipc/bindings';
	import { call } from '$lib/ipc/client';
	import { importQueries } from '$lib/ipc/queries';
	import { toast } from '$lib/stores/toasts.svelte';
	import { reportError } from '$lib/utils/errors';
	import { useTerm } from '$lib/stores/term.svelte';
	import { formatDateTime } from '$lib/utils/format';
	import { PAGE_SIZE, offsetFor, pageParam } from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';

	const offset = $derived(offsetFor(pageParam(page.url)));
	const term = useTerm();
	const list = createQuery(() => ({
		...importQueries.list({ limit: PAGE_SIZE, offset }),
		placeholderData: keepPreviousData
	}));

	let discarding = $state<ImportBatchDto_Serialize | null>(null);
	let discardOpen = $state(false);

	async function discard() {
		if (!discarding) return;
		try {
			await call(commands.importsDiscard({ batchId: discarding.id }));
			toast.success('Draft discarded');
		} catch (err) {
			reportError(err);
			throw err;
		}
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between gap-3">
		<h1 class="text-xl font-semibold">Imports</h1>
		<Button href={href('/imports/new')}><UploadIcon />Import list</Button>
	</div>

	{#if list.isError}
		<p class="text-sm text-destructive">Could not load imports: {list.error.message}</p>
	{:else if list.data && list.data.total === 0}
		<EmptyState title="Nothing imported yet">
			<p>Import an .xlsx, .ods or .csv list of {term.many}.</p>
			<Button href={href('/imports/new')}>Import list</Button>
		</EmptyState>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>Source</Table.Head>
					<Table.Head>Date</Table.Head>
					<Table.Head>Status</Table.Head>
					<Table.Head class="text-right">Added</Table.Head>
					<Table.Head class="text-right">Updated</Table.Head>
					<Table.Head class="text-right">Removed</Table.Head>
					<Table.Head></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each list.data?.items ?? [] as batch, index (batch.id)}
					<Table.Row
						class="stagger animate-row-in cursor-pointer"
						style="--i: {index}"
						onclick={() => go(`/imports/${batch.id}`)}
					>
						<Table.Cell class="font-medium">
							<a href={href(`/imports/${batch.id}`)} class="hover:underline">{batch.sourceName}</a>
							<span class="ml-1 text-xs text-muted-foreground uppercase">{batch.sourceKind}</span>
						</Table.Cell>
						<Table.Cell class="text-muted-foreground">
							{formatDateTime(batch.appliedAt ?? batch.createdAt)}
						</Table.Cell>
						<Table.Cell><StatusBadge status={batch.status} /></Table.Cell>
						<Table.Cell class="text-right tabular-nums">{batch.summary.added}</Table.Cell>
						<Table.Cell class="text-right tabular-nums">{batch.summary.updated}</Table.Cell>
						<Table.Cell class="text-right tabular-nums">{batch.summary.removed}</Table.Cell>
						<Table.Cell class="text-right">
							{#if batch.status === 'draft'}
								<Button
									size="sm"
									variant="outline"
									onclick={(event: MouseEvent) => {
										event.stopPropagation();
										discarding = batch;
										discardOpen = true;
									}}
								>
									Discard
								</Button>
							{/if}
						</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={7} class="py-8 text-center text-muted-foreground">
							{list.isPending ? 'Loading…' : 'No imports.'}
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

<ConfirmDialog
	bind:open={discardOpen}
	title="Discard this draft?"
	description="Nothing from it was applied, so no {term.one} changes. To import the file, start a new import."
	confirmLabel="Discard"
	destructive
	onConfirm={discard}
/>
