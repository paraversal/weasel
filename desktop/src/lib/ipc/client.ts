import type { AppError } from './bindings';

/**
 * A rejected command, always carrying the backend's structured error. Components switch
 * on `code`; `message` is display-only.
 */
export class IpcError extends Error {
	constructor(public readonly error: AppError) {
		super(error.message);
		this.name = 'IpcError';
	}

	get code(): AppError['code'] {
		return this.error.code;
	}

	get retryable(): boolean {
		return this.error.retryable;
	}
}

/** Mirrors the result shape tauri-specta emits in `ErrorHandlingMode::Result`. */
export type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: AppError };

export function isAppError(value: unknown): value is AppError {
	if (typeof value !== 'object' || value === null) return false;
	const candidate = value as Record<string, unknown>;
	return (
		typeof candidate.code === 'string' &&
		typeof candidate.message === 'string' &&
		typeof candidate.retryable === 'boolean'
	);
}

/**
 * Framework-level failures (unknown command, ACL denial, a panicking command) reject
 * instead of resolving to a result, and carry no `AppError`. Surface them as INTERNAL so
 * callers only ever handle one error type.
 */
function internalError(cause: unknown): AppError {
	console.error('ipc: unexpected failure', cause);
	return {
		code: 'INTERNAL',
		message: 'Unexpected error. Check the logs for details.',
		details: null,
		retryable: false
	};
}

/**
 * Unwrap a command result. Resolves with the payload, rejects with `IpcError` for both
 * structured errors and framework failures.
 */
export async function call<T>(pending: Promise<Result<T>>): Promise<T> {
	let result: Result<T>;
	try {
		result = await pending;
	} catch (cause) {
		throw new IpcError(internalError(cause));
	}
	if (result.status === 'ok') return result.data;
	throw new IpcError(isAppError(result.error) ? result.error : internalError(result.error));
}

export function isIpcError(error: unknown): error is IpcError {
	return error instanceof IpcError;
}

/**
 * Per-field validation message, so a form can show `fieldError(error, "theme")` next to
 * the input instead of a banner.
 */
export function fieldError(error: unknown, field: string): string | undefined {
	if (!isIpcError(error) || error.code !== 'VALIDATION') return undefined;
	const details = error.error.details;
	if (typeof details !== 'object' || details === null || Array.isArray(details)) {
		return undefined;
	}
	const record = details as Record<string, unknown>;
	return record.field === field && typeof record.reason === 'string' ? record.reason : undefined;
}
