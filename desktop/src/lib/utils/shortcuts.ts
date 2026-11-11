/**
 * Shortcut parsing and matching, independent of Svelte so it can be unit tested.
 *
 * Specs: `"mod+k"` (⌘ on macOS, Ctrl elsewhere), `"mod+shift+c"`, `"?"`, `"g d"` (a
 * two-key sequence). `mod` never means both meta and ctrl.
 */
export type Platform = 'mac' | 'other';

export interface Chord {
	kind: 'chord';
	mod: boolean;
	ctrl: boolean;
	alt: boolean;
	shift: boolean;
	key: string;
}

export interface Sequence {
	kind: 'sequence';
	first: string;
	second: string;
}

export type ParsedShortcut = Chord | Sequence;

export interface KeyEventLike {
	key: string;
	metaKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
	shiftKey: boolean;
	target?: EventTarget | null;
}

export const SEQUENCE_TIMEOUT_MS = 1000;

const MODIFIER_TOKENS: Record<string, true> = {
	mod: true,
	meta: true,
	cmd: true,
	ctrl: true,
	control: true,
	alt: true,
	option: true,
	shift: true
};

/** Keys whose `event.key` is longer than one character. */
const NAMED_KEYS: Record<string, true> = {
	escape: true,
	enter: true,
	tab: true,
	backspace: true,
	delete: true,
	arrowup: true,
	arrowdown: true,
	arrowleft: true,
	arrowright: true,
	space: true
};

const EDITABLE_TAGS: Record<string, true> = { input: true, textarea: true, select: true };

/** Shortcuts that still work while a form control has focus. */
const EDITABLE_ALLOWED: Record<string, true> = { 'mod+k': true, 'mod+enter': true };

function parseChord(spec: string): Chord | undefined {
	const parts = spec.toLowerCase().split('+');
	const key = parts.pop();
	if (!key || (key.length !== 1 && NAMED_KEYS[key] !== true)) return undefined;
	const chord: Chord = {
		kind: 'chord',
		mod: false,
		ctrl: false,
		alt: false,
		shift: false,
		key
	};
	for (const part of parts) {
		if (MODIFIER_TOKENS[part] !== true) return undefined;
		switch (part) {
			case 'mod':
			case 'meta':
			case 'cmd':
				chord.mod = true;
				break;
			case 'ctrl':
			case 'control':
				chord.ctrl = true;
				break;
			case 'alt':
			case 'option':
				chord.alt = true;
				break;
			case 'shift':
				chord.shift = true;
				break;
		}
	}
	return chord;
}

export function parseShortcut(spec: string): ParsedShortcut | undefined {
	const tokens = spec.trim().split(/\s+/).filter(Boolean);
	if (tokens.length === 1) return parseChord(tokens[0] ?? '');
	if (tokens.length !== 2) return undefined;
	const first = tokens[0]?.toLowerCase();
	const second = tokens[1]?.toLowerCase();
	if (!first || !second) return undefined;
	// Sequence keys are bare: no modifiers, single characters.
	if (first.length !== 1 || second.length !== 1) return undefined;
	return { kind: 'sequence', first, second };
}

/** Stable identity of a shortcut, used to detect two commands claiming one key. */
export function canonicalShortcut(spec: string): string {
	const parsed = parseShortcut(spec);
	if (!parsed) return spec.trim().toLowerCase();
	if (parsed.kind === 'sequence') return `${parsed.first} ${parsed.second}`;
	const modifiers = [
		parsed.mod ? 'mod' : '',
		parsed.ctrl ? 'ctrl' : '',
		parsed.alt ? 'alt' : '',
		parsed.shift ? 'shift' : ''
	].filter(Boolean);
	return [...modifiers, parsed.key].join('+');
}

export function detectPlatform(): Platform {
	if (typeof navigator === 'undefined') return 'other';
	const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
	const platform = nav.userAgentData?.platform ?? nav.platform ?? nav.userAgent;
	return /mac|iphone|ipad|ipod/i.test(platform) ? 'mac' : 'other';
}

export function isEditableTarget(target: EventTarget | null | undefined): boolean {
	if (!target || typeof target !== 'object') return false;
	const element = target as {
		tagName?: string;
		isContentEditable?: boolean;
		getAttribute?: (name: string) => string | null;
	};
	if (EDITABLE_TAGS[element.tagName?.toLowerCase() ?? ''] === true) return true;
	if (element.isContentEditable === true) return true;
	return element.getAttribute?.('contenteditable') === 'true';
}

