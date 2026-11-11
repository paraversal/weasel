import { getCurrentWebview } from '@tauri-apps/api/webview';

export type FileDragPhase = 'enter' | 'over' | 'leave' | 'drop';

export interface FileDrag {
	phase: FileDragPhase;
	/** Dragged file paths; empty on `over` and `leave`. */
	paths: string[];
	/** Cursor in CSS pixels relative to the webview, when the platform reports it. */
	x: number | null;
	y: number | null;
}

/**
 * Native file-drag phases for the drop overlay. Only the visual state comes from here:
 * the dropped file itself arrives as a `sourceDropped` event after the Rust side has
 * allowlisted it. Returns a disposer; outside Tauri it does nothing.
 */
export function onFileDrag(handler: (drag: FileDrag) => void): () => void {
	let unlisten: (() => void) | undefined;
	let disposed = false;
	const scale = () => (typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1);
	try {
		void getCurrentWebview()
			.onDragDropEvent(({ payload }) => {
				if (payload.type === 'leave') {
					handler({ phase: 'leave', paths: [], x: null, y: null });
					return;
				}
				// Tauri reports physical pixels; the overlay works in CSS pixels.
				const x = payload.position.x / scale();
				const y = payload.position.y / scale();
				const paths = payload.type === 'over' ? [] : payload.paths;
				handler({ phase: payload.type, paths, x, y });
			})
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch((cause: unknown) => console.warn('ipc: drag-drop listener unavailable', cause));
	} catch (cause) {
		console.warn('ipc: drag-drop listener unavailable', cause);
	}
	return () => {
		disposed = true;
		unlisten?.();
	};
}

const IMPORTABLE = ['xlsx', 'xls', 'ods', 'csv'];

/** Whether any dragged path has an extension the importer reads. */
export function hasImportable(paths: string[]): boolean {
	return paths.some((path) => {
		const extension = path.split('.').pop()?.toLowerCase() ?? '';
		return IMPORTABLE.includes(extension);
	});
}
