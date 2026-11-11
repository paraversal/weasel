import { createQuery } from '@tanstack/svelte-query';

import { settingsQuery } from '$lib/ipc/queries';

/** What the UI calls the people in the address book (Settings → Wording). */
export interface Term {
	/** "freshie" */
	one: string;
	/** "freshies" */
	many: string;
	/** "Freshie" */
	One: string;
	/** "Freshies" */
	Many: string;
}

export const DEFAULT_TERM = { singular: 'freshie', plural: 'freshies' };

export function capitalize(word: string): string {
	return word.charAt(0).toLocaleUpperCase() + word.slice(1);
}

export function termFrom(term: { singular: string; plural: string }): Term {
	return {
		one: term.singular,
		many: term.plural,
		One: capitalize(term.singular),
		Many: capitalize(term.plural)
	};
}

/**
 * The configured term, live: it follows `settingsChanged`. Call during component
 * setup; every caller shares one cached settings query.
 */
export function useTerm(): Term {
	const settings = createQuery(() => settingsQuery());
	const current = $derived(termFrom(settings.data?.audienceTerm ?? DEFAULT_TERM));
	return {
		get one() {
			return current.one;
		},
		get many() {
			return current.many;
		},
		get One() {
			return current.One;
		},
		get Many() {
			return current.Many;
		}
	};
}
