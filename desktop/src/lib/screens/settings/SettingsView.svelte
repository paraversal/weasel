<script lang="ts">
	import GeneralSettings from './GeneralSettings.svelte';
	import WordingSettings from './WordingSettings.svelte';
	import InvalidAddressSettings from '$lib/screens/email-checks/InvalidAddressSettings.svelte';
</script>

<div class="flex flex-col gap-6">
	<h1 class="text-xl font-semibold">Settings</h1>

	<section class="border-border-subtle rounded-lg border bg-surface-raised p-4">
		<h2 class="mb-3 text-sm font-semibold">General</h2>
		<GeneralSettings />
	</section>

	<section class="border-border-subtle rounded-lg border bg-surface-raised p-4">
		<h2 class="mb-3 text-sm font-semibold">Wording</h2>
		<WordingSettings />
	</section>

	<section class="border-border-subtle rounded-lg border bg-surface-raised p-4">
		<h2 class="mb-3 text-sm font-semibold">Invalid addresses</h2>
		<InvalidAddressSettings />
	</section>
</div>
