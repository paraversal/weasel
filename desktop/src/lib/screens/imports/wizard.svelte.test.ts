import { describe, expect, it, vi } from 'vitest';

import type { ImportPreviewDto_Serialize, SourceInspectionDto_Serialize } from '$lib/ipc/bindings';

import { ImportWizard, type ImportCommands } from './wizard.svelte';

const ok = <T>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const source = { path: '/tmp/roster.csv', name: 'roster.csv', kind: 'csv' as const, bytes: 10 };

function inspection(email: number | null): SourceInspectionDto_Serialize {
	return {
		name: 'roster.csv',
		kind: 'csv',
		sheets: [],
		sheetName: null,
		headerRow: 1,
		headers: ['Vorname', 'Nachname', 'Mail'],
		suggestedMapping: { firstName: 0, lastName: 1, email },
		sampleRows: [],
		rowCount: 3,
		ignoredTrailing: 0
	};
}

function preview(batchId: number, conflictRows: number[][] = []): ImportPreviewDto_Serialize {
	const zero = {
		rowCount: 0,
		added: 0,
		updated: 0,
		removed: 0,
		conflicts: 0,
		unchanged: 0,
		invalid: 0
	};
	return {
		batch: {
			id: batchId,
			sourceKind: 'csv',
			sourceName: 'roster.csv',
			sheetName: null,
			scope: 'delta',
			status: 'draft',
			createdAt: 0,
			appliedAt: null,
			summary: zero,
			error: null
		},
		headers: [],
		headerRow: 1,
		suggestedMapping: { firstName: 0, lastName: 1, email: 2 },
		mapping: { firstName: 0, lastName: 1, email: 2 },
		summary: zero,
		sample: [],
		conflicts: conflictRows.map((rows) => ({
			rows,
			kind: 'duplicateEmailInSource' as const,
			candidates: []
		})),
		errors: [],
		warnings: [],
		removals: []
	};
}

function api(overrides: Partial<ImportCommands> = {}): ImportCommands {
	return {
		importsPickSource: vi.fn(() => ok(source)),
		importsInspect: vi.fn(() => ok(inspection(2))),
		importsPreview: vi.fn(() => ok(preview(7))),
		importsApply: vi.fn(() => ok(preview(7).batch)),
		importsDiscard: vi.fn(() => ok(preview(7).batch)),
		...overrides
	} as ImportCommands;
}

describe('ImportWizard', () => {
	it('stays on the source step when the picker is cancelled', async () => {
		const commands = api({ importsPickSource: vi.fn(() => ok(null)) });
		const wizard = new ImportWizard(commands);
		await wizard.pick();
		expect(wizard.step).toBe('source');
		expect(commands.importsInspect).not.toHaveBeenCalled();
	});

	it('refuses to preview until every column is mapped', async () => {
		const commands = api({ importsInspect: vi.fn(() => ok(inspection(null))) });
		const wizard = new ImportWizard(commands);
		await wizard.pick();
		expect(wizard.step).toBe('columns');
		expect(wizard.mappingComplete).toBe(false);
		await wizard.runPreview();
		expect(commands.importsPreview).not.toHaveBeenCalled();

		wizard.mapping.email = 2;
		await wizard.runPreview();
		expect(commands.importsPreview).toHaveBeenCalledTimes(1);
		expect(wizard.step).toBe('review');
	});

	it('replaces the previous draft when previewing again', async () => {
		const commands = api();
		const wizard = new ImportWizard(commands);
		await wizard.pick();
		await wizard.runPreview();
		wizard.back();
		await wizard.runPreview();
		const calls = vi.mocked(commands.importsPreview).mock.calls;
		expect(calls[0]?.[0].replacesBatchId).toBeNull();
		expect(calls[1]?.[0].replacesBatchId).toBe(7);
	});

	it('blocks apply until every conflicting row has a decision', async () => {
		const commands = api({ importsPreview: vi.fn(() => ok(preview(7, [[3, 9]]))) });
		const wizard = new ImportWizard(commands);
		await wizard.pick();
		await wizard.runPreview();

		wizard.decide(3, { action: 'skip' });
		expect(wizard.undecidedRows).toEqual([9]);
		await wizard.apply();
		expect(commands.importsApply).not.toHaveBeenCalled();

		wizard.decide(9, { action: 'createNew' });
		await wizard.apply();
		expect(commands.importsApply).toHaveBeenCalledWith({
			batchId: 7,
			resolutions: [
				{ row: 3, action: 'skip', contactId: null },
				{ row: 9, action: 'createNew', contactId: null }
			]
		});
		expect(wizard.step).toBe('done');
	});

	it('starts over with a dropped file and discards the abandoned draft', async () => {
		const commands = api();
		const wizard = new ImportWizard(commands);
		await wizard.pick();
		await wizard.runPreview();
		expect(wizard.step).toBe('review');

		await wizard.useSource({ ...source, path: '/tmp/other.csv', name: 'other.csv' });
		expect(commands.importsDiscard).toHaveBeenCalledWith({ batchId: 7 });
		expect(wizard.preview).toBeNull();
		expect(wizard.source?.name).toBe('other.csv');
		expect(wizard.step).toBe('columns');
		expect(commands.importsPickSource).toHaveBeenCalledTimes(1);
	});

	it('keeps the draft and reports the error when apply fails', async () => {
		const failure = {
			code: 'VALIDATION' as const,
			message: 'nope',
			details: null,
			retryable: false
		};
		const commands = api({
			importsApply: vi.fn(() => Promise.resolve({ status: 'error' as const, error: failure }))
		});
		const wizard = new ImportWizard(commands);
		await wizard.pick();
		await wizard.runPreview();
		await wizard.apply();
		expect(wizard.step).toBe('review');
		expect(wizard.preview?.batch.id).toBe(7);
		expect(wizard.error).toBeTruthy();
	});
});
