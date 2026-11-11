<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import Field from '$lib/components/app/Field.svelte';
	import FormError from '$lib/components/app/FormError.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { call, fieldError } from '$lib/ipc/client';
	import { settingsQuery } from '$lib/ipc/queries';
	import { toast } from '$lib/stores/toasts.svelte';
	import { DEFAULT_TERM, capitalize } from '$lib/stores/term.svelte';

	const settings = createQuery(() => settingsQuery());

	let singular = $state('');
	let plural = $state('');
	let loaded = $state<string | null>(null);
	let error = $state<unknown>(null);
	let saving = $state(false);

	$effect(() => {
		const term = settings.data?.audienceTerm;
		if (!term) return;
		const key = `${term.singular}|${term.plural}`;
		if (key === loaded) return;
		loaded = key;
		singular = term.singular;
		plural = term.plural;
	});

	const dirty = $derived(loaded !== `${singular.trim()}|${plural.trim()}`);

	async function save(next = { singular, plural }) {
		saving = true;
		error = null;
		try {
			await call(commands.settingsUpdate({ patch: { audienceTerm: next } }));
			toast.success('Saved');
		} catch (err) {
			error = err;
		} finally {
			saving = false;
		}
	}
</script>

<form
	class="flex flex-col gap-3"
	onsubmit={(event) => {
		event.preventDefault();
		void save();
	}}
>
	<p class="text-sm text-muted-foreground">
		What to call the people you import and send to, e.g. “member” / “members”.
	</p>
	<div class="grid max-w-md grid-cols-2 gap-3">
		<Field id="term-singular" label="One" error={fieldError(error, 'audienceTerm')}>
			<Input id="term-singular" bind:value={singular} autocomplete="off" />
		</Field>
		<Field id="term-plural" label="Many">
			<Input id="term-plural" bind:value={plural} autocomplete="off" />
		</Field>
	</div>
	<p class="text-xs text-muted-foreground">
		Preview: “{capitalize(plural.trim() || '…')}” in the sidebar, “Add {singular.trim() || '…'}” on
		buttons.
	</p>
	<FormError {error} />
	<div class="flex gap-2">
		<Button type="submit" disabled={!dirty || saving}>Save wording</Button>
		<Button
			type="button"
			variant="outline"
			disabled={saving || loaded === `${DEFAULT_TERM.singular}|${DEFAULT_TERM.plural}`}
			onclick={() => save(DEFAULT_TERM)}
		>
			Reset to “{DEFAULT_TERM.plural}”
		</Button>
	</div>
</form>
