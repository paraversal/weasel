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

	it('answers the event plugin commands used by listen()', () => {
		const handlers = mockHandlers();
		expect(handlers['plugin:event|listen']).toBeTypeOf('function');
		expect(handlers['plugin:event|unlisten']).toBeTypeOf('function');
	});

	it('reports the schema version and domain counts', () => {
		const health = mockHandlers().app_health({}) as Record<string, unknown>;
		expect(health).toMatchObject({
			dbOk: true,
			schemaVersion: 2,
			contactCount: 0,
			pendingImports: 0
		});
	});
});