function matchesChord(chord: Chord, event: KeyEventLike, platform: Platform): boolean {
	if (event.key.toLowerCase() !== chord.key) return false;
	const modPressed = platform === 'mac' ? event.metaKey : event.ctrlKey;
	if (chord.mod !== modPressed) return false;
	// `ctrl` on macOS is a distinct modifier; elsewhere it is already `mod`.
	if (platform === 'mac' && chord.ctrl !== event.ctrlKey) return false;
	if (platform !== 'mac' && chord.ctrl && !event.ctrlKey) return false;
	if (platform === 'mac' && chord.ctrl && event.metaKey) return false;
	if (chord.alt !== event.altKey) return false;
	if (chord.shift && !event.shiftKey) return false;
	return true;
}

export function formatShortcut(spec: string, platform: Platform): string {
	const parsed = parseShortcut(spec);
	if (!parsed) return spec;
	if (parsed.kind === 'sequence')
		return `${parsed.first.toUpperCase()} then ${parsed.second.toUpperCase()}`;
	const isMac = platform === 'mac';
	const parts: string[] = [];
	if (parsed.mod) parts.push(isMac ? '⌘' : 'Ctrl');
	if (parsed.ctrl && (!parsed.mod || !isMac)) parts.push(isMac ? '⌃' : 'Ctrl');
	if (parsed.alt) parts.push(isMac ? '⌥' : 'Alt');
	if (parsed.shift) parts.push(isMac ? '⇧' : 'Shift');
	const key = parsed.key === '?' ? '?' : parsed.key.toUpperCase();
	parts.push(key);
	return isMac ? parts.join('') : parts.join('+');
}

export interface ShortcutBinding {
	/** The spec, kept for diagnostics. */
	spec: string;
	run: () => void;
}

interface MatcherOptions {
	platform?: Platform;
	now?: () => number;
	timeoutMs?: number;
	isEditable?: (target: EventTarget | null | undefined) => boolean;
}

/**
 * One global `keydown` handler for every registered shortcut. Reports whether the event
 * was consumed so callers know to `preventDefault`.
 */
export class ShortcutMatcher {
	private readonly platform: Platform;
	private readonly now: () => number;
	private readonly timeoutMs: number;
	private readonly editable: (target: EventTarget | null | undefined) => boolean;
	private readonly chords: Array<{ chord: Chord; spec: string; run: () => void }> = [];
	private readonly sequences: Array<{
		first: string;
		second: string;
		spec: string;
		run: () => void;
	}> = [];
	private pending: { key: string; at: number } | null = null;

	constructor(bindings: ShortcutBinding[], options: MatcherOptions = {}) {
		this.platform = options.platform ?? detectPlatform();
		this.now = options.now ?? (() => Date.now());
		this.timeoutMs = options.timeoutMs ?? SEQUENCE_TIMEOUT_MS;
		this.editable = options.isEditable ?? isEditableTarget;
		for (const binding of bindings) {
			const parsed = parseShortcut(binding.spec);
			if (!parsed) continue;
			if (parsed.kind === 'chord') {
				this.chords.push({ chord: parsed, spec: binding.spec, run: binding.run });
			} else {
				this.sequences.push({
					first: parsed.first,
					second: parsed.second,
					spec: binding.spec,
					run: binding.run
				});
			}
		}
	}

	handleKeydown(event: KeyEventLike): boolean {
		const editable = this.editable(event.target);
		const key = event.key.toLowerCase();
		const now = this.now();

		if (this.pending) {
			const pending = this.pending;
			this.pending = null;
			if (now - pending.at <= this.timeoutMs && !editable) {
				const match = this.sequences.find(
					(sequence) => sequence.first === pending.key && sequence.second === key
				);
				if (match) {
					match.run();
					return true;
				}
			}
		}

		for (const { chord, spec, run } of this.chords) {
			if (editable && EDITABLE_ALLOWED[canonicalShortcut(spec)] !== true) continue;
			if (matchesChord(chord, event, this.platform)) {
				run();
				return true;
			}
		}

		if (!editable && !event.metaKey && !event.ctrlKey && !event.altKey) {
			if (this.sequences.some((sequence) => sequence.first === key)) {
				this.pending = { key, at: now };
				return true;
			}
		}

		return false;
	}

	/** Forgets a half-typed sequence, e.g. when an overlay opens. */
	reset(): void {
		this.pending = null;
	}
}
