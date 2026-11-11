import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { PaletteEntry } from '$lib/features/registry';

import CommandPalette from './CommandPalette.svelte';

// Dialogs portal into `document.body`; unmount between tests so queries stay unambiguous.
afterEach(cleanup);

function entry(overrides: Partial<PaletteEntry>): PaletteEntry {
	return {
		id: 'test.command',
		title: 'Test command',
		group: 'Test',
		featureId: 'test',
		run: () => {},
		...overrides
	};
}

const commands: PaletteEntry[] = [
	entry({ id: 'nav:/', title: 'Go to Dashboard', group: 'Navigation', shortcut: 'g d' }),
	entry({ id: 'nav:/settings', title: 'Go to Settings', group: 'Navigation' }),
	entry({ id: 'campaigns.create', title: 'Create campaign', group: 'Campaigns', keywords: ['new'] })
];

describe('CommandPalette', () => {
	it('filters by title and keeps the shortcut visible', async () => {
		render(CommandPalette, { props: { open: true, commands, platform: 'mac', onRun: () => {} } });

		const input = screen.getByPlaceholderText(/type a command/i);
		await fireEvent.input(input, { target: { value: 'settings' } });

		await waitFor(() => {
			expect(screen.getByText('Go to Settings')).toBeTruthy();
			expect(screen.queryByText('Create campaign')).toBeNull();
		});
	});

	it('runs the first match when Enter is pressed', async () => {
		const onRun = vi.fn();
		render(CommandPalette, { props: { open: true, commands, platform: 'mac', onRun } });

		const input = screen.getByPlaceholderText(/type a command/i);
		await fireEvent.input(input, { target: { value: 'campaign' } });
		await fireEvent.keyDown(input, { key: 'Enter' });

		await waitFor(() => expect(onRun).toHaveBeenCalledTimes(1));
		expect(onRun.mock.calls[0]?.[0]).toMatchObject({ id: 'campaigns.create' });
	});
});
