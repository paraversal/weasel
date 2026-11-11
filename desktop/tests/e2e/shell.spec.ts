import { expect, test } from '@playwright/test';

/**
 * Runs against `VITE_IPC_MOCK=1` builds (see playwright.config.ts). The mock persists
 * settings in localStorage, standing in for SQLite so the reload assertion is meaningful.
 */
const mod = process.platform === 'darwin' ? 'Meta' : 'Control';

test('the sidebar lists the registered features', async ({ page }) => {
	await page.goto('/');

	for (const label of ['Dashboard', 'Settings', 'Help']) {
		await expect(page.getByRole('link', { name: label })).toBeVisible();
	}
});

test('the palette opens with the platform modifier and navigates', async ({ page }) => {
	await page.goto('/');
	// Shortcut listeners live in AppShell, which mounts only after the boot gate resolves.
	await page.getByRole('link', { name: 'Dashboard' }).waitFor();
	await page.keyboard.press(`${mod}+k`);

	const palette = page.getByRole('dialog', { name: 'Command palette' });
	await expect(palette).toBeVisible();

	await page.getByRole('option', { name: /Go to Settings/ }).click();
	await expect(page).toHaveURL(/\/settings$/);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
});

test('choosing Dark applies the class and survives reload', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Dashboard' })).toBeVisible();

	await page.getByRole('button', { name: 'Theme' }).click();
	await page.getByRole('menuitem', { name: 'Dark' }).click();
	await expect(page.locator('html')).toHaveClass(/dark/);

	await page.reload();
	await expect(page.getByRole('link', { name: 'Dashboard' })).toBeVisible();
	await expect(page.locator('html')).toHaveClass(/dark/);
});

test('g h navigates to Help but typing inside an input does not', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Dashboard' })).toBeVisible();

	// Bare sequences are ignored while a text field owns the keyboard.
	await page.keyboard.press(`${mod}+k`);
	await page.getByRole('dialog', { name: 'Command palette' }).waitFor();
	await page.keyboard.type('gd');
	await expect(page).toHaveURL(/\/$/);
	await page.keyboard.press('Escape');
	// Focus must leave the command input before bare sequences are handled again.
	await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeHidden();

	await page.keyboard.press('g');
	await page.keyboard.press('h');
	await expect(page).toHaveURL(/\/help$/);
	await expect(page.getByRole('heading', { name: 'Help' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Shortcuts' })).toBeVisible();
});
