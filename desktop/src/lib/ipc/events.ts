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
	contactsChanged: [
		keys.contacts.all,
		keys.campaigns.all,
		keys.searchAll,
		keys.activity.all,
		keys.app.health
	],
	campaignsChanged: [keys.campaigns.all, keys.searchAll, keys.activity.all],
	sendBatchesChanged: [
		keys.sendBatches.all,
		keys.campaigns.all,
		keys.contacts.detailAll,
		keys.searchAll,
		keys.activity.all
	],
	importsChanged: [keys.imports.all, keys.app.health, keys.activity.all],
	emailChecksChanged: [
		keys.emailChecks.all,
		keys.contacts.all,
		keys.campaigns.all,
		keys.sendBatches.all,
		keys.imports.all,
		keys.activity.all
	],
	importProgress: [],
	importFinished: [keys.imports.all, keys.contacts.all, keys.campaigns.all, keys.app.health],
	sourceDropped: [],
	sourceRejected: []
} satisfies Record<EventType, readonly QueryKey[]>;

/**
 * Feeds backend events into the query cache. Revisions are monotonic per process, so a
 * late or duplicated event never undoes a newer one. `onEvent` receives every accepted
 * event after invalidation, which is where the import progress store builds its state.
 * Returns the disposer.
 */
export function startEventBridge(
	queryClient: QueryClient,
	onFailed: (error: AppError, event: DomainEvent) => void,
	onEvent?: (event: DomainEvent) => void
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
				onEvent?.(event);
				return;
			}
			// Unknown variants are ignored on purpose: the backend may be newer.
			const stale = invalidations[event.type] ?? [];
			for (const queryKey of stale) {
				void queryClient.invalidateQueries({ queryKey });
			}
			onEvent?.(event);
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
