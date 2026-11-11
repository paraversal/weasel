<script lang="ts">
	import MailXIcon from '@lucide/svelte/icons/mail-x';

	import type { EmailHoldDto } from '$lib/ipc/bindings';
	import { Badge } from '$lib/components/ui/badge/index.js';

	import { holdLabel } from './labels';

	let { hold, label = 'Invalid email' }: { hold?: EmailHoldDto | null; label?: string } = $props();
</script>

<Badge
	variant="outline"
	class="animate-pop gap-1 border-amber-500/50 bg-amber-500/10 text-amber-700 dark:text-amber-300"
	title={hold ? holdLabel(hold) : 'This address is on hold'}
>
	<MailXIcon class="size-3" aria-hidden="true" />
	{label}
</Badge>
