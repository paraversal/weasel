<script lang="ts">
	import Field from '$lib/components/app/Field.svelte';
	import FormError from '$lib/components/app/FormError.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Textarea } from '$lib/components/ui/textarea/index.js';
	import type { CampaignDto } from '$lib/ipc/bindings';
	import { commands } from '$lib/ipc/bindings';
	import { call, fieldError } from '$lib/ipc/client';
	import { toast } from '$lib/stores/toasts.svelte';
	import { go } from '$lib/utils/paths';

	/** Without `campaign` this creates (and activates) one; with it, it edits. */
	let { open = $bindable(false), campaign }: { open?: boolean; campaign?: CampaignDto } = $props();

	let name = $state('');
	let description = $state('');
	let error = $state<unknown>(null);
	let pending = $state(false);

	$effect(() => {
		if (open) {
			name = campaign?.name ?? '';
			description = campaign?.description ?? '';
			error = null;
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		pending = true;
		error = null;
		try {
			if (campaign) {
				await call(
					commands.campaignsUpdate({
						id: campaign.id,
						version: campaign.version,
						name,
						description
					})
				);
				open = false;
				toast.success('Saved');
				return;
			}
			const created = await call(commands.campaignsCreate({ name, description }));
			open = false;
			// Campaigns start as drafts and drafts cannot send. Creating one here means it is
			// ready to send; if activation fails, the detail page offers "Activate".
			try {
				await call(
					commands.campaignsSetStatus({
						id: created.id,
						version: created.version,
						status: 'active'
					})
				);
				toast.success(`Created ${created.name}`);
			} catch {
				toast.error(`Created ${created.name}, but could not activate it`);
			}
			await go(`/campaigns/${created.id}`);
		} catch (err) {
			error = err;
		} finally {
			pending = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-w-lg">
		<form onsubmit={submit} class="flex flex-col gap-4">
			<Dialog.Header>
				<Dialog.Title>{campaign ? 'Edit campaign' : 'New campaign'}</Dialog.Title>
			</Dialog.Header>
			<Field id="campaign-name" label="Name" error={fieldError(error, 'name')}>
				<Input id="campaign-name" bind:value={name} autocomplete="off" />
			</Field>
			<Field id="campaign-description" label="Description" error={fieldError(error, 'description')}>
				<Textarea id="campaign-description" bind:value={description} rows={4} />
			</Field>
			<FormError {error} />
			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (open = false)}>Cancel</Button>
				<Button type="submit" disabled={pending}>{campaign ? 'Save' : 'Create campaign'}</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
