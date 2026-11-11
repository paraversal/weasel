import { SvelteMap } from 'svelte/reactivity';

import type {
	ConflictResolutionDto_Deserialize,
	ImportBatchDto_Serialize,
	ImportPreviewDto_Serialize,
	ImportScopeDto,
	SourceFileDto,
	SourceInspectionDto_Serialize,
	commands
} from '$lib/ipc/bindings';
import { call } from '$lib/ipc/client';

export type ImportCommands = Pick<
	typeof commands,
	'importsPickSource' | 'importsInspect' | 'importsPreview' | 'importsApply' | 'importsDiscard'
>;

export type WizardStep = 'source' | 'columns' | 'review' | 'done';

export interface Mapping {
	firstName: number | null;
	lastName: number | null;
	email: number | null;
}

export type Decision =
	{ action: 'linkContact'; contactId: number } | { action: 'createNew' } | { action: 'skip' };

/**
 * All state and rules of the import wizard; components only render it. Nothing is
 * written until `apply()`: `runPreview()` creates a draft batch, re-previewing replaces
 * that draft, and `discard()` drops it.
 */
export class ImportWizard {
	step = $state<WizardStep>('source');
	source = $state<SourceFileDto | null>(null);
	inspection = $state<SourceInspectionDto_Serialize | null>(null);
	sheet = $state<string | null>(null);
	mapping = $state<Mapping>({ firstName: null, lastName: null, email: null });
	scope = $state<ImportScopeDto>('delta');
	preview = $state<ImportPreviewDto_Serialize | null>(null);
	result = $state<ImportBatchDto_Serialize | null>(null);
	busy = $state<string | null>(null);
	error = $state<unknown>(null);
	readonly decisions = new SvelteMap<number, Decision>();

	constructor(private readonly api: ImportCommands) {}

	get mappingComplete(): boolean {
		const { firstName, lastName, email } = this.mapping;
		return firstName !== null && lastName !== null && email !== null;
	}

	/** Every source row that belongs to a conflict and still has no decision. */
	get undecidedRows(): number[] {
		const rows = this.preview?.conflicts.flatMap((conflict) => conflict.rows) ?? [];
		return rows.filter((row) => !this.decisions.has(row));
	}

	async pick(): Promise<void> {
		await this.run('Opening file…', async () => {
			const picked = await call(this.api.importsPickSource());
			if (!picked) return;
			this.source = picked;
			this.sheet = null;
			await this.load();
		});
	}

	async inspect(sheet: string | null): Promise<void> {
		await this.run('Reading file…', async () => {
			this.sheet = sheet;
			await this.load();
		});
	}

	async runPreview(): Promise<void> {
		const { firstName, lastName, email } = this.mapping;
		if (!this.source || firstName === null || lastName === null || email === null) return;
		const path = this.source.path;
		await this.run('Comparing with your list…', async () => {
			this.preview = await call(
				this.api.importsPreview({
					path,
					sheet: this.sheet,
					mapping: { firstName, lastName, email },
					scope: this.scope,
					// Re-previewing replaces the previous draft instead of piling up drafts.
					replacesBatchId: this.preview?.batch.id ?? null
				})
			);
			this.decisions.clear();
			this.step = 'review';
		});
	}

	back(): void {
		if (this.step === 'review') this.step = 'columns';
	}

	decide(row: number, decision: Decision): void {
		this.decisions.set(row, decision);
	}

	async apply(): Promise<void> {
		const preview = this.preview;
		if (!preview || this.undecidedRows.length > 0) return;
		const resolutions: ConflictResolutionDto_Deserialize[] = [...this.decisions].map(
			([row, decision]) => ({
				row,
				action: decision.action,
				contactId: decision.action === 'linkContact' ? decision.contactId : null
			})
		);
		await this.run('Applying…', async () => {
			this.result = await call(this.api.importsApply({ batchId: preview.batch.id, resolutions }));
			this.step = 'done';
		});
	}

	async discard(): Promise<void> {
		const preview = this.preview;
		if (!preview) return;
		await this.run('Discarding…', async () => {
			await call(this.api.importsDiscard({ batchId: preview.batch.id }));
			this.preview = null;
			this.decisions.clear();
		});
	}

	private async load(): Promise<void> {
		if (!this.source) return;
		const inspection = await call(
			this.api.importsInspect({ path: this.source.path, sheet: this.sheet })
		);
		this.inspection = inspection;
		this.sheet = inspection.sheetName;
		this.mapping = { ...inspection.suggestedMapping };
		this.step = 'columns';
	}

	private async run(label: string, work: () => Promise<void>): Promise<void> {
		this.busy = label;
		this.error = null;
		try {
			await work();
		} catch (err) {
			this.error = err;
		} finally {
			this.busy = null;
		}
	}
}
