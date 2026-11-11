import { describe, expect, it } from 'vitest';

import { formatDate, fullName, plural } from './format';

describe('format', () => {
	it('renders missing dates as a dash', () => {
		expect(formatDate(null)).toBe('—');
		expect(formatDate(undefined)).toBe('—');
		expect(formatDate(0)).not.toBe('—');
	});

	it('joins names without stray spaces', () => {
		expect(fullName({ firstName: 'Ada', lastName: 'Lovelace' })).toBe('Ada Lovelace');
		expect(fullName({ firstName: '', lastName: 'Lovelace' })).toBe('Lovelace');
	});

	it('pluralizes with an optional irregular form', () => {
		expect(plural(1, 'freshie')).toBe('1 freshie');
		expect(plural(2, 'freshie')).toBe('2 freshies');
		expect(plural(3, 'address', 'addresses')).toBe('3 addresses');
	});
});
