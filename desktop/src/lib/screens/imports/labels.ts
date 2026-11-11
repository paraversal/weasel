import type { ConflictKindDto, PlanActionDto, PlanWarningDto } from '$lib/ipc/bindings';
import { plural } from '$lib/utils/format';

export const conflictLabels: Record<ConflictKindDto, string> = {
	duplicateEmailInSource: 'The same email appears in several rows of the file.',
	ambiguousHistory: 'This email used to belong to more than one freshie.',
	ambiguousIdentity: 'More than one freshie has this name.',
	manualContact: 'This row matches a freshie who was added by hand.',
	contactMatchedTwice: 'Several rows match the same freshie.'
};

export function warningText(warning: PlanWarningDto): string {
	switch (warning.kind) {
		case 'largeRemoval':
			return `This would mark ${plural(warning.count, 'freshie')} removed (of ${warning.activeImportContacts} imported and active). Only continue if this file is the complete list.`;
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
