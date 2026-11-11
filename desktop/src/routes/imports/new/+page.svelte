<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';
	import FileIcon from '@lucide/svelte/icons/file-spreadsheet';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';

	import BackLink from '$lib/components/app/BackLink.svelte';
	import CountUp from '$lib/components/app/CountUp.svelte';
	import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte';
	import FormError from '$lib/components/app/FormError.svelte';
	import Section from '$lib/components/app/Section.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { commands } from '$lib/ipc/bindings';
	import { importQueries } from '$lib/ipc/queries';
	import ColumnSelect from '$lib/screens/imports/ColumnSelect.svelte';
	import ConflictCard from '$lib/screens/imports/ConflictCard.svelte';
	import { actionLabel, warningText } from '$lib/screens/imports/labels';
	import { ImportWizard } from '$lib/screens/imports/wizard.svelte';
	import { confetti } from '$lib/utils/confetti';
	import { formatNumber, fullName, plural } from '$lib/utils/format';
	import { enter } from '$lib/utils/motion';
	import { useTerm } from '$lib/stores/term.svelte';
	import { go, href } from '$lib/utils/paths';

	const wizard = new ImportWizard(commands);
	const term = useTerm();

	const steps = [
		{ id: 'source', label: 'File' },
		{ id: 'columns', label: 'Columns' },
		{ id: 'review', label: 'Review' },
		{ id: 'done', label: 'Done' }
	] as const;
	const stepIndex = $derived(steps.findIndex((s) => s.id === wizard.step));

	const preview = $derived(wizard.preview);
	// Rows shown next to conflicts; the preview sample may not include every conflicting row.
	const planRows = createQuery(() => ({
		...importQueries.planRows({ batchId: preview?.batch.id ?? 0, limit: 500, offset: 0 }),
		enabled: !!preview && preview.conflicts.length > 0 && wizard.step === 'review'
	}));

	let discardOpen = $state(false);

	// Celebrate once when an import lands: two cannons from the bottom corners.
	let celebrated = false;
	$effect(() => {
		if (wizard.step !== 'done' || celebrated) return;
		celebrated = true;
		const y = window.innerHeight - 40;
		confetti({ from: { x: 40, y }, count: 150, spread: 50, power: 22 });
		confetti({ from: { x: window.innerWidth - 40, y }, count: 150, spread: 50, power: 22 });
	});

	function kb(bytes: number): string {
		return bytes < 1024 ? `${bytes} B` : `${formatNumber(Math.round(bytes / 1024))} KB`;
	}

	async function discard() {
		await wizard.discard();
		if (wizard.error) throw wizard.error;
		await go('/imports');
	}
</script>

