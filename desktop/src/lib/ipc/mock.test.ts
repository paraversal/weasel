import { describe, expect, it } from 'vitest';

import { commands } from './bindings';
import { missingHandlers, mockHandlers, toCommandName } from './mock';

describe('mock ipc', () => {
	it('implements every generated command', () => {
		expect(missingHandlers(mockHandlers())).toEqual([]);
	});

	it('reports commands without a handler', () => {
		const missing = missingHandlers({ app_info: () => null });
		expect(missing).not.toContain('app_info');
		expect(missing).toContain('settings_update');
	});

	it('derives snake_case names that match the Rust command names', () => {
		const names = Object.keys(commands).map(toCommandName);
		expect(names).toContain('settings_get_all');
		expect(names).toContain('app_open_data_dir');
		for (const name of names) expect(name).toMatch(/^[a-z][a-z0-9_]*$/);
	});

	it('reports the schema version and domain counts', () => {
		const handler = mockHandlers().app_health;
		if (!handler) throw new Error('app_health handler missing');
		const health = handler({}) as {
			dbOk: boolean;
			schemaVersion: number;
			contactCount: number;
			pendingImports: number;
		};
		expect(health.dbOk).toBe(true);
		expect(health.schemaVersion).toBe(2);
		expect(health.contactCount).toBeGreaterThan(0);
		expect(health.pendingImports).toBe(0);
	});
});
