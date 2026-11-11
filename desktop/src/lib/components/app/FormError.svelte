<script lang="ts">
	import { isIpcError } from '$lib/ipc/client';

	/** Errors that no single field owns (conflicts, state, IO); field errors render inline. */
	let { error }: { error: unknown } = $props();

	const message = $derived.by(() => {
		if (!error) return undefined;
		if (!isIpcError(error)) return 'Unexpected error.';
		const details = error.error.details as Record<string, unknown> | null | undefined;
		if (error.code === 'VALIDATION' && details && typeof details.field === 'string') {
			return undefined;
		}
		return error.error.message;
	});
</script>

{#if message}
	<p class="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
		{message}
	</p>
{/if}
