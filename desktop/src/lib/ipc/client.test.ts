import { describe, expect, it, vi } from 'vitest';

import type { AppError } from './bindings';
import { IpcError, call, fieldError, isIpcError, type Result } from './client';

function validationError(field: string, reason: string): AppError {
	return {
		code: 'VALIDATION',
		message: 'Invalid input',
		details: { field, reason },
		retryable: false
	};
}

const internal: AppError = {
	code: 'INTERNAL',
	message: 'Unexpected error',
	details: { logId: 'x' },
	retryable: false
};

describe('call', () => {
	it('resolves with the payload of an ok result', async () => {
		const pending: Promise<Result<number>> = Promise.resolve({ status: 'ok', data: 42 });
		await expect(call(pending)).resolves.toBe(42);
	});

	it('rejects with IpcError carrying the backend error', async () => {
		const expected = validationError('theme', 'unknown');
		const pending: Promise<Result<number>> = Promise.resolve({ status: 'error', error: expected });
		const error = await call(pending).catch((cause: unknown) => cause);
		expect(error).toBeInstanceOf(IpcError);
		expect(isIpcError(error) && error.error).toBe(expected);
	});

	it('converts a framework rejection into INTERNAL', async () => {
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
		const pending = Promise.reject(new Error('command not found')) as Promise<Result<number>>;
		await expect(call(pending)).rejects.toMatchObject({ code: 'INTERNAL', retryable: false });
		spy.mockRestore();
	});

	it('converts a non-AppError error value into INTERNAL', async () => {
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
		const pending = Promise.resolve({
			status: 'error',
			error: 'denied'
		}) as unknown as Promise<Result<number>>;
		await expect(call(pending)).rejects.toMatchObject({ code: 'INTERNAL' });
		spy.mockRestore();
	});

	it('keeps the original error instance identity', async () => {
		const error = new IpcError(internal);
		const pending: Promise<Result<never>> = Promise.resolve({ status: 'error', error: internal });
		await expect(call(pending)).rejects.toBeInstanceOf(IpcError);
		expect(isIpcError(error)).toBe(true);
		expect(isIpcError(new Error('plain'))).toBe(false);
	});
});

describe('fieldError', () => {
	it('returns the reason for the matching field', () => {
		expect(fieldError(new IpcError(validationError('theme', 'unknown theme')), 'theme')).toBe(
			'unknown theme'
		);
	});

	it('ignores other fields, other codes and untagged errors', () => {
		expect(fieldError(new IpcError(validationError('theme', 'nope')), 'flags')).toBeUndefined();
		expect(fieldError(new IpcError(internal), 'theme')).toBeUndefined();
		expect(fieldError(new Error('plain'), 'theme')).toBeUndefined();
	});

	it('ignores validation errors without a details object', () => {
		const error: AppError = { code: 'VALIDATION', message: 'bad', details: null, retryable: false };
		expect(fieldError(new IpcError(error), 'theme')).toBeUndefined();
	});
});
