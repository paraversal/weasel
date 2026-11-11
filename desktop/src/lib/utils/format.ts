const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
	dateStyle: 'medium',
	timeStyle: 'short'
});
const numberFormat = new Intl.NumberFormat();

/** Unix milliseconds → local short date. `null`/`undefined` → an em dash. */
export function formatDate(ms: number | null | undefined): string {
	return ms == null ? '—' : dateFormat.format(ms);
}

export function formatDateTime(ms: number | null | undefined): string {
	return ms == null ? '—' : dateTimeFormat.format(ms);
}

export function formatNumber(n: number): string {
	return numberFormat.format(n);
}

export function fullName(person: { firstName: string; lastName: string }): string {
	return `${person.firstName} ${person.lastName}`.trim();
}

/** `plural(1, 'freshie')` → "1 freshie"; `plural(3, 'address', 'addresses')` → "3 addresses". */
export function plural(n: number, one: string, many = `${one}s`): string {
	return `${formatNumber(n)} ${n === 1 ? one : many}`;
}
