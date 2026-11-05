import { expect, test, type Page } from '@playwright/test';
import { createDocument, gotoStart, menuHint, waitForEditor } from './helpers';
import { installTauriMock } from './tauri-mock';

// Ctrl+N opens the New dialog only where the page gets the chord: the
// desktop app on Windows / Linux and an installed app window; a browser tab
// leaves it to the browser (docs/specs/power-user-shortcuts-coverage.md
// AC-14, AC-20). Playwright's synthetic keys reach the page even for chords
// a real browser keeps, so the tab case checks that nothing is bound.

const WINDOWS_UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

/** Chords a browser tab cannot capture (Technical notes). */
const RESERVED = [
  'Ctrl+N',
  'Ctrl+Shift+N',
  'Ctrl+T',
  'Ctrl+Shift+T',
  'Ctrl+W',
  'Ctrl+Shift+W',
  'Ctrl+Tab',
  'Ctrl+Shift+Tab',
  'Ctrl+R',
  'Ctrl+Shift+R',
  'F5',
];

const newDialog = (page: Page) => page.getByRole('dialog', { name: 'New document' });

/** Pretends to run as an installed app in its own window. */
async function installedWindow(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const original = window.matchMedia.bind(window);
    window.matchMedia = (query: string) =>
      query.replace(/\s/g, '') === '(display-mode:standalone)'
        ? ({ ...original(query), matches: true, media: query } as MediaQueryList)
        : original(query);
  });
}

/** Display chords of Help ▸ Keyboard Shortcuts, and the row labels. */
async function overview(page: Page): Promise<{ chords: string[]; labels: string[] }> {
  await page.keyboard.press('Control+/');
  const dialog = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
  await expect(dialog).toBeVisible();
  const chords = (await dialog.locator('kbd').allTextContents()).map((c) => c.trim());
  const labels = (await dialog.getByRole('row').allTextContents()).map((c) => c.trim());
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  return { chords, labels };
}

/**
 * Ctrl+N on the start screen and in both editors opens the New dialog; each
 * document is created through it (a reload would bring back the desktop
 * app's first-launch note).
 */
async function expectCtrlNOpensNew(page: Page): Promise<void> {
  for (const mode of ['pixel', 'image'] as const) {
    await page.keyboard.press('Control+n');
    await expect(newDialog(page)).toBeVisible();
    await newDialog(page)
      .getByRole('radio', { name: mode === 'pixel' ? /^Pixel/ : /^Image/ })
      .click();
    await newDialog(page).getByRole('button', { name: 'Create' }).click();
    await expect(newDialog(page)).toBeHidden();
    await waitForEditor(page, mode);

    // In the editor: the dialog opens preset to this mode; Cancel keeps it.
    await page.keyboard.press('Control+n');
    await expect(newDialog(page)).toBeVisible();
    await expect(
      newDialog(page).getByRole('radio', { name: mode === 'pixel' ? /^Pixel/ : /^Image/ }),
    ).toHaveAttribute('aria-checked', 'true');
    await page.keyboard.press('Escape');
    await expect(newDialog(page)).toBeHidden();
    expect(await menuHint(page, 'File', 'New…')).toBe('Ctrl+N');
    const { chords, labels } = await overview(page);
    expect(labels.some((l) => l.startsWith('New…') && l.includes('Ctrl+N'))).toBe(true);
    expect(chords.filter((c, i) => chords.indexOf(c) !== i)).toEqual([]);
  }
}

test.describe('in a browser tab', () => {
  test('Ctrl+N has no binding, no menu hint and no overview row; no default chord is browser-reserved (AC-14, AC-20)', async ({
    page,
  }) => {
    await gotoStart(page);
    await page.keyboard.press('Control+n');
    await page.waitForTimeout(300);
    await expect(newDialog(page)).toHaveCount(0);
    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode, { width: 16, height: 16 });
      await page.keyboard.press('Control+n');
      await page.waitForTimeout(300);
      await expect(newDialog(page)).toHaveCount(0);
      expect(await menuHint(page, 'File', 'New…')).toBe('');
      const { chords, labels } = await overview(page);
      expect(labels.some((l) => l.startsWith('New…'))).toBe(false);
      expect(chords.filter((c) => RESERVED.includes(c))).toEqual([]);
    }
  });
});

test.describe('in an installed app window', () => {
  test('Ctrl+N opens the New dialog on the start screen and in both editors, with hint and overview row; it is the only reserved chord (AC-14, AC-20)', async ({
    page,
  }) => {
    await installedWindow(page);
    await gotoStart(page);
    await expectCtrlNOpensNew(page);
    const { chords } = await overview(page);
    expect(chords.filter((c) => RESERVED.includes(c))).toEqual(['Ctrl+N']);
  });

  test('a focused text field keeps Ctrl+N (AC-20, edge case)', async ({ page }) => {
    await installedWindow(page);
    await createDocument(page, 'image', { width: 16, height: 16 });
    const list = page.getByRole('list', { name: 'Layer list' });
    const grip = list.getByRole('button', { name: /^Drag to reorder / }).first();
    const name = ((await grip.getAttribute('aria-label')) ?? '').replace('Drag to reorder ', '');
    await list.locator(`button[title="${name}"]`).dblclick();
    await expect(page.getByRole('textbox', { name: `Rename ${name}` })).toBeFocused();
    await page.keyboard.press('Control+n');
    await page.waitForTimeout(300);
    await expect(newDialog(page)).toHaveCount(0);
  });
});

test.describe('in the Windows desktop app', () => {
  test.use({ userAgent: WINDOWS_UA });

  test('Ctrl+N opens the New dialog on the start screen and in both editors (AC-20)', async ({
    page,
  }) => {
    await installTauriMock(page);
    await gotoStart(page);
    const note = page.getByRole('dialog', { name: 'Open sprites and images with Pincel' });
    await note.getByRole('button', { name: 'Got it' }).click();
    await expect(note).toBeHidden();
    await expectCtrlNOpensNew(page);
  });
});
