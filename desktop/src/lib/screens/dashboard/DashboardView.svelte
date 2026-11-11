<script lang="ts">
	import AppStatusWidget from './AppStatusWidget.svelte';
</script>

<div class="flex flex-col gap-4">
	<h1 class="text-xl font-semibold">Dashboard</h1>

	<div class="grid gap-3 lg:grid-cols-2">
		<AppStatusWidget />
	</div>
</div>
