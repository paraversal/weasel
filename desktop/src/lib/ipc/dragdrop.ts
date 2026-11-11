import { getCurrentWebview } from '@tauri-apps/api/webview';

export type FileDragPhase = 'enter' | 'leave' | 'drop';

/**
 * Native file-drag phases for the drop overlay. Only the visual state comes from here:
 * the dropped file itself arrives as a `sourceDropped` event after the Rust side has
 * allowlisted it. Returns a disposer; outside Tauri it does nothing.
 */
export function onFileDrag(handler: (phase: FileDragPhase, count: number) => void): () => void {
	let unlisten: (() => void) | undefined;
	let disposed = false;
	try {
		void getCurrentWebview()
			.onDragDropEvent(({ payload }) => {
				if (payload.type === 'enter') handler('enter', payload.paths.length);
				else if (payload.type === 'drop') handler('drop', payload.paths.length);
				else if (payload.type === 'leave') handler('leave', 0);
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
