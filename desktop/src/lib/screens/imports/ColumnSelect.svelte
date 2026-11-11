<script lang="ts">
	let {
		id,
		label,
		headers,
		value = $bindable()
	}: { id: string; label: string; headers: string[]; value: number | null } = $props();
</script>

<label for={id} class="flex flex-col gap-1.5 text-sm font-medium">
	{label}
	<select
		{id}
		class="h-8 rounded-lg border border-input bg-transparent px-2 text-sm dark:bg-input/30"
		value={value ?? ''}
		onchange={(event) => {
			const raw = event.currentTarget.value;
			value = raw === '' ? null : Number(raw);
		}}
	>
		<option value="">Choose a column…</option>
		{#each headers as header, index (index)}
			<option value={index}>{header || `Column ${index + 1}`}</option>
		{/each}
	</select>
</label>
