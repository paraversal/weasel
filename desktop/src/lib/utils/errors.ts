import { isIpcError } from '$lib/ipc/client';
import { toast } from '$lib/stores/toasts.svelte';

/**
 * The one way screens report a failed mutation. A `CONFLICT` means someone (another
 * window, an import) changed the record first; the domain event has already refetched
 * it, so the user only needs to look again.
 */
export function reportError(error: unknown): void {
	if (isIpcError(error) && error.code === 'CONFLICT') {
		toast.error(error.error.message || 'This changed elsewhere. The latest version is shown.');
		return;
	}
	toast.ipcError(error);
}
