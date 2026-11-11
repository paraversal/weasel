import { toast as sonner } from 'svelte-sonner';

import { isIpcError } from '$lib/ipc/client';

export interface ToastAction {
	label: string;
	onClick: () => void;
}

export interface ToastOptions {
	action?: ToastAction;
	duration?: number;
}

/** The subset of toasts features may use; `toast` is the only implementation. */
export interface ToastApi {
	success: (message: string, options?: ToastOptions) => void;
	error: (message: string, options?: ToastOptions) => void;
	info: (message: string, options?: ToastOptions) => void;
	ipcError: (error: unknown, options?: ToastOptions) => void;
}

function show(kind: 'success' | 'error' | 'info', message: string, options?: ToastOptions) {
	const payload = options?.action
		? { action: { label: options.action.label, onClick: options.action.onClick } }
		: undefined;
	const duration = options?.duration;
	if (kind === 'success') sonner.success(message, { ...payload, duration });
	else if (kind === 'error') sonner.error(message, { ...payload, duration });
	else sonner.info(message, { ...payload, duration });
}

async function copyDetails(error: unknown) {
	const details = isIpcError(error) ? error.error : error;
	try {
		await navigator.clipboard.writeText(JSON.stringify(details, null, 2));
		sonner.success('Error details copied');
	} catch {
		sonner.error('Could not copy to the clipboard');
	}
}

export const toast: ToastApi = {
	success: (message, options) => show('success', message, options),
	error: (message, options) => show('error', message, options),
	info: (message, options) => show('info', message, options),
	ipcError: (error, options) => {
		const message = isIpcError(error) ? error.error.message : 'Unexpected error';
		if (isIpcError(error) && error.code === 'INTERNAL') {
			show('error', message, {
				...options,
				action: { label: 'Copy details', onClick: () => void copyDetails(error) }
			});
			return;
		}
		show('error', message, options);
	}
};
