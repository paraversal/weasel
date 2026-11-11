import { QueryClient } from '@tanstack/svelte-query';
import { describe, expect, it, vi } from 'vitest';

import type { DomainEvent_Serialize, WeaselEvent_Serialize } from './bindings';
import { events } from './bindings';
import { startEventBridge } from './events';

type Delivered = { payload: WeaselEvent_Serialize };
type Listener = (event: Delivered) => void;

async function bridge(onEvent?: (event: DomainEvent_Serialize) => void) {
	const queryClient = new QueryClient();
	const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
	let listener: Listener | undefined;
	vi.spyOn(events.weaselEvent, 'listen').mockImplementation((async (handler: unknown) => {
		listener = handler as Listener;
		return () => {};
	}) as never);
	const stop = startEventBridge(queryClient, vi.fn(), onEvent);
	await vi.waitFor(() => expect(listener).toBeDefined());
	if (!listener) throw new Error('bridge did not subscribe');
	return {
		invalidate,
		send: (event: DomainEvent_Serialize) => listener?.({ payload: { revision: 1, event } }),
		stop
	};
}

describe('event bridge', () => {
	it('invalidates contact lists but never settings', async () => {
		const { invalidate, send, stop } = await bridge();
		send({ type: 'contactsChanged' });

		expect(invalidate).toHaveBeenCalledWith({ queryKey: ['contacts'] });
		const touched = invalidate.mock.calls.map((call) => call[0]?.queryKey?.[0]);
		expect(touched).not.toContain('settings');
		stop();
	});

	it('passes importProgress to onEvent', async () => {
		const seen: DomainEvent_Serialize[] = [];
		const { send, stop } = await bridge((event) => seen.push(event));
		const progress: DomainEvent_Serialize = {
			type: 'importProgress',
			progressId: 'import-1',
			phase: 'matching',
			processed: 3,
			total: 10
		};
		send(progress);

		expect(seen).toContainEqual(progress);
		stop();
	});
});
