<script lang="ts">
	import type { ConflictDto, PlannedRowDto } from '$lib/ipc/bindings';
	import { fullName } from '$lib/utils/format';

	import { conflictLabels } from './labels';
	import type { Decision, ImportWizard } from './wizard.svelte';

	let {
		conflict,
		rows,
		wizard
	}: { conflict: ConflictDto; rows: PlannedRowDto[]; wizard: ImportWizard } = $props();

	function encode(decision: Decision | undefined): string {
		if (!decision) return '';
		return decision.action === 'linkContact' ? `link:${decision.contactId}` : decision.action;
	}

	function decode(value: string): Decision | null {
		if (value === 'createNew' || value === 'skip') return { action: value };
		if (value.startsWith('link:'))
			return { action: 'linkContact', contactId: Number(value.slice(5)) };
		return null;
	}

	const byRow = $derived(new Map(rows.map((row) => [row.row, row])));
</script>

<div
	class={[
		'border-border-subtle flex animate-rise flex-col gap-3 rounded-lg border p-3 transition-colors duration-300',
		conflict.rows.every((row) => wizard.decisions.has(row)) ? 'border-success/50 bg-success/5' : ''
	]}
>
	<p class="text-sm font-medium">{conflictLabels[conflict.kind]}</p>
	{#each conflict.rows as row (row)}
		{@const source = byRow.get(row)}
		<div class="flex flex-wrap items-center justify-between gap-2 text-sm">
			<span>
				<span class="text-muted-foreground">Row {row}:</span>
				{#if source}
					{fullName(source)} &lt;{source.email}&gt;
				{/if}
			</span>
			<select
				aria-label="Decision for row {row}"
				class="h-8 rounded-lg border border-input bg-transparent px-2 text-sm dark:bg-input/30"
				value={encode(wizard.decisions.get(row))}
				onchange={(event) => {
					const decision = decode(event.currentTarget.value);
					if (decision) wizard.decide(row, decision);
				}}
			>
				<option value="" disabled>Choose…</option>
				{#each conflict.candidates as candidate (candidate.id)}
					<option value="link:{candidate.id}">
						Same person as {fullName(candidate)} &lt;{candidate.email}&gt;
					</option>
				{/each}
				<option value="createNew">Add as a new freshie</option>
				<option value="skip">Skip this row</option>
			</select>
		</div>
	{/each}
</div>
