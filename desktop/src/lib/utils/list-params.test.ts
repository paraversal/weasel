import { describe, expect, it } from 'vitest';

import { intParam, listHref, offsetFor, pageParam, param } from './list-params';

const url = (path: string) => new URL(path, 'http://app');

describe('list params', () => {
	it('reads trimmed params and positive integers only', () => {
		const u = url('/freshies?search=%20ada%20&page=3&id=0&bad=x1');
		expect(param(u, 'search')).toBe('ada');
		expect(param(u, 'missing')).toBeUndefined();
		expect(pageParam(u)).toBe(3);
		expect(intParam(u, 'id')).toBeUndefined();
		expect(intParam(u, 'bad')).toBeUndefined();
		expect(pageParam(url('/freshies'))).toBe(1);
		expect(offsetFor(3)).toBe(100);
	});

	it('resets the page when a filter changes', () => {
		const u = url('/freshies?search=ada&page=4');
		expect(listHref(u, { status: 'removed' })).toBe('/freshies?search=ada&status=removed');
	});

	it('keeps other params when paging and drops page 1', () => {
		const u = url('/freshies?search=ada&page=4');
		expect(listHref(u, { page: 5 })).toBe('/freshies?search=ada&page=5');
		expect(listHref(u, { page: 1 })).toBe('/freshies?search=ada');
	});

	it('removes blank values', () => {
		expect(listHref(url('/freshies?search=ada'), { search: '' })).toBe('/freshies');
	});
});