<div class="flex max-w-4xl flex-col gap-4">
	<BackLink to="/imports" label="Imports" />
	<h1 class="text-xl font-semibold">Import list</h1>

	<ol class="flex gap-2 text-sm">
		{#each steps as step, index (step.id)}
			<li
				class={[
					'flex items-center gap-1.5',
					index === stepIndex ? 'font-semibold' : 'text-muted-foreground'
				]}
			>
				{#key index <= stepIndex}
					<span
						class={[
							'flex size-5 items-center justify-center rounded-full text-xs transition-colors duration-300',
							index <= stepIndex ? 'animate-pop bg-primary text-primary-foreground' : 'bg-muted'
						]}>{index < stepIndex ? '✓' : index + 1}</span
					>
				{/key}
				{step.label}
				{#if index < steps.length - 1}<span class="text-muted-foreground">›</span>{/if}
			</li>
		{/each}
	</ol>

	{#if wizard.busy}
		<p class="flex items-center gap-2 text-sm text-muted-foreground" role="status">
			<LoaderIcon class="size-4 animate-spin" />{wizard.busy}
		</p>
	{/if}
	<FormError error={wizard.error} />

	{#key wizard.step}
		<div in:enter={{ x: 24, y: 0 }} class="flex flex-col gap-4">
			{#if wizard.step === 'source'}
				<Section title="Choose a file">
					<div class="flex flex-col items-start gap-3">
						<p class="text-sm text-muted-foreground">
							An .xlsx, .xls, .ods or .csv file with a header row containing first name, last name
							and email columns. Nothing changes until you apply the import.
						</p>
						<Button onclick={() => wizard.pick()} disabled={!!wizard.busy}>
							<FileIcon />Choose file…
						</Button>
					</div>
				</Section>
			{:else if wizard.step === 'columns' && wizard.inspection && wizard.source}
				{@const inspection = wizard.inspection}
				<Section title="Columns">
					<div class="flex flex-col gap-4">
						<p class="text-sm text-muted-foreground">
							<span class="font-medium text-foreground">{wizard.source.name}</span>
							· {inspection.kind.toUpperCase()} · {kb(wizard.source.bytes)} ·
							{plural(inspection.rowCount, 'row')} · headers in row {inspection.headerRow}
							<button class="ml-2 underline" onclick={() => wizard.pick()}
								>Choose another file</button
							>
						</p>

						{#if inspection.sheets.length > 1}
							<label class="flex max-w-xs flex-col gap-1.5 text-sm font-medium">
								Sheet
								<select
									class="h-8 rounded-lg border border-input bg-transparent px-2 text-sm dark:bg-input/30"
									value={wizard.sheet ?? ''}
									onchange={(event) => wizard.inspect(event.currentTarget.value)}
								>
									{#each inspection.sheets as sheet (sheet)}
										<option value={sheet}>{sheet}</option>
									{/each}
								</select>
							</label>
						{/if}

						<div class="grid gap-3 sm:grid-cols-3">
							<ColumnSelect
								id="map-first"
								label="First name"
								headers={inspection.headers}
								bind:value={wizard.mapping.firstName}
							/>
							<ColumnSelect
								id="map-last"
								label="Last name"
								headers={inspection.headers}
								bind:value={wizard.mapping.lastName}
							/>
							<ColumnSelect
								id="map-email"
								label="Email"
								headers={inspection.headers}
								bind:value={wizard.mapping.email}
							/>
						</div>

						{#if inspection.sampleRows.length > 0}
							<Table.Root>
								<Table.Header>
									<Table.Row>
										<Table.Head>Row</Table.Head>
										<Table.Head>First name</Table.Head>
										<Table.Head>Last name</Table.Head>
										<Table.Head>Email</Table.Head>
									</Table.Row>
								</Table.Header>
								<Table.Body>
									{#each inspection.sampleRows.slice(0, 5) as sample, index (sample.row)}
										<Table.Row class="stagger animate-row-in" style="--i: {index}">
											<Table.Cell class="text-muted-foreground">{sample.row}</Table.Cell>
											{#each [wizard.mapping.firstName, wizard.mapping.lastName, wizard.mapping.email] as column, index (index)}
												<Table.Cell
													>{column === null ? '—' : (sample.cells[column] ?? '')}</Table.Cell
												>
											{/each}
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						{/if}

						<fieldset class="flex flex-col gap-2">
							<legend class="mb-1 text-sm font-medium">What is this file?</legend>
							<label class="flex items-start gap-2 text-sm">
								<input type="radio" bind:group={wizard.scope} value="delta" class="mt-1" />
								<span>
									<span class="font-medium">Additions and changes</span>
									<span class="block text-muted-foreground">
										Add new people and update known ones. Nobody is removed.
									</span>
								</span>
							</label>
							<label class="flex items-start gap-2 text-sm">
								<input type="radio" bind:group={wizard.scope} value="roster" class="mt-1" />
								<span>
									<span class="font-medium">The complete list</span>
									<span class="block text-muted-foreground">
										Imported {term.many} missing from this file are marked removed (you will see who before
										applying). People added by hand are never removed.
									</span>
								</span>
							</label>
						</fieldset>

						<div class="flex justify-end">
							<Button
								onclick={() => wizard.runPreview()}
								disabled={!wizard.mappingComplete || !!wizard.busy}
							>
								Preview changes
							</Button>
						</div>
					</div>
				</Section>
			{:else if wizard.step === 'review' && preview}
				<dl class="grid grid-cols-3 gap-3 sm:grid-cols-6">
					{#each [['Add', preview.summary.added], ['Update', preview.summary.updated], ['Mark removed', preview.summary.removed], ['Unchanged', preview.summary.unchanged], ['Need a decision', preview.summary.conflicts], ['Invalid', preview.summary.invalid]] as [label, value], index (label)}
						<div
							class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-3"
							style="--i: {index * 2}"
						>
							<dt class="text-xs text-muted-foreground">{label}</dt>
							<dd class="text-lg font-semibold"><CountUp value={Number(value)} /></dd>
						</div>
					{/each}
				</dl>

				{#each preview.warnings as warning, index (index)}
					<p
						class={[
							'rounded-md px-3 py-2 text-sm',
							warning.kind === 'largeRemoval'
								? 'bg-destructive/10 text-destructive'
								: 'bg-warning/10'
						]}
						role="alert"
					>
						{warningText(warning, term)}
					</p>
				{/each}

				{#if preview.conflicts.length > 0}
					<Section title="Decide what to do with these rows">
						<div class="flex flex-col gap-3">
							{#each preview.conflicts as conflict, index (index)}
								<ConflictCard {conflict} rows={planRows.data?.items ?? preview.sample} {wizard} />
							{/each}
						</div>
					</Section>
				{/if}

				{#if preview.removals.length > 0}
					<Section title="Will be marked removed ({preview.removals.length})">
						<ul class="flex flex-col gap-1 text-sm">
							{#each preview.removals as person (person.id)}
								<li>
									{fullName(person)}
									<span class="text-muted-foreground">&lt;{person.email}&gt;</span>
								</li>
							{/each}
						</ul>
					</Section>
				{/if}

				{#if preview.errors.length > 0}
					<Section title="Rows that will be skipped ({preview.errors.length})">
						<ul class="flex flex-col gap-1 text-sm">
							{#each preview.errors as rowError, index (index)}
								<li>
									<span class="text-muted-foreground">Row {rowError.row}:</span>
									{rowError.field}
									{rowError.reason}
								</li>
							{/each}
						</ul>
					</Section>
				{/if}

				{#if preview.sample.length > 0}
					<Section title="Sample">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head>Row</Table.Head>
									<Table.Head>Name</Table.Head>
									<Table.Head>Email</Table.Head>
									<Table.Head>Change</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each preview.sample as row, index (row.row)}
									<Table.Row class="stagger animate-row-in" style="--i: {index}">
										<Table.Cell class="text-muted-foreground">{row.row}</Table.Cell>
										<Table.Cell>{fullName(row)}</Table.Cell>
										<Table.Cell>{row.email}</Table.Cell>
										<Table.Cell>{actionLabel(row.action)}</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</Section>
				{/if}

				<div class="flex flex-wrap items-center justify-between gap-2">
					<div class="flex gap-2">
						<Button variant="outline" onclick={() => wizard.back()} disabled={!!wizard.busy}>
							Back
						</Button>
						<Button variant="outline" onclick={() => (discardOpen = true)} disabled={!!wizard.busy}>
							Discard
						</Button>
					</div>
					<div class="flex items-center gap-3">
						{#if wizard.undecidedRows.length > 0}
							<span class="text-sm text-muted-foreground">
								{plural(wizard.undecidedRows.length, 'row needs', 'rows need')} a decision
							</span>
						{/if}
						<Button
							onclick={() => wizard.apply()}
							disabled={wizard.undecidedRows.length > 0 || !!wizard.busy}
						>
							Apply import
						</Button>
					</div>
				</div>
			{:else if wizard.step === 'done' && wizard.result}
				{@const result = wizard.result}
				<Section title="Import applied">
					<div class="flex flex-col items-start gap-3 text-sm">
						<span
							class="flex size-12 animate-pop items-center justify-center rounded-full bg-success/15 text-2xl text-success"
							aria-hidden="true">✓</span
						>
						<p>
							{plural(result.summary.added, term.one, term.many)} added, {result.summary.updated} updated,
							{result.summary.removed} marked removed, {result.summary.unchanged} unchanged.
						</p>
						<div class="flex gap-2">
							<Button href={href(`/imports/${result.id}`)}>View import</Button>
							<Button variant="outline" href={href('/freshies')}>Go to {term.many}</Button>
						</div>
					</div>
				</Section>
			{/if}
		</div>
	{/key}
</div>

<ConfirmDialog
	bind:open={discardOpen}
	title="Discard this import?"
	description="Nothing has been applied yet, so no {term.one} changes."
	confirmLabel="Discard"
	destructive
	onConfirm={discard}
/>
