import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  fakeFileHandles,
  fakeFs,
  fakeFsFile,
  imageCanvas,
  menu,
  setColor,
} from './helpers';
import { dropFiles, fileOf, prompt, tab, tabBar } from './tab-helpers';

// What commits a live object and what does not
// (docs/specs/image-live-objects-ui.md AC-09, AC-14, AC-15).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

function handles(page: Page): Locator {
  return page.locator('[data-live-handle]');
}

function undoButton(page: Page): Locator {
  return page.getByRole('button', { name: 'Undo', exact: true });
}

/** Page coordinates of a fractional canvas position. */
async function at(page: Page, [fx, fy]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + box.width * fx, box.y + box.height * fy];
}

/** The color shown at a fractional canvas position (screenshot, test side). */
async function shown(page: Page, [fx, fy]: [number, number]): Promise<string> {
  const png = await imageCanvas(page).screenshot();
  return page.evaluate(
    async ({ b64, fx, fy }) => {
      const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.drawImage(bitmap, 0, 0);
      const x = Math.min(bitmap.width - 1, Math.floor(bitmap.width * fx));
      const y = Math.min(bitmap.height - 1, Math.floor(bitmap.height * fy));
      const [r = 0, g = 0, b = 0] = ctx.getImageData(x, y, 1, 1).data;
      return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
    },
    { b64: png.toString('base64'), fx, fy },
  );
}

/** A live red rectangle over the canvas fractions 0.2 to 0.4. */
async function liveRedBox(page: Page): Promise<void> {
  await setColor(page, 'fg', '#000000');
  await setColor(page, 'bg', '#ff0000');
  await page.keyboard.press('u');
  await page
    .getByRole('radiogroup', { name: 'Shapes variant' })
    .getByRole('radio', { name: 'Rectangle', exact: true })
    .click();
  await page.getByRole('combobox', { name: 'Mode', exact: true }).selectOption('fill');
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.4, 0.4]);
  await expect(handles(page)).toHaveCount(8);
}

/** Undoes `steps` steps and expects nothing left to undo. */
async function undoAll(page: Page, steps: number): Promise<void> {
  for (let i = 0; i < steps; i += 1) await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
}

test('layer, document and selection actions commit the live object first', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  await page.keyboard.press('Shift+N'); // New Layer
  await expect(handles(page)).toHaveCount(0);
  expect(await shown(page, [0.3, 0.3])).toBe('#ff0000');

  // Another active layer commits.
  await liveRedBox(page);
  await page.keyboard.press('Alt+ArrowDown');
  await expect(handles(page)).toHaveCount(0);

  // A selection action commits.
  await liveRedBox(page);
  await page.keyboard.press('Control+a');
  await expect(handles(page)).toHaveCount(0);
  // box, layer, box, box, select all
  await undoAll(page, 5);
});

test('Copy, an effect, Export and Save commit first', async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  await page.keyboard.press('Control+c');
  await expect(handles(page)).toHaveCount(0);
  await undoAll(page, 1);

  await liveRedBox(page);
  await menu(page, 'Adjustments', 'Levels…');
  await expect(handles(page)).toHaveCount(0);
  await page.keyboard.press('Escape');
  await undoAll(page, 1);

  await liveRedBox(page);
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    menu(page, 'File', 'Export', 'PNG'),
  ]);
  await download.path();
  await expect(handles(page)).toHaveCount(0);
  await undoAll(page, 1);

  await liveRedBox(page);
  const [saved] = await Promise.all([page.waitForEvent('download'), page.keyboard.press('Control+s')]);
  await saved.path();
  await expect(handles(page)).toHaveCount(0);
  await expect(page).not.toHaveTitle(/^● /);
});

test('view actions and the right button keep the object live', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  const [cx, cy] = await at(page, [0.7, 0.7]);
  await page.mouse.move(cx, cy);
  await page.mouse.wheel(0, -200);
  await page.keyboard.down('Space');
  await page.mouse.down();
  await page.mouse.move(cx + 30, cy + 20, { steps: 4 });
  await page.mouse.up();
  await page.keyboard.up('Space');
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(cx - 20, cy - 10, { steps: 4 });
  await page.mouse.up({ button: 'middle' });
  const [rx, ry] = await at(page, [0.8, 0.8]);
  await page.mouse.click(rx, ry, { button: 'right' });
  await expect(handles(page)).toHaveCount(8);
  await undoAll(page, 1);
  await expect(handles(page)).toHaveCount(0);
});

test('switching to another tab commits in the tab the object belongs to', async ({ page }) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  await dropFiles(page, [await fileOf('image', 'other')]);
  await expect(tab(page, 'other.png')).toHaveAttribute('aria-selected', 'true');
  await tab(page, 'image.ora').click();
  await expect(tab(page, 'image.ora')).toHaveAttribute('aria-selected', 'true');
  await expect(handles(page)).toHaveCount(0);
  expect(await shown(page, [0.3, 0.3])).toBe('#ff0000');
  // Committed, not just hidden: Redo after Undo brings back pixels, no handles.
  await undoAll(page, 1);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  expect(await shown(page, [0.3, 0.3])).toBe('#ff0000');
  await expect(handles(page)).toHaveCount(0);
});

test('autosave keeps the object live; a recovered document has it as pixels', async ({ page }) => {
  await page.clock.install();
  await createDocument(page, 'image', { width: 64, height: 64 });
  await liveRedBox(page);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  await expect(handles(page)).toHaveCount(8);
  await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled();

  await page.reload();
  await createDocument(page, 'image', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await dialog.getByRole('button', { name: 'Recover' }).first().click();
  await expect(dialog).toHaveCount(0);
  await expect(handles(page)).toHaveCount(0);
  await expect.poll(() => shown(page, [0.3, 0.3])).toBe('#ff0000');
});

test('closing a tab with a live object asks; Save commits and saves it', async ({ page }) => {
  await fakeFileHandles(page, {});
  await createDocument(page, 'image', { width: 64, height: 64 });
  await fakeFs(page, { savePick: 'kept.ora' });
  await liveRedBox(page);
  await tabBar(page).getByRole('button', { name: 'Close image.ora' }).click();
  await expect(prompt(page)).toBeVisible();
  await expect(prompt(page)).toContainText('image.ora');
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await expect(prompt(page)).toHaveCount(0);

  // The file has the object: open it again.
  const bytes = await fakeFsFile(page, 'kept.ora');
  expect(bytes.length).toBeGreaterThan(0);
  await createDocument(page, 'image', { width: 8, height: 8 });
  await dropFiles(page, [{ name: 'kept.ora', bytes }]);
  await expect(tab(page, 'kept.ora')).toHaveAttribute('aria-selected', 'true');
  await expect.poll(() => shown(page, [0.3, 0.3])).toBe('#ff0000');
  await expect(handles(page)).toHaveCount(0);
});
