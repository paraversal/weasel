import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const generator = join(dirname(fileURLToPath(import.meta.url)), 'gen-feature.mjs');

/** Minimal tree containing only the files the generator reads and edits. */
function makeRoot() {
	const root = mkdtempSync(join(tmpdir(), 'gen-feature-'));
	mkdirSync(join(root, 'desktop/src-tauri/src/commands'), { recursive: true });
	mkdirSync(join(root, 'crates/weasel-core/src'), { recursive: true });
	writeFileSync(
		join(root, 'desktop/src-tauri/src/commands/mod.rs'),
		`use weasel_core::FeatureDescriptor;

pub mod app;
// @gen:modules

pub fn feature_descriptors() -> Vec<FeatureDescriptor> {
    vec![
        app::descriptor(),
        // @gen:descriptors
    ]
}

pub fn builder() -> () {
    let _ = [
        app::app_info,
        // @gen:commands
    ];
}
`
	);
	writeFileSync(join(root, 'crates/weasel-core/src/lib.rs'), `pub mod error;
// @gen:modules
`);
	mkdirSync(join(root, 'desktop/src/lib/ipc'), { recursive: true });
	writeFileSync(
		join(root, 'desktop/src/lib/ipc/mock.ts'),
		`const features = [
	// @gen:features
];

export function mockHandlers() {
	return {
		// @gen:handlers
		'plugin:event|listen': () => 1
	};
}
`
	);
	return root;
}

function run(root, args) {
	return spawnSync(process.execPath, [generator, ...args, '--root', root], {
		encoding: 'utf8'
	});
}

test('generates a nav + command slice and registers it through the markers', () => {
	const root = makeRoot();
	const result = run(root, ['feature', 'demo', '--nav', '--command']);

	assert.equal(result.status, 0, result.stderr);
	for (const file of [
		'desktop/src/lib/features/demo/manifest.ts',
		'desktop/src/lib/features/demo/README.md',
		'desktop/src/lib/features/demo/components/DemoPanel.svelte',
		'desktop/src/routes/demo/+page.svelte',
		'desktop/src-tauri/src/commands/demo.rs',
		'crates/weasel-core/src/demo.rs'
	]) {
		assert.ok(existsSync(join(root, file)), `missing ${file}`);
	}

	const manifest = readFileSync(join(root, 'desktop/src/lib/features/demo/manifest.ts'), 'utf8');
	assert.match(manifest, /icon: 'box'/);
	assert.match(manifest, /Go to Demo/);
	const panel = readFileSync(
		join(root, 'desktop/src/lib/features/demo/components/DemoPanel.svelte'),
		'utf8'
	);
	assert.match(panel, /commands\.demoList\(\)/);
	const command = readFileSync(join(root, 'desktop/src-tauri/src/commands/demo.rs'), 'utf8');
	assert.match(command, /pub fn descriptor\(\)/);
	assert.match(command, /pub async fn demo_list\(\)/);

	const mod = readFileSync(join(root, 'desktop/src-tauri/src/commands/mod.rs'), 'utf8');
	assert.match(mod, /^pub mod demo;$/m);
	assert.match(mod, /^        demo::descriptor\(\),$/m);
	assert.match(mod, /^        demo::demo_list,$/m);

	const core = readFileSync(join(root, 'crates/weasel-core/src/lib.rs'), 'utf8');
	assert.match(core, /^pub mod demo;$/m);

	const mock = readFileSync(join(root, 'desktop/src/lib/ipc/mock.ts'), 'utf8');
	assert.match(mock, /^\t\{ id: "demo", title: "Demo", commands: \["demo_list"\], events: \[\] \},$/m);
	assert.match(mock, /^\t\tdemo_list: \(\) => \[\],$/m);
});

test('refuses to run twice', () => {
	const root = makeRoot();
	assert.equal(run(root, ['feature', 'demo', '--nav', '--command']).status, 0);

	const again = run(root, ['feature', 'demo', '--nav', '--command']);
	assert.notEqual(again.status, 0);
	assert.match(again.stderr, /refusing to overwrite/);
});

test('--dry-run writes nothing', () => {
	const root = makeRoot();
	const before = readFileSync(join(root, 'desktop/src-tauri/src/commands/mod.rs'), 'utf8');

	const result = run(root, ['feature', 'demo', '--nav', '--dry-run']);

	assert.equal(result.status, 0, result.stderr);
	assert.match(result.stdout, /create desktop\/src\/lib\/features\/demo\/manifest\.ts/);
	assert.ok(!existsSync(join(root, 'desktop/src/lib/features/demo')));
	assert.equal(readFileSync(join(root, 'desktop/src-tauri/src/commands/mod.rs'), 'utf8'), before);
});

test('rejects invalid names and missing markers', () => {
	const root = makeRoot();
	assert.notEqual(run(root, ['feature', 'NotValid']).status, 0);

	writeFileSync(join(root, 'desktop/src-tauri/src/commands/mod.rs'), 'pub mod app;\n');
	const result = run(root, ['feature', 'demo']);
	assert.notEqual(result.status, 0);
	assert.match(result.stderr, /missing the marker/);
});
