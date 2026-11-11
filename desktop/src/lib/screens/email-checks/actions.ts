import { commands } from '$lib/ipc/bindings';
import type { EmailMarkDto } from '$lib/ipc/bindings';
import { call } from '$lib/ipc/client';
import { toast } from '$lib/stores/toasts.svelte';
import { reportError } from '$lib/utils/errors';

const confirmations: Record<EmailMarkDto, string> = {
	invalid: 'Address marked invalid',
	valid: 'Address marked valid',
	default: 'Address follows the rules again'
};

/** Set an address's verdict. Events refresh every list it appears in. */
export async function markEmail(email: string, mark: EmailMarkDto): Promise<boolean> {
	try {
		await call(commands.emailMark({ email, mark }));
		toast.success(confirmations[mark], {
			action:
				mark === 'invalid'
					? { label: 'Undo', onClick: () => void markEmail(email, 'default') }
					: undefined
		});
		return true;
	} catch (error) {
		reportError(error);
		return false;
	}
}

/** Record or undo a bounce for one recipient of a send. */
export async function setBounced(recordId: number, bounced: boolean): Promise<boolean> {
	try {
		const record = await call(commands.sendRecordsSetBounced({ recordId, bounced }));
		if (bounced) {
			toast.success(`${record.email} marked as bounced`, {
				action: { label: 'Undo', onClick: () => void setBounced(recordId, false) }
			});
		} else {
			toast.success('Bounce undone');
		}
		return true;
	} catch (error) {
		reportError(error);
		return false;
	}
}
