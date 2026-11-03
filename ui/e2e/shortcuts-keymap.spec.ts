import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  imageCanvas,
  menu,
  pixelCanvas,
  storedPref,
  waitForEditor,
} from './helpers';

// Custom keymap (docs/specs/power-user-shortcuts-keymap.md): rebinding in
// the Keyboard Shortcuts dialog, conflicts, reset and persistence.

type Mode = 'pixel' | 'image';

function overview(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
}

async function openOverview(page: Page): Promise<Locator> {
  await menu(page, 'Help', 'Keyboard Shortcuts');
  const dialog = overview(page);
  await expect(dialog).toBeVisible();
  return dialog;
}

/** The table row whose label cell reads exactly `label`. */
function row(dialog: Locator, label: string): Locator {
  return dialog
    .getByRole('row')
    .filter({ has: dialog.page().getByRole('rowheader', { name: label, exact: true }) });
}

async function chords(dialog: Locator, label: string): Promise<string[]> {
  return row(dialog, label).locator('kbd').allTextContents();
}

/** Clicks Change on the row `label`; the dialog then waits for a chord. */
async function change(dialog: Locator, label: string): Promise<void> {
  await row(dialog, label).getByRole('button', { name: 'Change', exact: true }).click();
  await expect(row(dialog, label)).toContainText('Press a key');
}

function canvasOf(page: Page, mode: Mode): Locator {
  return mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
}

function undoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Undo', exact: true });
}

function activeTool(page: Page): Locator {
  return page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
}

/** The shortcut hint a menu-bar item prints next to its label. */
async function menuHint(page: Page, top: string, label: string): Promise<string> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  const item = page
    .getByRole('menu', { name: top, exact: true })
    .getByRole('menuitem', { name: label, exact: true });
  const text = (await item.locator('span[aria-hidden="true"]').last().textContent()) ?? '';
  await page.keyboard.press('Escape');
  return text.trim();
}

/** Draws a stroke, so Undo has something to undo. */
async function stroke(page: Page, mode: Mode): Promise<void> {
  await drag(page, canvasOf(page, mode), [0.2, 0.5], [0.8, 0.5]);
  await expect(undoButton(page)).toBeEnabled();
}

test('rebinding Undo moves the action, its hints, and survives a reload (AC-01, AC-02, AC-06)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  const dialog = await openOverview(page);
  await change(dialog, 'Undo');
  await page.keyboard.press('Control+u');
  expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+U']);
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();

  // AC-02: menu hint, tooltip and the overview row show the new chord.
  expect(await menuHint(page, 'Edit', 'Undo')).toBe('Ctrl+U');
  await expect(undoButton(page)).toHaveAttribute('title', 'Undo (Ctrl+U)');

  // AC-01: Ctrl+U undoes, Ctrl+Z no longer does.
  await stroke(page, 'pixel');
  await page.keyboard.press('Control+z');
  await expect(undoButton(page)).toBeEnabled();
  await page.keyboard.press('Control+u');
  await expect(undoButton(page)).toBeDisabled();

  // AC-06: kept after a reload, in both modes.
  await expect.poll(() => storedPref(page, 'keymap')).toEqual({ undo: [{ key: 'u', mod: true }] });
  for (const mode of ['pixel', 'image'] as const) {
    await page.reload();
    await createDocument(page, mode, { width: 32, height: 32 });
    expect(await menuHint(page, 'Edit', 'Undo')).toBe('Ctrl+U');
    await stroke(page, mode);
    await page.keyboard.press('Control+z');
    await expect(undoButton(page)).toBeEnabled();
    await page.keyboard.press('Control+u');
    await expect(undoButton(page)).toBeDisabled();
  }
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: a rebound tool key selects and cycles its group (AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = await openOverview(page);
    await change(dialog, 'Lasso');
    await page.keyboard.press('y');
    expect(await chords(dialog, 'Lasso')).toEqual(['Y']);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();

    await page.keyboard.press('i');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');
    await page.keyboard.press('q');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');
    await page.keyboard.press('y');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Lasso: Lasso');
    // The rail's tooltip follows the new key.
    await expect(activeTool(page)).toHaveAttribute('title', 'Lasso (Y)');
    await page.keyboard.press('y');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Lasso: Polygonal Lasso');
  });
}

