<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';

	let {
		open = $bindable(false),
		title,
		description,
		confirmLabel,
		destructive = false,
		onConfirm
	}: {
		open?: boolean;
		title: string;
		description: string;
		confirmLabel: string;
		destructive?: boolean;
		/** Resolve to close; throw to keep the dialog open (the caller reports the error). */
		onConfirm: () => Promise<unknown>;
	} = $props();

	let pending = $state(false);

	async function confirm() {
		pending = true;
		try {
			await onConfirm();
			open = false;
		} catch {
			// The caller already reported it; stay open so the user can retry or cancel.
		} finally {
			pending = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>{description}</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)} disabled={pending}>Cancel</Button>
			<Button
				variant={destructive ? 'destructive' : 'default'}
				onclick={confirm}
				disabled={pending}
			>
				{confirmLabel}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
