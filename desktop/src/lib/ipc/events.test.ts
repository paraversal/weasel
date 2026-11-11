import type { QueryClient } from '@tanstack/svelte-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { WeaselEvent } from './bindings';
import { invalidations, startEventBridge } from './events';
import { keys } from './queries';

const mocks = vi.hoisted(() => ({
	listeners: [] as Array<(event: { payload: WeaselEvent }) => void>,
	unlisten: vi.fn()
}));

vi.mock('./bindings', () => ({
	events: {
		weaselEvent: {
			listen: vi.fn(async (callback: (event: { payload: WeaselEvent }) => void) => {
				mocks.listeners.push(callback);
				return mocks.unlisten;
			})
		}
	}
}));

function emit(payload: WeaselEvent) {
	for (const listener of mocks.listeners) listener({ payload });
}

function fakeQueryClient() {
	return { invalidateQueries: vi.fn(() => Promise.resolve()) };
}

async function startBridge() {
	const queryClient = fakeQueryClient();
	const onFailed = vi.fn();
	const stop = startEventBridge(queryClient as unknown as QueryClient, onFailed);
	await vi.waitFor(() => expect(mocks.listeners).toHaveLength(1));
	return { queryClient, onFailed, stop };
}

beforeEach(() => {
	mocks.listeners = [];
	mocks.unlisten.mockClear();
});

describe('invalidations', () => {
	it('covers every domain event type', () => {
		expect(Object.keys(invalidations).sort()).toEqual(['operationFailed', 'settingsChanged']);
	});
});

describe('startEventBridge', () => {
	it('invalidates the mapped keys for settingsChanged', async () => {
		const { queryClient } = await startBridge();

		emit({ revision: 1, event: { type: 'settingsChanged', keys: ['theme'] } });

		expect(queryClient.invalidateQueries).toHaveBeenCalledTimes(1);
		expect(queryClient.invalidateQueries).toHaveBeenCalledWith({ queryKey: keys.settings });
	});

	it('drops events whose revision is not newer', async () => {
		const { queryClient } = await startBridge();

		emit({ revision: 2, event: { type: 'settingsChanged', keys: [] } });
		emit({ revision: 2, event: { type: 'settingsChanged', keys: [] } });
		emit({ revision: 1, event: { type: 'settingsChanged', keys: [] } });

		expect(queryClient.invalidateQueries).toHaveBeenCalledTimes(1);
	});

	it('ignores unknown variants from a newer backend', async () => {
		const { queryClient, onFailed } = await startBridge();

		emit({ revision: 1, event: { type: 'laterThing' } } as unknown as WeaselEvent);

		expect(queryClient.invalidateQueries).not.toHaveBeenCalled();
		expect(onFailed).not.toHaveBeenCalled();
	});

	it('reports operationFailed without invalidating anything', async () => {
		const { queryClient, onFailed } = await startBridge();
		const error = {
			code: 'IO' as const,
			message: 'disk full',
			details: null,
			retryable: true
		};

		emit({ revision: 1, event: { type: 'operationFailed', operation: 'import', error } });

		expect(onFailed).toHaveBeenCalledWith(error, {
			type: 'operationFailed',
			operation: 'import',
			error
		});
		expect(queryClient.invalidateQueries).not.toHaveBeenCalled();
	});

	it('unlistens when disposed', async () => {
		const { stop } = await startBridge();

		stop();

		expect(mocks.unlisten).toHaveBeenCalledTimes(1);
	});
});
