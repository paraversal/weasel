export const ssr = false;
export const prerender = false;

/**
 * `VITE_IPC_MOCK=1` swaps the Rust backend for an in-memory one. Installing it in `load`
 * guarantees it exists before any component mounts and issues a query; a component's
 * `onMount` would run too late.
 */
export async function load() {
	if (import.meta.env.VITE_IPC_MOCK === '1') {
		const { installMockIpc } = await import('$lib/ipc/mock');
		await installMockIpc();
	}
}
