import { readFile } from 'node:fs/promises';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  imageCanvas,
  menu,
  menuHint,
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

/** Draws a stroke, so Undo has something to undo. */
async function stroke(page: Page, mode: Mode): Promise<void> {
  await drag(page, canvasOf(page, mode), [0.2, 0.5], [0.8, 0.5]);
  await expect(undoButton(page)).toBeEnabled();
}

// --- Export and import (AC-07 – AC-11, AC-16, AC-17) ---

const U = { key: 'u', mod: true };

function keymapFile(bindings: Record<string, unknown>): string {
  return JSON.stringify({ format: 'pincel-keymap', version: 1, bindings });
}

/** Imports `text` as a JSON file through the dialog's Import button. */
async function importKeymap(
  page: Page,
  dialog: Locator,
  text: string,
  button = 'Import…',
): Promise<void> {
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    dialog.getByRole('button', { name: button, exact: true }).click(),
  ]);
  await chooser.setFiles({
    name: 'keymap.json',
    mimeType: 'application/json',
    buffer: Buffer.from(text),
  });
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
    await page.addInitScript(() => {
      delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
    });
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
    await warning.getByRole('button', { name: 'Abbrechen' }).click();

    await expect(dialog.getByRole('button', { name: 'Exportieren…', exact: true })).toBeVisible();
    await importKeymap(page, dialog, '{nope', 'Importieren…');
    await expect(dialog.getByRole('alert')).toContainText('keine Pincel-Tastenbelegung');
    await importKeymap(page, dialog, keymapFile({ undo: [U], warp: [] }), 'Importieren…');
    await expect(dialog.getByRole('status')).toContainText('1 unbekannter Eintrag ignoriert');
  });
});

test.describe('export and import', () => {
  // Force the download / <input type=file> fallbacks (see palette.spec).
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
      delete w.showSaveFilePicker;
      delete w.showOpenFilePicker;
    });
  });

  test('Export offers the overrides as JSON, with the neutral modifier (AC-07, AC-11)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const dialog = await openOverview(page);
    await change(dialog, 'Undo');
    await page.keyboard.press('Control+u');
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      dialog.getByRole('button', { name: 'Export…', exact: true }).click(),
    ]);
    expect(download.suggestedFilename()).toMatch(/\.json$/);
    const text = await readFile(await download.path(), 'utf8');
    expect(JSON.parse(text)).toEqual({
      format: 'pincel-keymap',
      version: 1,
      bindings: { undo: [U] },
    });
  });

  test('an imported keymap becomes the effective one; defaults give way (AC-08, AC-17)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    const dialog = await openOverview(page);
    // Z is the Zoom tool's default key, a default the file doesn't name.
    await importKeymap(page, dialog, keymapFile({ undo: [U], 'tool:lasso': [{ key: 'z' }] }));
    await expect(dialog.getByRole('status')).toContainText('Keymap imported');
    await expect(dialog.getByRole('status')).toContainText('1 default replaced');
    expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+U']);
    expect(await chords(dialog, 'Lasso')).toEqual(['Z']);
    expect(await chords(dialog, 'Zoom')).toEqual([]);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();

    await page.keyboard.press('z');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Lasso: Lasso');
    await stroke(page, 'image');
    await page.keyboard.press('Control+u');
    await expect(undoButton(page)).toBeDisabled();
    // An exported empty keymap imports as Reset all.
    const again = await openOverview(page);
    await importKeymap(page, again, keymapFile({}));
    expect(await chords(again, 'Undo')).toEqual(['Ctrl+Z']);
    expect(await chords(again, 'Zoom')).toEqual(['Z']);
  });

  test('a broken or foreign file is rejected and changes nothing (AC-09)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const dialog = await openOverview(page);
    await change(dialog, 'Undo');
    await page.keyboard.press('Control+u');
    for (const text of [
      '{nope',
      JSON.stringify({ format: 'something-else', version: 1, bindings: {} }),
      keymapFile({ undo: [{ key: 7 }] }),
      keymapFile({ undo: 'Ctrl+Z' }),
    ]) {
      await importKeymap(page, dialog, text);
      await expect(dialog.getByRole('alert')).toContainText('not a Pincel keymap');
      expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+U']);
    }
  });

  test('unknown entries are ignored with a count (AC-10)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const dialog = await openOverview(page);
    await importKeymap(
      page,
      dialog,
      keymapFile({ undo: [U], frobnicate: [{ key: 'k' }], 'tool:warp': [{ key: 'j' }] }),
    );
    await expect(dialog.getByRole('status')).toContainText('2 unknown entries ignored');
    expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+U']);
  });

  test('a chord bound twice in the file rejects it, naming the chord (AC-16)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const dialog = await openOverview(page);
    await importKeymap(page, dialog, keymapFile({ undo: [U], redo: [{ key: 'U', mod: true }] }));
    await expect(dialog.getByRole('alert')).toContainText('Ctrl+U');
    expect(await chords(dialog, 'Undo')).toEqual(['Ctrl+Z']);
    expect(await chords(dialog, 'Redo')).toEqual(['Ctrl+Shift+Z', 'Ctrl+Y']);
  });
});

test.describe('macOS', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  });

  test('a keymap made with Ctrl+U shows and works as ⌘U (AC-11)', async ({ page }) => {
    await page.addInitScript(() => {
      delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
    });
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    const dialog = await openOverview(page);
    await importKeymap(page, dialog, keymapFile({ undo: [U] }));
    expect(await chords(dialog, 'Undo')).toEqual(['⌘U']);
    await page.keyboard.press('Escape');
    await stroke(page, 'pixel');
    await page.keyboard.press('Meta+u');
    await expect(undoButton(page)).toBeDisabled();
  });
});
