<script lang="ts">
	import FormError from '$lib/components/app/FormError.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call } from '$lib/ipc/client';
	import { toast } from '$lib/stores/toasts.svelte';
	import { fullName } from '$lib/utils/format';
	import { go } from '$lib/utils/paths';

	import ContactFields, { type ContactValues } from './ContactFields.svelte';

	let { open = $bindable(false) }: { open?: boolean } = $props();

	const empty = (): ContactValues => ({ firstName: '', lastName: '', email: '', notes: '' });
	let values = $state(empty());
	let error = $state<unknown>(null);
	let pending = $state(false);

	$effect(() => {
		if (open) {
			values = empty();
			error = null;
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		pending = true;
		error = null;
		try {
			const contact = await call(
				commands.contactsCreate({ ...values, notes: values.notes.trim() || null })
			);
			open = false;
			toast.success(`Added ${fullName(contact)}`);
			await go(`/freshies/${contact.id}`);
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
				<Dialog.Title>Add freshie</Dialog.Title>
				<Dialog.Description>
					Added by hand, so later roster imports never mark them removed.
				</Dialog.Description>
			</Dialog.Header>
			<ContactFields bind:values {error} idPrefix="add" />
			<FormError {error} />
			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (open = false)}>Cancel</Button>
				<Button type="submit" disabled={pending}>Add freshie</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
