<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';
	import { flip } from 'svelte/animate';
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import Field from '$lib/components/app/Field.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { commands } from '$lib/ipc/bindings';
	import type { EmailFlagDto, InvalidRuleDto } from '$lib/ipc/bindings';
	import { call, fieldError } from '$lib/ipc/client';
	import { emailCheckQueries } from '$lib/ipc/queries';
	import { useTerm } from '$lib/stores/term.svelte';
	import { toast } from '$lib/stores/toasts.svelte';
	import { reportError } from '$lib/utils/errors';
	import { formatDate, plural } from '$lib/utils/format';
	import { flipParams } from '$lib/utils/motion';

	import { markEmail } from './actions';

	const term = useTerm();
	const rules = createQuery(() => emailCheckQueries.rules());
	const flags = createQuery(() => emailCheckQueries.flags());

	let pattern = $state('');
	let debounced = $state('');
	let error = $state<unknown>(null);
	let adding = $state(false);

	// Preview as the user types, without a query per keystroke.
	$effect(() => {
		const next = pattern.trim();
		const timer = setTimeout(() => (debounced = next), 250);
		return () => clearTimeout(timer);
	});

	const preview = createQuery(() => ({
		...emailCheckQueries.preview(debounced),
		enabled: debounced.length > 0,
		retry: false
	}));

	async function add(event: SubmitEvent) {
		event.preventDefault();
		adding = true;
		error = null;
		try {
			const rule = await call(commands.emailRulesAdd({ pattern }));
			pattern = '';
			toast.success(
				rule.matches > 0
					? `Rule added. ${plural(rule.matches, term.one, term.many)} on hold.`
					: 'Rule added'
			);
		} catch (err) {
			error = err;
		} finally {
			adding = false;
		}
	}

	async function remove(rule: InvalidRuleDto) {
		try {
			await call(commands.emailRulesDelete({ id: rule.id }));
			toast.success(`Removed ${rule.pattern}`, {
				action: {
					label: 'Undo',
					onClick: () =>
						void call(commands.emailRulesAdd({ pattern: rule.pattern })).catch(reportError)
				}
			});
		} catch (err) {
			reportError(err);
		}
	}

	function verdict(flag: EmailFlagDto): string {
		if (!flag.invalid) return 'Marked valid';
		return flag.bounced ? 'Bounced' : 'Marked invalid';
	}
</script>

<div class="flex flex-col gap-6">
	<div class="flex flex-col gap-3">
		<p class="text-sm text-muted-foreground">
			Addresses that match a rule are put on hold automatically, including in future imports. People
			with an address on hold stay in your list but are left out of sends until the address changes.
			Use <code class="rounded bg-muted px-1">*</code> for any run of characters and
			<code class="rounded bg-muted px-1">?</code> for one, e.g.
			<code class="rounded bg-muted px-1">*@temp.uni.edu</code>.
		</p>

		<form class="flex max-w-xl flex-col gap-2" onsubmit={add}>
			<Field id="rule-pattern" label="New rule" error={fieldError(error, 'pattern')}>
				<div class="flex gap-2">
					<Input
						id="rule-pattern"
						bind:value={pattern}
						placeholder="*@temp.uni.edu"
						autocomplete="off"
						spellcheck={false}
						class="font-mono"
					/>
					<Button type="submit" disabled={adding || pattern.trim() === ''}>Add rule</Button>
				</div>
			</Field>
			{#if debounced && !error}
				<p class="min-h-5 text-xs text-muted-foreground" aria-live="polite">
					{#if preview.isError}
						<span class="text-destructive"
							>{fieldError(preview.error, 'pattern') ?? preview.error.message}</span
						>
					{:else if preview.data}
						{#if preview.data.matches === 0}
							Matches nobody right now; it will still catch future imports.
						{:else}
							Matches {plural(preview.data.matches, term.one, term.many)}:
							{preview.data.examples.join(', ')}{preview.data.matches > preview.data.examples.length
								? ', …'
								: ''}
						{/if}
					{/if}
				</p>
			{/if}
		</form>

		{#if rules.data && rules.data.length > 0}
			<ul class="flex max-w-xl flex-col divide-y rounded-lg border">
				{#each rules.data as rule (rule.id)}
					<li class="flex items-center gap-3 px-3 py-2 text-sm" animate:flip={flipParams()}>
						<code class="flex-1 truncate font-mono">{rule.pattern}</code>
						<span class="text-muted-foreground">{plural(rule.matches, term.one, term.many)}</span>
						<Button
							size="icon-sm"
							variant="ghost"
							aria-label="Remove rule {rule.pattern}"
							onclick={() => remove(rule)}
						>
							<TrashIcon />
						</Button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<div class="flex flex-col gap-2">
		<h3 class="text-sm font-medium">Single addresses</h3>
		{#if flags.data && flags.data.length > 0}
			<ul class="flex max-w-xl flex-col divide-y rounded-lg border">
				{#each flags.data as flag (flag.email)}
					<li class="flex items-center gap-3 px-3 py-2 text-sm" animate:flip={flipParams()}>
						<span class="min-w-0 flex-1 truncate">{flag.email}</span>
						<span
							class={flag.invalid ? 'text-amber-700 dark:text-amber-300' : 'text-muted-foreground'}
						>
							{verdict(flag)} · {formatDate(flag.createdAt)}
						</span>
						<Button size="sm" variant="ghost" onclick={() => markEmail(flag.email, 'default')}>
							Forget
						</Button>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="text-sm text-muted-foreground">
				None yet. Mark an address invalid from a {term.one}'s page, or mark a recipient as bounced
				in the send history.
			</p>
		{/if}
	</div>
</div>
