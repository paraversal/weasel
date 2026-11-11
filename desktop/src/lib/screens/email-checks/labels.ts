import type { EmailHoldDto } from '$lib/ipc/bindings';

/** One line saying why an address is on hold. */
export function holdLabel(hold: EmailHoldDto): string {
	switch (hold.kind) {
		case 'manual':
			return 'Marked invalid by hand';
		case 'bounced':
			return 'An email to this address bounced';
		case 'rule':
			return `Matches the rule ${hold.pattern}`;
	}
}
