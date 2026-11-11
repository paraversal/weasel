<script lang="ts">
	import { page } from '$app/state';
	import { createQuery } from '@tanstack/svelte-query';

	import BackLink from '$lib/components/app/BackLink.svelte';
	import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte';
	import FormError from '$lib/components/app/FormError.svelte';
	import Section from '$lib/components/app/Section.svelte';
	import StatusBadge from '$lib/components/app/StatusBadge.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { ContactStatusDto } from '$lib/ipc/bindings';
	import { commands } from '$lib/ipc/bindings';
	import { call, isIpcError } from '$lib/ipc/client';
	import { contactQueries } from '$lib/ipc/queries';
	import ContactFields, { type ContactValues } from '$lib/screens/freshies/ContactFields.svelte';
	import { toast } from '$lib/stores/toasts.svelte';
	import { reportError } from '$lib/utils/errors';
	import { formatDate, formatDateTime, fullName } from '$lib/utils/format';
	import { href } from '$lib/utils/paths';

	const id = $derived(Number(page.params.id));
	const detail = createQuery(() => contactQueries.detail(id));
	const contact = $derived(detail.data?.contact);

	let values = $state<ContactValues>({ firstName: '', lastName: '', email: '', notes: '' });
	let loaded = $state<string | null>(null);
	let error = $state<unknown>(null);
	let saving = $state(false);

	// Reset the form whenever a new version arrives (our save, or an event-driven refetch).
	$effect(() => {
		if (!contact) return;
		const key = `${contact.id}:${contact.version}`;
		if (key === loaded) return;
		loaded = key;
		values = {
			firstName: contact.firstName,
			lastName: contact.lastName,
			email: contact.email,
			notes: contact.notes
		};
		error = null;
	});

	const changes = $derived.by(() => {
		if (!contact) return {};
		const patch: Partial<ContactValues> = {};
		for (const key of ['firstName', 'lastName', 'email', 'notes'] as const) {
			if (values[key].trim() !== contact[key]) patch[key] = values[key];
		}
		return patch;
	});
	const dirty = $derived(Object.keys(changes).length > 0);

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!contact || !dirty) return;
		saving = true;
		error = null;
		try {
			await call(commands.contactsUpdate({ id: contact.id, version: contact.version, ...changes }));
			toast.success('Saved');
		} catch (err) {
			error = err;
		} finally {
			saving = false;
		}
	}

	type StatusAction = {
		status: ContactStatusDto;
		label: string;
		title: string;
		description: string;
		destructive: boolean;
	};

	const actions = $derived.by((): StatusAction[] => {
		if (!contact) return [];
		const name = fullName(contact);
		if (contact.status === 'active') {
			return [
				{
					status: 'removed',
					label: 'Mark removed',
					title: `Mark ${name} removed?`,
					description: 'They stop counting as unsent for campaigns. Their history is kept.',
					destructive: true
				},
				{
					status: 'archived',
					label: 'Archive',
					title: `Archive ${name}?`,
					description: 'Archived freshies are hidden from lists and campaigns. History is kept.',
					destructive: false
				}
			];
		}
		return [
			{
				status: 'active',
				label: 'Restore',
				title: `Restore ${name}?`,
				description: 'They become active again and count as unsent for campaigns they missed.',
				destructive: false
			}
		];
	});

	let pendingAction = $state<StatusAction | null>(null);
	let confirmOpen = $state(false);

	function ask(action: StatusAction) {
		pendingAction = action;
		confirmOpen = true;
	}

	async function applyStatus() {
		if (!contact || !pendingAction) return;
		try {
			await call(
				commands.contactsSetStatus({
					id: contact.id,
					version: contact.version,
					status: pendingAction.status
				})
			);
		} catch (err) {
			reportError(err);
			throw err;
		}
	}
</script>

<div class="flex flex-col gap-4">
	<BackLink to="/freshies" label="Freshies" />

	{#if detail.isError}
		<p class="text-sm text-destructive">
			{isIpcError(detail.error) && detail.error.code === 'NOT_FOUND'
				? 'This freshie no longer exists.'
				: `Could not load this freshie: ${detail.error.message}`}
		</p>
	{:else if contact && detail.data}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div class="flex items-center gap-3">
				<h1 class="text-xl font-semibold">{fullName(contact)}</h1>
				<StatusBadge status={contact.status} />
			</div>
			<div class="flex gap-2">
				{#each actions as action (action.status)}
					<Button
						variant={action.destructive ? 'destructive' : 'outline'}
						onclick={() => ask(action)}
					>
						{action.label}
					</Button>
				{/each}
			</div>
		</div>
		<p class="text-sm text-muted-foreground">
			Added {formatDate(contact.dateAdded)} ·
			{contact.source === 'manual' ? 'added by hand' : 'imported'}
			{#if contact.importBatchId}
				· <a class="hover:underline" href={href(`/imports/${contact.importBatchId}`)}>
					import #{contact.importBatchId}
				</a>
			{/if}
		</p>

		<div class="grid gap-4 lg:grid-cols-2">
			<Section title="Details">
				<form onsubmit={save} class="flex flex-col gap-4">
					<ContactFields bind:values {error} idPrefix="edit" />
					<FormError {error} />
					<div class="flex justify-end">
						<Button type="submit" disabled={!dirty || saving}>Save changes</Button>
					</div>
				</form>
			</Section>

			<div class="flex flex-col gap-4">
				<Section title="Campaigns received">
					{#if detail.data.campaigns.length === 0}
						<p class="text-sm text-muted-foreground">Not included in any send yet.</p>
					{:else}
						<ul class="flex flex-col gap-2 text-sm">
							{#each detail.data.campaigns as receipt (receipt.batchId)}
								<li class="flex justify-between gap-2">
									<a class="hover:underline" href={href(`/campaigns/${receipt.campaignId}`)}>
										{receipt.campaignName}
									</a>
									<a
										class="text-muted-foreground hover:underline"
										href={href(`/history/${receipt.batchId}`)}
									>
										{formatDateTime(receipt.sentAt)}
									</a>
								</li>
							{/each}
						</ul>
					{/if}
				</Section>

				<Section title="Email history">
					<ul class="flex flex-col gap-2 text-sm">
						{#each detail.data.emailHistory as entry (entry.validFrom + entry.email)}
							<li class="flex justify-between gap-2">
								<span class={entry.validTo ? 'text-muted-foreground line-through' : ''}>
									{entry.email}
								</span>
								<span class="text-muted-foreground">
									{formatDate(entry.validFrom)} – {entry.validTo
										? formatDate(entry.validTo)
										: 'now'}
									· {entry.reason}
								</span>
							</li>
						{/each}
					</ul>
				</Section>
			</div>
		</div>
	{:else}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{/if}
</div>

{#if pendingAction}
	<ConfirmDialog
		bind:open={confirmOpen}
		title={pendingAction.title}
		description={pendingAction.description}
		confirmLabel={pendingAction.label}
		destructive={pendingAction.destructive}
		onConfirm={applyStatus}
	/>
{/if}
