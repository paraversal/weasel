import { describe, expect, it } from 'vitest';

import { holdLabel } from './labels';

describe('holdLabel', () => {
	it('names the rule that matched', () => {
		expect(holdLabel({ kind: 'rule', pattern: '*@temp.uni.edu' })).toBe(
			'Matches the rule *@temp.uni.edu'
		);
	});

	it('explains manual and bounced holds', () => {
		expect(holdLabel({ kind: 'manual' })).toMatch(/by hand/);
		expect(holdLabel({ kind: 'bounced' })).toMatch(/bounced/);
	});
});
