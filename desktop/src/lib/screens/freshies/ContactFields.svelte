<script lang="ts">
	import Field from '$lib/components/app/Field.svelte';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Textarea } from '$lib/components/ui/textarea/index.js';
	import { fieldError } from '$lib/ipc/client';

	export interface ContactValues {
		firstName: string;
		lastName: string;
		email: string;
		notes: string;
	}

	let {
		values = $bindable(),
		error,
		idPrefix
	}: { values: ContactValues; error: unknown; idPrefix: string } = $props();
</script>

<div class="grid grid-cols-2 gap-3">
	<Field id="{idPrefix}-first" label="First name" error={fieldError(error, 'firstName')}>
		<Input id="{idPrefix}-first" bind:value={values.firstName} autocomplete="off" />
	</Field>
	<Field id="{idPrefix}-last" label="Last name" error={fieldError(error, 'lastName')}>
		<Input id="{idPrefix}-last" bind:value={values.lastName} autocomplete="off" />
	</Field>
	<div class="col-span-2">
		<Field id="{idPrefix}-email" label="Email" error={fieldError(error, 'email')}>
			<Input id="{idPrefix}-email" type="email" bind:value={values.email} autocomplete="off" />
		</Field>
	</div>
	<div class="col-span-2">
		<Field id="{idPrefix}-notes" label="Notes" error={fieldError(error, 'notes')}>
			<Textarea id="{idPrefix}-notes" bind:value={values.notes} rows={3} />
		</Field>
	</div>
</div>
