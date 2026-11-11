import type { ConflictKindDto, PlanActionDto, PlanWarningDto } from '$lib/ipc/bindings';
import type { Term } from '$lib/stores/term.svelte';
import { plural } from '$lib/utils/format';

export function conflictLabel(kind: ConflictKindDto, term: Term): string {
	switch (kind) {
		case 'duplicateEmailInSource':
			return 'The same email appears in several rows of the file.';
		case 'ambiguousHistory':
			return `This email used to belong to more than one ${term.one}.`;
		case 'ambiguousIdentity':
			return `More than one ${term.one} has this name.`;
		case 'manualContact':
			return `This row matches a ${term.one} who was added by hand.`;
		case 'contactMatchedTwice':
			return `Several rows match the same ${term.one}.`;
	}
}

export function warningText(warning: PlanWarningDto, term: Term): string {
	switch (warning.kind) {
		case 'largeRemoval':
			return `This would mark ${plural(warning.count, term.one, term.many)} removed (of ${warning.activeImportContacts} imported and active). Only continue if this file is the complete list.`;
		case 'sameFileAsBatch':
			return `This exact file was already imported (import #${warning.batchId}).`;
		case 'ignoredTrailingRows':
			return `${plural(warning.count, 'row')} at the end of the sheet were ignored (totals or notes).`;
	}
}

export function actionLabel(action: PlanActionDto): string {
	switch (action.kind) {
		case 'add':
			return 'Add';
		case 'updateDetails':
			return 'Update details';
		case 'updateEmail':
			return `Update email (was ${action.target.email})`;
		case 'restore':
			return 'Restore';
		case 'noop':
			return 'No change';
		case 'conflict':
			return 'Needs a decision';
	}
}
