#!/usr/bin/env node
/**
 * Feature generator: scaffolds a vertical slice (manifest → route → component → command →
 * domain module) and registers it through the `@gen:` markers.
 *
 * Zero dependencies by design: this runs before `pnpm install` in CI and must never
 * drift from the repository it edits.
 *
 *   pnpm gen feature <name> [--nav] [--command] [--dry-run] [--force-none] [--root <dir>]
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';

const NAME_PATTERN = /^[a-z][a-z0-9-]{1,30}$/;

const HELP = `Usage: pnpm gen feature <name> [options]

  <name>          lowercase kebab-case, e.g. favorites (${NAME_PATTERN})

Options:
  --nav           add a sidebar entry and a "Go to …" palette command
  --command       add the IPC command, Rust descriptor and a query-backed panel
  --dry-run       print the plan without writing anything
  --force-none    ignore --nav/--command (generate the bare module)
  --root <dir>    repository root to generate into (tests use a temp tree)
  -h, --help      show this help
`;

function fail(message) {
	console.error(`gen-feature: ${message}`);
	process.exit(1);
}

const parsed = parseArgs({
	args: process.argv.slice(2),
	allowPositionals: true,
	options: {
		nav: { type: 'boolean', default: false },
		command: { type: 'boolean', default: false },
		'dry-run': { type: 'boolean', default: false },
		'force-none': { type: 'boolean', default: false },
		root: { type: 'string' },
		help: { type: 'boolean', short: 'h', default: false }
	}
});

if (parsed.values.help) {
	console.log(HELP);
	process.exit(0);
}

const [group, name] = parsed.positionals;
if (group !== 'feature' || !name) {
	console.error(HELP);
	process.exit(1);
}
if (!NAME_PATTERN.test(name)) {
	fail(`"${name}" is not a valid feature name; expected ${NAME_PATTERN}`);
}

const withNav = parsed.values.nav && !parsed.values['force-none'];
const withCommand = parsed.values.command && !parsed.values['force-none'];
const dryRun = parsed.values['dry-run'];
const root = parsed.values.root
	? resolve(parsed.values.root)
	: resolve(dirname(fileURLToPath(import.meta.url)), '..');

const id = name;
const snake = id.replaceAll('-', '_');
const pascal = id
	.split('-')
	.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
	.join('');
const camel = pascal.charAt(0).toLowerCase() + pascal.slice(1);
const title = id
	.split('-')
	.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
	.join(' ');

const commandsMod = join(root, 'desktop/src-tauri/src/commands/mod.rs');
const coreLib = join(root, 'crates/weasel-core/src/lib.rs');

function requireMarkers(file, markers) {
	if (!existsSync(file)) fail(`missing ${file}`);
	const text = readFileSync(file, 'utf8');
	for (const marker of markers) {
		if (!text.includes(marker)) fail(`${file} is missing the marker ${marker}`);
	}
	return text;
}

requireMarkers(commandsMod, ['// @gen:modules', '// @gen:descriptors', '// @gen:commands']);
if (withCommand) requireMarkers(coreLib, ['// @gen:modules']);

const files = new Map();

files.set(
	join(root, `desktop/src/lib/features/${id}/manifest.ts`),
	withNav
		? `import type { FeatureManifest } from '$lib/features/types';

export default {
	id: '${id}',
	title: '${title}',
	description: 'TODO: one line on what this feature is for.',
	order: 50,
	nav: [{ label: '${title}', href: '/${id}', icon: 'box', order: 50 }],
	commands: [
		{
			id: '${id}.go',
			title: 'Go to ${title}',
			group: '${title}',
			run: (context) => context.goto('/${id}')
		}
	]
} satisfies FeatureManifest;
`
		: `import type { FeatureManifest } from '$lib/features/types';

export default {
	id: '${id}',
	title: '${title}',
	description: 'TODO: one line on what this feature is for.',
	order: 50
} satisfies FeatureManifest;
`
);

files.set(
	join(root, `desktop/src/lib/features/${id}/README.md`),
	`# ${title}

TODO: what this feature is for and who owns it.

- Commands: ${withCommand ? `\`${snake}_list\`` : 'none yet'}
- Checklist: [05 § Extensibility review checklist](../../../../../docs/rewrite/05-extensibility.md#extensibility-review-checklist)
`
);

files.set(
	join(root, `desktop/src/lib/features/${id}/components/${pascal}Panel.svelte`),
	withCommand
		? `<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';

	import { commands } from '$lib/ipc/bindings';
	import { call } from '$lib/ipc/client';

	// TODO: replace the list with the feature's real read model.
	const items = createQuery(() => ({
		queryKey: ['${id}'],
		queryFn: () => call(commands.${camel}List())
	}));
</script>

<div class="flex flex-col gap-3">
	<h1 class="text-xl font-semibold">${title}</h1>

	{#if items.isPending}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else if items.isError}
		<p class="text-sm text-danger">{items.error.message}</p>
	{:else if (items.data ?? []).length === 0}
		<p class="text-sm text-muted-foreground">Nothing here yet.</p>
	{:else}
		<ul class="list-disc pl-5 text-sm">
			{#each items.data ?? [] as item}
				<li>{item}</li>
			{/each}
		</ul>
	{/if}
</div>
`
		: `<div class="flex flex-col gap-3">
	<h1 class="text-xl font-semibold">${title}</h1>
	<p class="text-sm text-muted-foreground">TODO: build the ${title} screen here.</p>
</div>
`
);

if (withNav) {
	files.set(
		join(root, `desktop/src/routes/${id}/+page.svelte`),
		`<script lang="ts">
	import ${pascal}Panel from '$lib/features/${id}/components/${pascal}Panel.svelte';
</script>

<${pascal}Panel />
`
	);
}

if (withCommand) {
	files.set(
		join(root, `desktop/src-tauri/src/commands/${snake}.rs`),
		`//! ${title} commands. TODO: back these with a weasel-core service.

use weasel_core::FeatureDescriptor;

use crate::error::AppError;

pub fn descriptor() -> FeatureDescriptor {
	FeatureDescriptor {
		id: "${id}",
		title: "${title}",
		commands: &["${snake}_list"],
		events: &[],
	}
}

#[tauri::command]
#[specta::specta]
pub async fn ${snake}_list() -> Result<Vec<String>, AppError> {
	// TODO: query the store through a service instead of returning a stub.
	Ok(Vec::new())
}
`
	);
	files.set(
		join(root, `crates/weasel-core/src/${snake}.rs`),
		`//! ${title} domain types and service. TODO: implement.
`
	);
}

const markerEdits = [
	{ file: commandsMod, marker: '// @gen:modules', line: `pub mod ${snake};` },
	{
		file: commandsMod,
		marker: '// @gen:descriptors',
		line: `${snake}::descriptor(),`
	},
	{ file: commandsMod, marker: '// @gen:commands', line: `${snake}::${snake}_list,` }
];
if (withCommand) {
	markerEdits.push({ file: coreLib, marker: '// @gen:modules', line: `pub mod ${snake};` });
}

// The browser mock must answer every generated command, and expose the feature in
// `app_features`, or `mock.test.ts` fails on the next `pnpm test`.
const mockFile = join(root, 'desktop/src/lib/ipc/mock.ts');
if (withCommand && existsSync(mockFile)) {
	const mock = readFileSync(mockFile, 'utf8');
	if (mock.includes('// @gen:features') && mock.includes('// @gen:handlers')) {
		markerEdits.push(
			{
				file: mockFile,
				marker: '// @gen:features',
				line: `{ id: "${id}", title: "${title}", commands: ["${snake}_list"], events: [] },`
			},
			{ file: mockFile, marker: '// @gen:handlers', line: `${snake}_list: () => [],` }
		);
	} else {
		console.warn(
			`gen-feature: ${mockFile.slice(root.length + 1)} has no @gen markers; add a mock handler for ${snake}_list by hand`
		);
	}
}

const existing = [...files.keys()].filter((file) => existsSync(file));
if (existing.length > 0) {
	fail(`refusing to overwrite:\n  ${existing.join('\n  ')}`);
}

console.log(`Feature ${id} (${title})${withNav ? ' + nav' : ''}${withCommand ? ' + command' : ''}`);

for (const [file, content] of files) {
	const relative = file.slice(root.length + 1);
	if (dryRun) {
		console.log(`  create ${relative}`);
		continue;
	}
	mkdirSync(dirname(file), { recursive: true });
	writeFileSync(file, content);
	console.log(`  created ${relative}`);
}

const pending = new Map();
for (const edit of markerEdits) {
	const text = pending.get(edit.file) ?? readFileSync(edit.file, 'utf8');
	const lines = text.split('\n');
	const index = lines.findIndex((line) => line.trim() === edit.marker);
	if (index === -1) fail(`${edit.file} lost the marker ${edit.marker}`);
	if (lines.some((line) => line.trim() === edit.line.trim())) continue;
	const indent = lines[index].slice(0, lines[index].indexOf(edit.marker));
	lines.splice(index, 0, `${indent}${edit.line}`);
	pending.set(edit.file, lines.join('\n'));
	if (!dryRun) console.log(`  register in ${edit.file.slice(root.length + 1)}`);
}

if (!dryRun) {
	for (const [file, text] of pending) writeFileSync(file, text);
}

console.log(`
Next steps:
  1. pnpm bindings${withCommand ? '' : ' (no commands to export yet)'}
  2. Add a migration in migrations/ by hand if the feature stores data.
  3. Add a DomainEvent variant and an invalidations entry if it mutates data.
  4. Write tests; then run pnpm verify.`);
