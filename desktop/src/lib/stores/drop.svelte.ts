import type { SourceFileDto } from '$lib/ipc/bindings';

/**
 * A file dropped on the window, waiting for the import wizard to pick it up. Set by the
 * event bridge in the root layout; the wizard takes it and clears it.
 */
export const droppedFile = $state<{ source: SourceFileDto | null }>({ source: null });

export function takeDroppedFile(): SourceFileDto | null {
	const source = droppedFile.source;
	droppedFile.source = null;
	return source;
}
