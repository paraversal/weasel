import { describe, expect, it, vi } from 'vitest';

import {
	ShortcutMatcher,
	canonicalShortcut,
	formatShortcut,
	isEditableTarget,
	parseShortcut,
	type KeyEventLike
} from './shortcuts';

function event(key: string, overrides: Partial<KeyEventLike> = {}): KeyEventLike {
	return {
		key,
		metaKey: false,
		ctrlKey: false,
		altKey: false,
		shiftKey: false,
		target: null,
		...overrides
	};
}

function inputTarget(): EventTarget {
	return { tagName: 'input' } as unknown as EventTarget;
}

function editableDiv(): EventTarget {
	return {
		tagName: 'div',
		getAttribute: (name: string) => (name === 'contenteditable' ? 'true' : null)
	} as unknown as EventTarget;
}

describe('parseShortcut', () => {
	it('parses modifiers and a key', () => {
		expect(parseShortcut('mod+shift+c')).toEqual({
			kind: 'chord',
			mod: true,
			ctrl: false,
			alt: false,
			shift: true,
			key: 'c'
		});
	});

	it('parses sequences and punctuation keys', () => {
		expect(parseShortcut('g d')).toEqual({ kind: 'sequence', first: 'g', second: 'd' });
		expect(parseShortcut('?')).toMatchObject({ kind: 'chord', key: '?' });
	});

	it('rejects malformed specs', () => {
		expect(parseShortcut('hyper+k')).toBeUndefined();
		expect(parseShortcut('g d f')).toBeUndefined();
		expect(parseShortcut('mod+')).toBeUndefined();
	});

	it('canonicalises aliases to one identity', () => {
		expect(canonicalShortcut('cmd+shift+K')).toBe('mod+shift+k');
		expect(canonicalShortcut('mod+shift+k')).toBe('mod+shift+k');
	});
});

describe('formatShortcut', () => {
	it("uses the platform's modifier glyphs", () => {
		expect(formatShortcut('mod+k', 'mac')).toBe('⌘K');
		expect(formatShortcut('mod+k', 'other')).toBe('Ctrl+K');
		expect(formatShortcut('mod+shift+c', 'mac')).toBe('⌘⇧C');
		expect(formatShortcut('g d', 'other')).toBe('G then D');
	});
});

describe('isEditableTarget', () => {
	it('detects form controls and contenteditable', () => {
		expect(isEditableTarget(inputTarget())).toBe(true);
		expect(isEditableTarget(editableDiv())).toBe(true);
		expect(isEditableTarget({ tagName: 'div' } as unknown as EventTarget)).toBe(false);
		expect(isEditableTarget(null)).toBe(false);
	});
});

describe('ShortcutMatcher', () => {
	it('maps mod to meta on macOS and ctrl elsewhere', () => {
		const macRun = vi.fn();
		const mac = new ShortcutMatcher([{ spec: 'mod+k', run: macRun }], { platform: 'mac' });
		expect(mac.handleKeydown(event('k', { ctrlKey: true }))).toBe(false);
		expect(mac.handleKeydown(event('k', { metaKey: true }))).toBe(true);
		expect(macRun).toHaveBeenCalledOnce();

		const otherRun = vi.fn();
		const other = new ShortcutMatcher([{ spec: 'mod+k', run: otherRun }], { platform: 'other' });
		expect(other.handleKeydown(event('k', { metaKey: true }))).toBe(false);
		expect(other.handleKeydown(event('k', { ctrlKey: true }))).toBe(true);
		expect(otherRun).toHaveBeenCalledOnce();
	});

	it('ignores bare-letter shortcuts while an input has focus, but not mod+k', () => {
		const run = vi.fn();
		const matcher = new ShortcutMatcher([{ spec: '?', run }], { platform: 'mac' });
		expect(matcher.handleKeydown(event('?', { target: inputTarget(), shiftKey: true }))).toBe(
			false
		);
		expect(run).not.toHaveBeenCalled();

		const palette = vi.fn();
		const withPalette = new ShortcutMatcher([{ spec: 'mod+k', run: palette }], { platform: 'mac' });
		expect(withPalette.handleKeydown(event('k', { metaKey: true, target: inputTarget() }))).toBe(
			true
		);
		expect(palette).toHaveBeenCalledOnce();
	});

	it('fires two-key sequences inside the timeout and forgets them after', () => {
		let now = 0;
		const run = vi.fn();
		const matcher = new ShortcutMatcher([{ spec: 'g d', run }], {
			platform: 'mac',
			now: () => now
		});

		expect(matcher.handleKeydown(event('g'))).toBe(true);
		expect(run).not.toHaveBeenCalled();
		now = 500;
		expect(matcher.handleKeydown(event('d'))).toBe(true);
		expect(run).toHaveBeenCalledOnce();

		matcher.handleKeydown(event('g'));
		now = 2500;
		expect(matcher.handleKeydown(event('d'))).toBe(false);
		expect(run).toHaveBeenCalledOnce();
	});

	it('does not start a sequence from inside a text field', () => {
		const run = vi.fn();
		const matcher = new ShortcutMatcher([{ spec: 'g d', run }], { platform: 'mac' });
		matcher.handleKeydown(event('g', { target: inputTarget() }));
		expect(matcher.handleKeydown(event('d', { target: inputTarget() }))).toBe(false);
		expect(run).not.toHaveBeenCalled();
	});

	it('drops invalid bindings instead of throwing', () => {
		const run = vi.fn();
		const matcher = new ShortcutMatcher([{ spec: 'hyper+x', run }], { platform: 'mac' });
		expect(matcher.handleKeydown(event('x', { metaKey: true }))).toBe(false);
		expect(run).not.toHaveBeenCalled();
	});
});
