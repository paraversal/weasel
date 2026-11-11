import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { describe, expect, it } from 'vitest';

/**
 * Architectural guard: features are vertical slices. A feature may use shared modules
 * (`$lib/features/types`, `$lib/features/registry`) but never another feature's internals,
 * and only `$lib/ipc` may touch the Tauri API, so the browser mock stays the single seam.
 */
const ROOT = join(import.meta.dirname, '..');
const FEATURES = join(ROOT, 'src', 'lib', 'features');
const SHELL = join(ROOT, 'src', 'lib', 'shell');

const SOURCE = /\.(ts|svelte)$/;
const SHARED_FEATURE_MODULES = [
	'$lib/features/types',
	'$lib/features/registry',
	'$lib/features/registry.svelte'
];
const IMPORT = /(?:from\s+|import\(\s*)["']([^"']+)["']/g;

function sources(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		if (name === 'node_modules') return [];
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return sources(path);
		return SOURCE.test(name) ? [path] : [];
	});
}

function importsOf(file: string): string[] {
	const text = readFileSync(file, 'utf8');
	return [...text.matchAll(IMPORT)].map((match) => match[1] ?? '');
}

function featureDirectories(): string[] {
	try {
		return readdirSync(FEATURES).filter((name) => statSync(join(FEATURES, name)).isDirectory());
	} catch {
		return [];
	}
}

describe('feature boundaries', () => {
	it("never imports another feature's internals or the Tauri API directly", () => {
		const violations: string[] = [];
		for (const feature of featureDirectories()) {
			for (const file of sources(join(FEATURES, feature))) {
				for (const specifier of importsOf(file)) {
					if (specifier.startsWith('@tauri-apps/')) {
						violations.push(`${relative(ROOT, file)} imports ${specifier}`);
					}
					const match = /^\$lib\/features\/([^/]+)/.exec(specifier);
					if (match && match[1] !== feature && !SHARED_FEATURE_MODULES.includes(specifier)) {
						violations.push(`${relative(ROOT, file)} imports ${specifier}`);
					}
				}
			}
		}
		expect(violations).toEqual([]);
	});

	it('keeps the shell generic: it may only use the registry and feature types', () => {
		const violations: string[] = [];
		for (const file of sources(SHELL)) {
			for (const specifier of importsOf(file)) {
				if (!specifier.startsWith('$lib/features/')) continue;
				if (!SHARED_FEATURE_MODULES.includes(specifier)) {
					violations.push(`${relative(ROOT, file)} imports ${specifier}`);
				}
			}
		}
		expect(violations).toEqual([]);
	});

	it('scans a non-empty source tree (guards against a broken path)', () => {
		expect(sources(SHELL).length).toBeGreaterThan(0);
	});
});
