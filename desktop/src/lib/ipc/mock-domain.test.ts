import { describe, expect, it, vi } from 'vitest';

import type { DomainEvent_Serialize, PageDto, PreparedSendDto } from './bindings';
import { createDomainHandlers } from './mock-domain';
import { createMockData } from './mock-data';

function expectFailure(call: () => unknown, code: string): void {
	try {
		call();
		expect.unreachable('expected the handler to throw');
	} catch (error) {
		expect(error).toMatchObject({ code });
	}
}

describe('mock domain', () => {
	it('paginates contacts without losing the total', () => {
		const handlers = createDomainHandlers();
		const list = handlers.contacts_list;
		if (!list) throw new Error('contacts_list handler missing');
		const first = list({
			request: { statuses: ['active', 'removed', 'archived'], limit: 10, offset: 0 }
		}) as PageDto<unknown>;
		const second = list({
			request: { statuses: ['active', 'removed', 'archived'], limit: 10, offset: 10 }
		}) as PageDto<unknown>;

		expect(first.total).toBe(60);
		expect(first.items).toHaveLength(10);
		expect(second.total).toBe(60);
		expect(second.offset).toBe(10);
		expect(second.items).toHaveLength(10);
		expect(second.items[0]).not.toEqual(first.items[0]);
	});

	it('rejects a stale contact update with CONFLICT', () => {
		const handlers = createDomainHandlers();
		const update = handlers.contacts_update;
		if (!update) throw new Error('contacts_update handler missing');
		expectFailure(
			() => update({ request: { id: 1, version: 999, firstName: 'Changed' } }),
			'CONFLICT'
		);
	});

	it('prepares a send, emits sendBatchesChanged and lists the new batch', () => {
		const events: DomainEvent_Serialize[] = [];
		const handlers = createDomainHandlers(createMockData(), (event) => events.push(event));
		const before = handlers.send_batches_list?.({ request: {} }) as PageDto<{ id: number }>;
		const prepare = handlers.campaigns_prepare_send;
		if (!prepare) throw new Error('campaigns_prepare_send handler missing');

		const prepared = prepare({ request: { campaignId: 2 } }) as PreparedSendDto;
		expect(prepared.recipientCount).toBeGreaterThan(0);
		expect(prepared.emails).toHaveLength(prepared.recipientCount);
		expect(events).toContainEqual({ type: 'sendBatchesChanged' });

		const after = handlers.send_batches_list?.({ request: {} }) as PageDto<{ id: number }>;
		expect(after.total).toBe(before.total + 1);
		expect(after.items.some((batch) => batch.id === prepared.batchId)).toBe(true);
	});

	it('previews and applies a deterministic import', () => {
		const handlers = createDomainHandlers();
		const preview = handlers.imports_preview?.({
			request: { path: '/mock/roster.xlsx', scope: 'roster' }
		}) as {
			batch: {
				id: number;
				summary: { added: number; updated: number; removed: number; conflicts: number };
			};
			conflicts: unknown[];
		};
		expect(preview.batch.summary).toMatchObject({ added: 5, updated: 2, removed: 1, conflicts: 1 });
		expect(preview.conflicts).toHaveLength(1);

		expectFailure(
			() => handlers.imports_apply?.({ request: { batchId: preview.batch.id, resolutions: [] } }),
			'INVALID_STATE'
		);

		const applied = handlers.imports_apply?.({
			request: {
				batchId: preview.batch.id,
				resolutions: [{ row: 10, action: 'createNew' }]
			}
		}) as { status: string; summary: { added: number } };
		expect(applied.status).toBe('applied');
		expect(applied.summary.added).toBe(5);
	});

	it('keeps a spy satisfied when an event reaches the bridge', () => {
		const emit = vi.fn();
		const handlers = createDomainHandlers(createMockData(), emit);
		handlers.campaigns_prepare_send?.({ request: { campaignId: 2 } });
		expect(emit).toHaveBeenCalled();
	});
});
