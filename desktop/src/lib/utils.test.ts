import { describe, expect, it } from 'vitest';
import { cn } from './utils';

describe('cn', () => {
	it('joins truthy class names and drops falsy ones', () => {
		const disabled = false;
		expect(cn('px-2', disabled && 'opacity-50', undefined, 'text-sm')).toBe('px-2 text-sm');
	});
});
