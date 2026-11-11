<script lang="ts">
	import { Badge, type BadgeVariant } from '$lib/components/ui/badge/index.js';

	let { status }: { status: string } = $props();

	const styles: Record<string, { label: string; variant: BadgeVariant }> = {
		active: { label: 'Active', variant: 'secondary' },
		removed: { label: 'Removed', variant: 'destructive' },
		archived: { label: 'Archived', variant: 'outline' },
		draft: { label: 'Draft', variant: 'outline' },
		applied: { label: 'Applied', variant: 'secondary' },
		discarded: { label: 'Discarded', variant: 'outline' },
		failed: { label: 'Failed', variant: 'destructive' }
	};

	const style = $derived(styles[status] ?? { label: status, variant: 'outline' as BadgeVariant });
</script>

<Badge variant={style.variant}>{style.label}</Badge>