test('a taken chord warns, naming its owner; Cancel keeps, Replace moves it (AC-04)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const dialog = await openOverview(page);

  await change(dialog, 'Undo');
  await page.keyboard.press('Control+Shift+z');
  const warning = dialog.getByRole('alert');
  await expect(warning).toContainText('Ctrl+Shift+Z');
  await expect(warning).toContainText('Redo');
  await warning.getByRole('button', { name: 'Cancel' }).click();
  await expect(warning).toBeHidden();
  expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+Z']);
  expect(await chords(dialog, 'Redo')).toEqual(['Ctrl+Shift+Z', 'Ctrl+Y']);

  await change(dialog, 'Undo');
  await page.keyboard.press('Control+Shift+z');
  await warning.getByRole('button', { name: 'Replace' }).click();
  await expect(warning).toBeHidden();
  expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+Shift+Z']);
  expect(await chords(dialog, 'Redo')).toEqual(['Ctrl+Y']);

  // A tool key counts too, across both modes' groups (Pan is Image-only).
  await change(dialog, 'Swap Colors');
  await page.keyboard.press('h');
  await expect(warning).toContainText('Pan');
  await warning.getByRole('button', { name: 'Replace' }).click();
  expect(await chords(dialog, 'Swap Colors')).toEqual(['H']);

  // Pressing the current chord again changes nothing and warns about nothing.
  await change(dialog, 'Redo');
  await page.keyboard.press('Control+y');
  await expect(warning).toBeHidden();
  expect(await chords(dialog, 'Redo')).toEqual(['Ctrl+Y']);
});

test('Reset restores a row, Reset all every binding (AC-05)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  const dialog = await openOverview(page);
  await change(dialog, 'Undo');
  await page.keyboard.press('Control+u');
  await change(dialog, 'Lasso');
  await page.keyboard.press('y');
  expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+U']);
  expect(await chords(dialog, 'Lasso')).toEqual(['Y']);

  await row(dialog, 'Undo').getByRole('button', { name: 'Reset', exact: true }).click();
  expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+Z']);
  expect(await chords(dialog, 'Lasso')).toEqual(['Y']);

  await change(dialog, 'Redo');
  await page.keyboard.press('Control+u');
  await dialog.getByRole('button', { name: 'Reset all' }).click();
  expect(await chords(dialog, 'Lasso')).toEqual(['Q']);
  expect(await chords(dialog, 'Redo')).toEqual(['Ctrl+Shift+Z', 'Ctrl+Y']);
  await expect.poll(() => storedPref(page, 'keymap')).toEqual({});
});

test('capture ignores bare modifiers and editor keys; Escape cancels it (AC-12, AC-13)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press('i');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');
  const dialog = await openOverview(page);

  await change(dialog, 'Undo');
  for (const key of ['Shift', 'Control', 'Alt', 'Meta']) {
    await page.keyboard.press(key);
    await expect(row(dialog, 'Undo')).toContainText('Press a key');
  }
  // Escape cancels the capture, not the dialog.
  await page.keyboard.press('Escape');
  await expect(dialog).toBeVisible();
  await expect(row(dialog, 'Undo')).not.toContainText('Press a key');
  expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+Z']);

  // A tool key pressed during capture is the chord, not a tool switch.
  await change(dialog, 'Undo');
  await page.keyboard.press('e');
  await expect(dialog.getByRole('alert')).toContainText('Eraser');
  await dialog.getByRole('alert').getByRole('button', { name: 'Cancel' }).click();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('the keymap editor speaks German (AC-15)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Neues Pixel-Sprite' }).click();
    await page
      .getByRole('form', { name: 'Neues Pixel-Sprite' })
      .getByRole('button', { name: 'Erstellen' })
      .click();
    await waitForEditor(page, 'pixel');
    await page.keyboard.press('Control+/');
    const dialog = page.getByRole('dialog', { name: 'Tastenkürzel' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Alle zurücksetzen', exact: true })).toBeVisible();
    const undo = row(dialog, 'Rückgängig');
    await expect(undo.getByRole('button', { name: 'Zurücksetzen', exact: true })).toBeVisible();
    await undo.getByRole('button', { name: 'Ändern', exact: true }).click();
    await expect(undo).toContainText('Taste drücken');
    await page.keyboard.press('Control+Shift+z');
    const warning = dialog.getByRole('alert');
    await expect(warning).toContainText('bereits');
    await expect(warning).toContainText('Wiederholen');
    await expect(warning.getByRole('button', { name: 'Ersetzen' })).toBeVisible();
    await expect(warning.getByRole('button', { name: 'Abbrechen' })).toBeVisible();
  });
});
