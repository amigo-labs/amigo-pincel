import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu, storedPref, waitForEditor } from './helpers';

// Edit ▸ Settings… (docs/specs/settings-dialog.md): the app settings that
// used to sit in View — Language, Reduce motion, Touch gestures — in one
// dialog owned by the shell, opened by the menu, Ctrl+, and on the start
// screen by a button.

function settings(page: Page, name = 'Settings'): Locator {
  return page.getByRole('dialog', { name, exact: true });
}

/** The entries of an open top-level menu, as their accessible names. */
async function menuEntries(page: Page, top: string): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  const names = await page
    .getByRole('menu', { name: top, exact: true })
    .locator(':scope > [role^="menuitem"]')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? el.textContent ?? ''));
  await page.keyboard.press('Escape');
  return names.map((n) => n.replace(/\s+/g, ' ').trim());
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: Edit ▸ Settings… opens the dialog with the three settings (AC-01)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await menu(page, 'Edit', 'Settings…');
    const dialog = settings(page);
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAttribute('aria-modal', 'true');
    const language = dialog.getByRole('combobox', { name: 'Language' });
    await expect(language.locator('option')).toHaveText([
      'English',
      'Deutsch',
      'Español',
      '日本語',
      'Français',
      'Português (Brasil)',
    ]);
    await expect(language).toHaveValue('en');
    await expect(dialog.getByRole('checkbox', { name: 'Reduce motion' })).not.toBeChecked();
    await expect(dialog.getByRole('checkbox', { name: 'Touch gestures' })).toBeChecked();
  });

  test(`${mode}: Ctrl+, opens it, Escape closes it and no key reaches the editor (AC-02, AC-06)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const active = page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
    const before = await active.getAttribute('aria-label');
    await page.keyboard.press('Control+,');
    const dialog = settings(page);
    await expect(dialog).toBeVisible();
    // A tool key stays in the dialog: the active tool does not change.
    // (Not 'e': the focused language list would jump to Español.)
    await page.keyboard.press('m');
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    expect(await active.getAttribute('aria-label')).toBe(before);
    await menu(page, 'Edit', 'Settings…');
    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();
  });

  test(`${mode}: View lists no app settings any more (AC-07)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const view = await menuEntries(page, 'View');
    expect(view.filter((n) => /Reduce Motion|Touch Gestures|^Language/.test(n))).toEqual([]);
    expect(view.some((n) => n.startsWith('Zoom In'))).toBe(true);
  });

  test(`${mode}: a changed setting takes effect at once and is kept after a reload (AC-04)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('Control+,');
    const dialog = settings(page);
    await dialog.getByRole('checkbox', { name: 'Touch gestures' }).uncheck();
    await dialog.getByRole('checkbox', { name: 'Reduce motion' }).check();
    await expect(page.locator('html')).toHaveAttribute('data-reduce-motion', '');
    await dialog.getByRole('combobox', { name: 'Language' }).selectOption('de');
    // The dialog follows the language at once (Edge cases).
    const german = settings(page, 'Einstellungen');
    await expect(german).toBeVisible();
    await expect(german.getByRole('checkbox', { name: 'Bewegung reduzieren' })).toBeChecked();
    await expect(page.getByRole('menubar').getByRole('menuitem', { name: 'Ansicht' })).toBeVisible();
    await expect.poll(() => storedPref(page, 'touchGestures')).toBe(false);
    await expect.poll(() => storedPref(page, 'reduceMotion')).toBe(true);

    // Reloaded: the start screen is German now.
    await gotoStart(page);
    const card = mode === 'pixel' ? 'Neues Pixel-Sprite' : 'Neues Bild';
    await page.getByRole('button', { name: card }).click();
    await page.getByRole('form', { name: card }).getByRole('button', { name: 'Erstellen' }).click();
    await waitForEditor(page, mode);
    await page.keyboard.press('Control+,');
    const again = settings(page, 'Einstellungen');
    await expect(again.getByRole('combobox', { name: 'Sprache' })).toHaveValue('de');
    await expect(again.getByRole('checkbox', { name: 'Bewegung reduzieren' })).toBeChecked();
    await expect(again.getByRole('checkbox', { name: 'Touch-Gesten' })).not.toBeChecked();
  });
}

test('the system setting shows as a note on an unchecked Reduce motion (AC-05)', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await createDocument(page, 'image', { width: 16, height: 16 });
  await page.keyboard.press('Control+,');
  const dialog = settings(page);
  const box = dialog.getByRole('checkbox', { name: 'Reduce motion' });
  await expect(box).not.toBeChecked();
  await expect(box).toHaveAccessibleDescription('On in system settings');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await expect(box).not.toHaveAccessibleDescription('On in system settings');
});

test('the Keyboard Shortcuts overview lists Settings with Ctrl+, (AC-02)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press('Control+/');
  const overview = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
  const row = overview
    .getByRole('row')
    .filter({ has: page.getByRole('rowheader', { name: 'Settings', exact: true }) });
  await expect(row.locator('kbd')).toHaveText(['Ctrl+,']);
});

test('the start screen opens Settings by its button and by Ctrl+, (AC-02, AC-03)', async ({
  page,
}) => {
  await gotoStart(page);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const dialog = settings(page);
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await page.getByRole('heading', { name: 'Pincel', exact: true }).click();
  await page.keyboard.press('Control+,');
  await expect(dialog).toBeVisible();
});

test('the German UI names the entry and the dialog in German (AC-09)', async ({ page }) => {
  await gotoStart(page);
  await page.getByLabel('Language').selectOption('de');
  await page.getByRole('button', { name: 'Neues Bild' }).click();
  await page.getByRole('form', { name: 'Neues Bild' }).getByRole('button', { name: 'Erstellen' }).click();
  await waitForEditor(page, 'image');
  await menu(page, 'Bearbeiten', 'Einstellungen…');
  const dialog = settings(page, 'Einstellungen');
  await expect(dialog.getByRole('combobox', { name: 'Sprache' })).toBeVisible();
  await expect(dialog.getByRole('checkbox', { name: 'Bewegung reduzieren' })).toBeVisible();
  await expect(dialog.getByRole('checkbox', { name: 'Touch-Gesten' })).toBeVisible();
});
