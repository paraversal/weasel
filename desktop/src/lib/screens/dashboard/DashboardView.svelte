<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import CountUp from '$lib/components/app/CountUp.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import {
		campaignQueries,
		contactQueries,
		importQueries,
		sendBatchQueries
	} from '$lib/ipc/queries';
	import { formatDateTime, formatNumber } from '$lib/utils/format';
	import { href } from '$lib/utils/paths';
	import { useTerm } from '$lib/stores/term.svelte';

	import AppStatusWidget from './AppStatusWidget.svelte';

	const contacts = createQuery(() => contactQueries.stats());
	const term = useTerm();
	const campaigns = createQuery(() => campaignQueries.list({ limit: 1 }));
	const lastImport = createQuery(() => importQueries.list({ status: 'applied', limit: 1 }));
	const lastSend = createQuery(() => sendBatchQueries.list({ limit: 1 }));

	const importItem = $derived(lastImport.data?.items[0]);
	const sendItem = $derived(lastSend.data?.items[0]);
</script>

<div class="flex flex-col gap-4">
	<div class="flex flex-wrap items-center justify-between gap-3">
		<h1 class="text-xl font-semibold">Dashboard</h1>
		<div class="flex gap-2">
			<Button href={href('/imports/new')}>Import list</Button>
			<Button variant="outline" href={href('/campaigns?new=1')}>New campaign</Button>
			<Button variant="outline" href={href('/freshies?new=1')}>Add {term.one}</Button>
		</div>
	</div>

	<div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
		<a
			href={href('/freshies')}
			class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-4 transition-all duration-200 hover:-translate-y-0.5 hover:bg-muted/50 hover:shadow-md"
			style="--i: 0"
		>
			<p class="text-xs text-muted-foreground">Active {term.many}</p>
			<p class="text-2xl font-semibold tabular-nums">
				{#if contacts.data}<CountUp value={contacts.data.active} />{:else}…{/if}
			</p>
			{#if contacts.data && contacts.data.removed > 0}
				<p class="text-xs text-muted-foreground">{formatNumber(contacts.data.removed)} removed</p>
			{/if}
		</a>
		<a
			href={href('/campaigns')}
			class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-4 transition-all duration-200 hover:-translate-y-0.5 hover:bg-muted/50 hover:shadow-md"
			style="--i: 2"
		>
			<p class="text-xs text-muted-foreground">Campaigns</p>
			<p class="text-2xl font-semibold tabular-nums">
				{#if campaigns.data}<CountUp value={campaigns.data.total} />{:else}…{/if}
			</p>
		</a>
		<a
			href={importItem ? href(`/imports/${importItem.id}`) : href('/imports')}
			class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-4 transition-all duration-200 hover:-translate-y-0.5 hover:bg-muted/50 hover:shadow-md"
			style="--i: 4"
		>
			<p class="text-xs text-muted-foreground">Last import</p>
			<p class="truncate text-sm font-semibold">{importItem?.sourceName ?? 'None yet'}</p>
			{#if importItem}
				<p class="text-xs text-muted-foreground">
					{formatDateTime(importItem.appliedAt ?? importItem.createdAt)}
				</p>
			{/if}
		</a>
		<a
			href={sendItem ? href(`/history/${sendItem.id}`) : href('/history')}
			class="border-border-subtle stagger animate-rise rounded-lg border bg-surface-raised p-4 transition-all duration-200 hover:-translate-y-0.5 hover:bg-muted/50 hover:shadow-md"
			style="--i: 6"
		>
			<p class="text-xs text-muted-foreground">Last send</p>
			<p class="truncate text-sm font-semibold">{sendItem?.campaignName ?? 'None yet'}</p>
			{#if sendItem}
				<p class="text-xs text-muted-foreground">
					{formatDateTime(sendItem.createdAt)} · {formatNumber(sendItem.recipientCount)} recipients
				</p>
			{/if}
		</a>
	</div>

	<div class="grid gap-3 lg:grid-cols-2">
		<AppStatusWidget />
	</div>
</div>
