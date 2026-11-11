<script lang="ts">
	import { page } from '$app/state';
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';
	import CopyIcon from '@lucide/svelte/icons/copy';

	import BackLink from '$lib/components/app/BackLink.svelte';
	import Field from '$lib/components/app/Field.svelte';
	import FormError from '$lib/components/app/FormError.svelte';
	import Pager from '$lib/components/app/Pager.svelte';
	import SearchInput from '$lib/components/app/SearchInput.svelte';
	import Section from '$lib/components/app/Section.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { Textarea } from '$lib/components/ui/textarea/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call, fieldError, isIpcError } from '$lib/ipc/client';
	import { sendBatchQueries } from '$lib/ipc/queries';
	import { toast } from '$lib/stores/toasts.svelte';
	import { formatDateTime, plural } from '$lib/utils/format';
	import { PAGE_SIZE, listHref, offsetFor, pageParam, param } from '$lib/utils/list-params';
	import { go, href } from '$lib/utils/paths';

	const id = $derived(Number(page.params.id));
	const batch = createQuery(() => sendBatchQueries.detail(id));
	const search = $derived(param(page.url, 'search'));
	const offset = $derived(offsetFor(pageParam(page.url)));
	const recipients = createQuery(() => ({
		...sendBatchQueries.recipients(id, { search, limit: PAGE_SIZE, offset }),
		placeholderData: keepPreviousData
	}));

	let note = $state('');
	let loadedNote = $state<string | null>(null);
	let noteError = $state<unknown>(null);
	let savingNote = $state(false);
	let copying = $state(false);

	$effect(() => {
		const current = batch.data?.note;
		if (current !== undefined && current !== loadedNote) {
			loadedNote = current;
			note = current;
		}
	});

	async function copyAgain() {
		copying = true;
		try {
			const result = await call(commands.sendBatchesRecopy({ id }));
			toast.success(`Copied ${plural(result.recipientCount, 'address', 'addresses')} again`);
		} catch (err) {
			toast.ipcError(err);
		} finally {
			copying = false;
		}
	}

	async function saveNote(event: SubmitEvent) {
		event.preventDefault();
		savingNote = true;
		noteError = null;
		try {
			await call(commands.sendBatchesSetNote({ id, note }));
			toast.success('Note saved');
		} catch (err) {
			noteError = err;
		} finally {
			savingNote = false;
		}
	}

	function onSearch(value: string) {
		void go(listHref(page.url, { search: value }), { replace: true, reset: false });
	}
</script>

<div class="flex flex-col gap-4">
	<BackLink to="/history" label="Send history" />

	{#if batch.isError}
		<p class="text-sm text-destructive">
			{isIpcError(batch.error) && batch.error.code === 'NOT_FOUND'
				? 'This batch does not exist.'
				: `Could not load this batch: ${batch.error.message}`}
		</p>
	{:else if batch.data}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div class="flex flex-col gap-1">
				<h1 class="text-xl font-semibold">
					<a href={href(`/campaigns/${batch.data.campaignId}`)} class="hover:underline">
						{batch.data.campaignName}
					</a>
				</h1>
				<p class="text-sm text-muted-foreground">
					{formatDateTime(batch.data.createdAt)} · {plural(batch.data.recipientCount, 'recipient')}
				</p>
			</div>
			<Button onclick={copyAgain} disabled={copying}><CopyIcon />Copy again</Button>
		</div>

		<Section title="Note">
			<form onsubmit={saveNote} class="flex flex-col gap-3">
				<Field id="batch-note" label="Note" error={fieldError(noteError, 'note')}>
					<Textarea id="batch-note" bind:value={note} rows={2} placeholder="Optional" />
				</Field>
				<FormError error={noteError} />
				<div class="flex justify-end">
					<Button type="submit" variant="outline" disabled={savingNote || note === loadedNote}>
						Save note
					</Button>
				</div>
			</form>
		</Section>

		<div class="flex items-center gap-3">
			<h2 class="text-sm font-semibold">Recipients</h2>
			<SearchInput value={search ?? ''} placeholder="Search name or email" {onSearch} />
		</div>
		<p class="text-xs text-muted-foreground">
			Names and addresses as they were when this batch was copied.
		</p>
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>Name</Table.Head>
					<Table.Head>Email</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each recipients.data?.items ?? [] as record (record.id)}
					<Table.Row>
						<Table.Cell class="font-medium">
							{#if record.contactId}
								<a href={href(`/freshies/${record.contactId}`)} class="hover:underline">
									{record.lastName}, {record.firstName}
								</a>
							{:else}
								{record.lastName}, {record.firstName}
							{/if}
						</Table.Cell>
						<Table.Cell>{record.email}</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={2} class="py-8 text-center text-muted-foreground">
							{recipients.isPending ? 'Loading…' : 'Nobody matches.'}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
		{#if recipients.data}
			<Pager total={recipients.data.total} offset={recipients.data.offset} />
		{/if}
	{:else}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{/if}
</div>
