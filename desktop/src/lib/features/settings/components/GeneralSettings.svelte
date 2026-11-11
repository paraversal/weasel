<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import * as RadioGroup from '$lib/components/ui/radio-group/index.js';
	import { settingsQuery } from '$lib/ipc/queries';
	import { useTheme, type ThemeMode } from '$lib/stores/theme.svelte';

	const theme = useTheme();
	const settings = createQuery(() => settingsQuery());

	const modes: Array<{ value: ThemeMode; label: string; hint: string }> = [
		{ value: 'system', label: 'System', hint: 'Follow the operating system appearance.' },
		{ value: 'light', label: 'Light', hint: 'Always use the light palette.' },
		{ value: 'dark', label: 'Dark', hint: 'Always use the dark palette.' }
	];
</script>

<div class="flex flex-col gap-4">
	<div class="flex flex-col gap-2">
		<p class="text-sm font-medium">Theme</p>
		<RadioGroup.Root
			value={theme.mode}
			onValueChange={(value) => value && theme.setMode(value as ThemeMode)}
			class="gap-2"
			aria-label="Theme"
		>
			{#each modes as mode (mode.value)}
				<label
					class="border-border-subtle flex cursor-pointer items-start gap-3 rounded-md border p-3 hover:bg-muted/50"
				>
					<RadioGroup.Item value={mode.value} class="mt-0.5" />
					<span class="flex flex-col gap-0.5">
						<span class="text-sm">{mode.label}</span>
						<span class="text-xs text-muted-foreground">{mode.hint}</span>
					</span>
				</label>
			{/each}
		</RadioGroup.Root>
	</div>

	{#if import.meta.env.DEV}
		<div class="flex flex-col gap-1">
			<p class="text-sm font-medium">Feature flags</p>
			{#if settings.data}
				<pre
					class="border-border-subtle overflow-auto rounded-md border bg-muted/40 p-2 font-mono text-xs">{JSON.stringify(
						settings.data.featureFlags,
						null,
						2
					)}</pre>
			{:else}
				<p class="text-xs text-muted-foreground">Loading…</p>
			{/if}
		</div>
	{/if}
</div>
