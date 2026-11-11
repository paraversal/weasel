<script lang="ts">
	import { Toaster } from '$lib/components/ui/sonner/index.js';

	let { theme }: { theme: 'light' | 'dark' } = $props();
</script>

<Toaster position="bottom-right" richColors closeButton {theme} />
