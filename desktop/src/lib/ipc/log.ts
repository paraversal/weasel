import { error as pluginError, info as pluginInfo } from '@tauri-apps/plugin-log';

/**
 * The only place the app talks to the log plugin, so the rest of the frontend keeps the
 * "no direct @tauri-apps imports" rule. Logging must never throw into a render path.
 */
function describe(value: unknown): string {
	if (value instanceof Error) return `${value.name}: ${value.message}\n${value.stack ?? ''}`;
	if (typeof value === 'string') return value;
	try {
		return JSON.stringify(value);
	} catch {
		return String(value);
	}
}

function write(kind: 'error' | 'info', message: string, details: unknown[]) {
	const suffix = details.length ? ` ${details.map(describe).join(' ')}` : '';
	const sink = kind === 'error' ? pluginError : pluginInfo;
	void sink(`${message}${suffix}`).catch((cause: unknown) => {
		console.error(message, ...details, cause);
	});
}

export function logError(message: string, ...details: unknown[]): void {
	write('error', message, details);
}

export function logInfo(message: string, ...details: unknown[]): void {
	write('info', message, details);
}
