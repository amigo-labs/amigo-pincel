import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, pixelCanvas } from './helpers';

// Help ▸ Keyboard Shortcuts (docs/specs/shortcut-overview.md): a searchable
// list of every shortcut of the current mode, generated from the one
// keymap (lib/shell/shortcuts.ts).

function overview(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
}

/** The table row whose label cell reads exactly `label`. */
function row(dialog: Locator, label: string): Locator {
  return dialog
    .getByRole('row')
    .filter({ has: dialog.page().getByRole('rowheader', { name: label, exact: true }) });
}

/** The chord texts (`<kbd>`) of the row labelled `label`. */
async function chords(dialog: Locator, label: string): Promise<string[]> {
  return row(dialog, label).locator('kbd').allTextContents();
}

/** Section names in display order. */
async function sections(dialog: Locator): Promise<string[]> {
  return dialog.locator('table caption').allTextContents();
}

/** The active tool-rail button. */
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

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Help ▸ Keyboard Shortcuts opens the overview (AC-01, AC-16)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await menu(page, 'Help', 'Keyboard Shortcuts');
    const dialog = overview(page);
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAttribute('aria-modal', 'true');
  });

  test(`${mode} mode: Ctrl+/ opens the overview, focused on the filter (AC-02, AC-12, AC-15)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('Control+/');
    const dialog = overview(page);
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('searchbox', { name: 'Filter shortcuts' })).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  });

  test(`${mode} mode: rows carry labels and the menu's chords (AC-04, AC-05, AC-06)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const undoHint = await menuHint(page, 'Edit', 'Undo');
    expect(undoHint).toBe('Ctrl+Z');
    const saveAsHint = await menuHint(page, 'File', 'Save As…');

    await page.keyboard.press('Control+/');
    const dialog = overview(page);
    await expect(dialog).toBeVisible();
    expect(await chords(dialog, 'Undo')).toEqual([undoHint]);
    expect((await chords(dialog, 'Save As…'))[0]).toBe(saveAsHint);
    // Alternates follow the primary chord, which is the menu's hint.
    expect(await chords(dialog, 'Redo')).toEqual(['Ctrl+Shift+Z', 'Ctrl+Y']);
    expect(await chords(dialog, 'Lasso')).toEqual(['Q']);
    expect(await chords(dialog, 'Keyboard Shortcuts')).toEqual(['Ctrl+/']);
    // The Pan group (H) exists in Image mode only.
    await expect(row(dialog, 'Pan')).toHaveCount(mode === 'image' ? 1 : 0);

    const labels = await dialog.getByRole('rowheader').allTextContents();
    expect(labels.length).toBeGreaterThan(20);
    for (const label of labels) {
      expect(label.trim()).not.toBe('');
      // An untranslated key reads like `menu.undo`.
      expect(label).not.toMatch(/^[a-z]+(\.[a-zA-Z]+)+$/);
    }
  });

  test(`${mode} mode: sections, tool notes and gestures (AC-07, AC-08, AC-09, AC-10)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('Control+/');
    const dialog = overview(page);
    await expect(dialog).toBeVisible();

    const expected = ['File', 'Edit', 'Select', 'View', 'Tools'];
    if (mode === 'pixel') expected.push('Frames');
    expect(await sections(dialog)).toEqual(expected);

    // Frame keys exist in Pixel mode only.
    const frameRows = ['Previous Frame', 'Next Frame', 'Play / Pause'];
    for (const label of frameRows) {
      await expect(row(dialog, label)).toHaveCount(mode === 'pixel' ? 1 : 0);
    }
    if (mode === 'pixel') {
      const frames = dialog.getByRole('table', { name: 'Frames' });
      expect(await chords(frames, 'Previous Frame')).toEqual([',']);
      expect(await chords(frames, 'Next Frame')).toEqual(['.']);
      expect(await chords(frames, 'Play / Pause')).toEqual(['↵']);
    }

    // A group with variants cycles them; a single tool doesn't.
    await expect(row(dialog, 'Lasso')).toContainText(/press again to cycle/i);
    await expect(row(dialog, 'Shapes')).toContainText(/press again to cycle/i);
    await expect(row(dialog, 'Text')).not.toContainText(/cycle/i);

    // The pointer gestures, each in the section of the related menu.
    const view = dialog.getByRole('table', { name: 'View' });
    expect(await chords(view, 'Pan the view')).toEqual(['Space + drag']);
    expect(await chords(view, 'Pan and zoom (touch)')).toEqual(['Two-finger drag / pinch']);
    const select = dialog.getByRole('table', { name: 'Select' });
    expect(await chords(select, 'Add to selection')).toEqual(['Shift + drag']);
    expect(await chords(select, 'Subtract from selection')).toEqual(['Alt + drag']);
    expect(await chords(select, 'Intersect with selection')).toEqual(['Shift + Alt + drag']);
  });

  test(`${mode} mode: the filter narrows rows and holds the keyboard (AC-13, AC-14)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
    await expect(canvas).toBeVisible();
    await page.keyboard.press('i');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');

    await page.keyboard.press('Control+/');
    const dialog = overview(page);
    const filter = dialog.getByRole('searchbox', { name: 'Filter shortcuts' });
    await expect(filter).toBeFocused();

    await page.keyboard.type('lasso');
    await expect(dialog.getByRole('rowheader')).toHaveCount(1);
    expect(await chords(dialog, 'Lasso')).toEqual(['Q']);
    expect(await sections(dialog)).toEqual(['Tools']);

    await filter.fill('LASSO');
    await expect(dialog.getByRole('rowheader')).toHaveText(['Lasso']);

    await filter.fill('zzz-no-such-shortcut');
    await expect(dialog.getByRole('rowheader')).toHaveCount(0);
    await expect(dialog.locator('table')).toHaveCount(0);
    await expect(filter).toHaveValue('zzz-no-such-shortcut');

    // Typing a tool key into the filter does not switch the tool.
    await filter.fill('');
    await page.keyboard.type('b');
    await expect(filter).toHaveValue('b');
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');
  });
}

