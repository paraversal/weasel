import type { QueryClient, QueryKey } from '@tanstack/svelte-query';

import { events } from './bindings';
import type { AppError, WeaselEvent } from './bindings';
import { keys } from './queries';

type EventType = WeaselEvent['event']['type'];
type DomainEvent = WeaselEvent['event'];

/**
 * Which cached queries an event makes stale. The `satisfies` clause makes a new
 * `DomainEvent` variant a type error until it is listed here.
 */
export const invalidations = {
	settingsChanged: [keys.settings],
	operationFailed: [],
	contactsChanged: [],
	campaignsChanged: [],
	sendBatchesChanged: [],
	importsChanged: [],
	importProgress: [],
	importFinished: []
} satisfies Record<EventType, readonly QueryKey[]>;

/**
 * Feeds backend events into the query cache. Revisions are monotonic per process, so a
 * late or duplicated event never undoes a newer one. Returns the disposer.
 */
export function startEventBridge(
	queryClient: QueryClient,
	onFailed: (error: AppError, event: DomainEvent) => void
): () => void {
	let lastSeen = 0;
	let disposed = false;
	let unlisten: (() => void) | undefined;

	void events.weaselEvent
		.listen(({ payload }) => {
			if (payload.revision <= lastSeen) return;
			lastSeen = payload.revision;

			const event = payload.event;
			if (event.type === 'operationFailed') {
				onFailed(event.error, event);
				return;
			}
			// Unknown variants are ignored on purpose: the backend may be newer.
			const stale = invalidations[event.type] ?? [];
			for (const queryKey of stale) {
				void queryClient.invalidateQueries({ queryKey });
			}
		})
		.then((stop) => {
			if (disposed) stop();
			else unlisten = stop;
		})
		.catch((cause: unknown) => {
			console.error('ipc: event bridge failed to start', cause);
		});

	return () => {
		disposed = true;
		unlisten?.();
		unlisten = undefined;
	};
}
