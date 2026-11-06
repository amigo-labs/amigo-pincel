import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { menu, waitForEditor } from './helpers';
import { centre, fill, type Mode } from './effect-panel-helpers';
import {
  emit,
  installTauriMock,
  launchDesktop,
  nativeSave,
  putFile,
  setMock,
} from './tauri-mock';

// Saving, closing and opening while an effect is open (effect-panel AC-17),
// in the desktop app against the mocked Tauri bridge of `tauri-mock.ts`.
// The web menu bar's File menu is inert while an effect is open (AC-03);
// the native menu is how a save, a close or an open reaches the editor
// then.

const SPRITE = readFile(
  join(
    import.meta.dirname,
    '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite',
  ),
);

const invertPanel = (page: Page): Locator => page.getByRole('dialog', { name: 'Invert Colors' });
const undoButton = (page: Page): Locator =>
  page.getByRole('button', { name: 'Undo', exact: true });

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

/** A new 16 × 16 document of `mode` from the start screen. */
async function create(page: Page, mode: Mode): Promise<void> {
  await launchDesktop(page);
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

/** A `size`² PNG of one colour, drawn in the page. */
async function png(page: Page, size: number, color: string): Promise<Buffer> {
  const bytes = await page.evaluate(
    async ([s, c]) => {
      const canvas = new OffscreenCanvas(s as number, s as number);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = c as string;
      ctx.fillRect(0, 0, s as number, s as number);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      return Array.from(new Uint8Array(await blob.arrayBuffer()));
    },
    [size, color] as const,
  );
  return Buffer.from(bytes);
}

async function openInvert(page: Page): Promise<void> {
  await menu(page, 'Adjustments', 'Invert Colors');
  await expect(invertPanel(page)).toBeVisible();
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a save while an effect is open writes the document without the preview (AC-17)`, async ({
    page,
  }) => {
    let path: string;
    if (mode === 'pixel') {
      path = '/docs/sprite.aseprite';
      await create(page, 'pixel');
      await setMock(page, { savePick: path });
    } else {
      // An opened PNG saves back to its path as PNG, byte-stable across saves.
      path = '/docs/tile.png';
      await launchDesktop(page);
      await putFile(page, path, await png(page, 16, '#336699'));
      await setMock(page, { openPick: path });
      await page.getByRole('button', { name: 'Open…' }).click();
      await waitForEditor(page, 'image');
    }
    await fill(page, mode);
    const filled = await centre(page, mode);

    await openInvert(page);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(false);
    const withPreview = await nativeSave(page, path);
    await expect(invertPanel(page)).toBeVisible();
    await invertPanel(page).getByRole('button', { name: 'Cancel' }).click();
    const without = await nativeSave(page, path);
    expect(withPreview.length).toBeGreaterThan(0);
    expect(withPreview.equals(without)).toBe(true);
  });

  test(`${mode}: the unsaved-changes prompt cancels the open effect first (AC-17)`, async ({
    page,
  }) => {
    await create(page, mode);
    await fill(page, mode);
    const filled = await centre(page, mode);
    await openInvert(page);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(false);

    // Since document-tabs only closing a tab (or the app) asks.
    await emit(page, 'menu', 'menu:close');
    const prompt = page.getByRole('alertdialog');
    await expect(prompt).toBeVisible();
    await expect(invertPanel(page)).toHaveCount(0);
    await prompt.getByRole('button', { name: 'Cancel' }).click();
    await expect(prompt).toHaveCount(0);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(true);
    await expect(invertPanel(page)).toHaveCount(0);
  });
}

test('pixel: opening a file while an effect is open cancels the effect (AC-17)', async ({
  page,
}) => {
  await create(page, 'pixel');
  await putFile(page, '/docs/b.aseprite', await SPRITE);
  await setMock(page, { openPick: '/docs/b.aseprite' });
  await openInvert(page);

  // The file opens in a new tab (document-tabs); the effect ends with the
  // tab it was open on.
  await emit(page, 'menu', 'menu:open');
  await expect(page.getByRole('tab', { name: /^b\.aseprite( |$)/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(invertPanel(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeDisabled();
});