test('Ctrl+/ in a text field does not open the overview (AC-02)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'Help', 'Keyboard Shortcuts');
  const dialog = overview(page);
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  // A text field outside the dialog keeps the chord.
  await page.evaluate(() => {
    const input = document.createElement('input');
    input.type = 'text';
    input.setAttribute('aria-label', 'Scratch field');
    document.body.append(input);
    input.focus();
  });
  await page.keyboard.press('Control+/');
  await expect(dialog).toBeHidden();
});

test.describe('macOS', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  });

  test('chords use macOS notation (AC-11)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.keyboard.press('Meta+/');
    const dialog = overview(page);
    await expect(dialog).toBeVisible();
    expect(await chords(dialog, 'Undo')).toEqual(['⌘Z']);
    expect(await chords(dialog, 'Save As…')).toEqual(['⇧⌘S']);
    expect(await chords(dialog, 'Keyboard Shortcuts')).toEqual(['⌘/']);
  });
});

test('printing shows the full list without the editor (AC-17)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.setViewportSize({ width: 1000, height: 400 });
  await page.keyboard.press('Control+/');
  const dialog = overview(page);
  await expect(dialog).toBeVisible();
  const list = dialog.getByTestId('shortcut-list');
  // On screen the list scrolls inside the dialog.
  expect(await list.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);

  await page.emulateMedia({ media: 'print' });
  await expect(pixelCanvas(page)).toBeHidden();
  await expect(page.getByRole('menubar')).toBeHidden();
  expect(await list.evaluate((el) => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
  await expect(row(dialog, 'Intersect with selection')).toBeVisible();
  await expect(dialog.getByRole('searchbox')).toBeHidden();
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('the overview speaks German (AC-18)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Neues Pixel-Sprite' }).click();
    const card = page.getByRole('form', { name: 'Neues Pixel-Sprite' });
    await card.getByRole('button', { name: 'Erstellen' }).click();
    await expect(pixelCanvas(page)).toHaveAttribute('data-ready', 'true', { timeout: 60_000 });

    await page.keyboard.press('Control+/');
    const dialog = page.getByRole('dialog', { name: 'Tastenkürzel' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('searchbox', { name: 'Tastenkürzel filtern' })).toHaveAttribute(
      'placeholder',
      'Tastenkürzel filtern',
    );
    expect(await sections(dialog)).toEqual([
      'Datei',
      'Bearbeiten',
      'Auswahl',
      'Ansicht',
      'Werkzeuge',
      'Frames',
    ]);
    expect(await chords(dialog, 'Ansicht verschieben')).toEqual(['Leertaste + Ziehen']);
    await expect(row(dialog, 'Lasso')).toContainText('Erneut drücken');
  });
});
