<script lang="ts">
	import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query';
	import { onMount } from 'svelte';

	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { isIpcError } from '$lib/ipc/client';
	import { startEventBridge } from '$lib/ipc/events';
	import BootGate from '$lib/shell/BootGate.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: 30_000,
				refetchOnWindowFocus: false,
				// Only transport-ish failures are worth retrying; a validation error never is.
				retry: (failureCount, error) => isIpcError(error) && error.retryable && failureCount < 2
			}
		}
	});

	onMount(() => {
		const stopBridge = startEventBridge(queryClient, (error) => {
			console.error(`ipc: operation failed (${error.code})`, error.message);
		});
		return stopBridge;
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Weasel</title>
</svelte:head>

<QueryClientProvider client={queryClient}>
	<BootGate>
		{@render children()}
	</BootGate>
</QueryClientProvider>
