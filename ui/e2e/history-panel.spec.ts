import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, menu, pixelCanvas, storedPref } from './helpers';
import {
  expectCurrent,
  expectRows,
  expectShot,
  historyList,
  historyPanel,
  historyRows,
  nth,
  openHistory,
  pixelStroke,
  settledShot,
} from './history-helpers';
import { installPicker, picker } from './tab-helpers';

// docs/specs/history-panel.md, Pixel mode and both modes' panel. Image-mode
// specifics are in `history-panel-image.spec.ts`.

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true }).first();
const redoButton = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true }).first();
const panelButton = (page: Page, name: string) =>
  historyPanel(page).getByRole('button', { name, exact: true });

async function pixelDoc(page: Page): Promise<void> {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openHistory(page);
}

test('both modes have a History panel, collapsed at first, its state kept per mode (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const header = historyPanel(page).getByRole('button', { name: 'History', exact: true });
  await expect(header).toHaveAttribute('aria-expanded', 'false');
  await expect(historyList(page)).toHaveCount(0);
  await header.click();
  await expect(historyList(page)).toBeVisible();
  await expect
    .poll(() => storedPref(page, 'layout:pixel'))
    .toMatchObject({ collapsed: { history: false } });

  // A new session of the same mode keeps it expanded …
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(historyList(page)).toBeVisible();
  // … the other mode keeps its own state: collapsed.
  await createDocument(page, 'image', { width: 16, height: 16 });
  const imageHeader = historyPanel(page).getByRole('button', { name: 'History', exact: true });
  await expect(imageHeader).toHaveAttribute('aria-expanded', 'false');
  await imageHeader.click();
  await expect(historyList(page)).toBeVisible();
});

test.describe('phone', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('the History panel is a tab of the bottom sheet (AC-01)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'Panels' }).click();
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await sheet.getByRole('tab', { name: 'History' }).click();
    await expect(sheet.getByRole('listbox', { name: 'History steps' })).toBeVisible();
    await expect(sheet.getByRole('option', { name: 'New' })).toBeVisible();
  });
});

test.describe('tablet', () => {
  test.use({ viewport: { width: 820, height: 1180 } });

  test('the History panel is in the drawer (AC-01)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'Toggle panels' }).click();
    const drawer = page.getByRole('complementary', { name: 'Panels' });
    await expect(drawer.getByRole('region', { name: 'History' })).toBeVisible();
  });
});

test('rows list the start entry, the undo steps and the dimmed redo steps; a new edit drops the redo rows (AC-02, AC-04, AC-06)', async ({
  page,
}) => {
  await pixelDoc(page);
  await expectRows(page, ['New']);
  await expectCurrent(page, 0);

  await pixelStroke(page, 0.2);
  await pixelStroke(page, 0.5);
  await page.getByRole('button', { name: 'Add layer' }).click();
  await menu(page, 'Image', 'Flip Horizontal');
  await expectRows(page, ['New', 'Pencil', 'Pencil', 'Add Layer', 'Flip Horizontal']);
  await expectCurrent(page, 4);

  await undoButton(page).click();
  await undoButton(page).click();
  await expectCurrent(page, 2);
  // Redo rows follow in the order Redo restores them, dimmed.
  await expectRows(page, ['New', 'Pencil', 'Pencil', 'Add Layer', 'Flip Horizontal']);
  for (const i of [0, 1, 2]) await expect(historyRows(page).nth(i)).toHaveCSS('opacity', '1');
  for (const i of [3, 4]) await expect(historyRows(page).nth(i)).toHaveCSS('opacity', '0.5');

  await page.getByTitle('Eraser (E)').click();
  await drag(page, pixelCanvas(page), [0.1, 0.2], [0.9, 0.2]);
  await expectRows(page, ['New', 'Pencil', 'Pencil', 'Eraser']);
  await expectCurrent(page, 3);
  for (const i of [0, 1, 2, 3]) await expect(historyRows(page).nth(i)).toHaveCSS('opacity', '1');
});

test('an effect step is named like the effect menu names it (AC-04)', async ({ page }) => {
  await pixelDoc(page);
  await pixelStroke(page, 0.5);
  await menu(page, 'Adjustments', 'Invert Colors');
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'OK' }).click();
  await expect(dialog).toHaveCount(0);
  await expectRows(page, ['New', 'Pencil', 'Invert Colors']);
});

test('a click, Enter or Space on a row jumps there as that many Undo / Redo presses would (AC-05)', async ({
  page,
}) => {
  await pixelDoc(page);
  const canvas = pixelCanvas(page);
  const shots = [await settledShot(canvas)];
  for (const y of [0.2, 0.5, 0.8]) {
    await pixelStroke(page, y);
    shots.push(await settledShot(canvas));
  }
  await expectCurrent(page, 3);

  await historyRows(page).nth(1).click();
  await expectCurrent(page, 1);
  await expectShot(canvas, nth(shots, 1));
  await expect(undoButton(page)).toBeEnabled();
  await expect(redoButton(page)).toBeEnabled();
  for (const i of [2, 3]) await expect(historyRows(page).nth(i)).toHaveCSS('opacity', '0.5');

  // The same as two Redo presses from here, then one Undo press.
  await redoButton(page).click();
  await redoButton(page).click();
  await expectShot(canvas, nth(shots, 3));
  await undoButton(page).click();
  await expectShot(canvas, nth(shots, 2));

  // The start entry: the state before the oldest step.
  await historyRows(page).first().click();
  await expectCurrent(page, 0);
  await expectShot(canvas, nth(shots, 0));
  await expect(undoButton(page)).toBeDisabled();

  // Enter and Space on the focused row.
  await historyList(page).focus();
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await expectCurrent(page, 3);
  await expectShot(canvas, nth(shots, 3));
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press(' ');
  await expectCurrent(page, 1);
  await expectShot(canvas, nth(shots, 1));

  // A click on the current row changes nothing.
  await historyRows(page).nth(1).click();
  await expectCurrent(page, 1);
  await expect(redoButton(page)).toBeEnabled();
});

test('Rewind, Undo, Redo and Fast-forward jump and are disabled when they cannot change anything (AC-07)', async ({
  page,
}) => {
  await pixelDoc(page);
  for (const name of ['Rewind', 'Undo', 'Redo', 'Fast-forward']) {
    await expect(panelButton(page, name)).toBeDisabled();
  }
  const canvas = pixelCanvas(page);
  const shots = [await settledShot(canvas)];
  for (const y of [0.2, 0.5, 0.8]) {
    await pixelStroke(page, y);
    shots.push(await settledShot(canvas));
  }
  await expect(panelButton(page, 'Redo')).toBeDisabled();
  await expect(panelButton(page, 'Fast-forward')).toBeDisabled();

  await panelButton(page, 'Undo').click();
  await expectCurrent(page, 2);
  await expectShot(canvas, nth(shots, 2));
  await panelButton(page, 'Rewind').click();
  await expectCurrent(page, 0);
  await expectShot(canvas, nth(shots, 0));
  await expect(panelButton(page, 'Rewind')).toBeDisabled();
  await expect(panelButton(page, 'Undo')).toBeDisabled();
  await expect(panelButton(page, 'Redo')).toBeEnabled();
  await expect(panelButton(page, 'Fast-forward')).toBeEnabled();

  await panelButton(page, 'Redo').click();
  await expectCurrent(page, 1);
  await expectShot(canvas, nth(shots, 1));
  await panelButton(page, 'Fast-forward').click();
  await expectCurrent(page, 3);
  await expectShot(canvas, nth(shots, 3));
  await expect(panelButton(page, 'Redo')).toBeDisabled();
  await expect(panelButton(page, 'Fast-forward')).toBeDisabled();
  await expect(panelButton(page, 'Rewind')).toBeEnabled();
});

test('the panel follows steps from tools, menus, shortcuts and the header buttons, with the current row in view (AC-08)', async ({
  page,
}) => {
  await pixelDoc(page);
  // A menu command, a tool, then Layer ▸ New Layer by its key.
  await menu(page, 'Image', 'Flip Horizontal');
  await pixelStroke(page, 0.5);
  await pixelCanvas(page).hover();
  for (let i = 0; i < 18; i += 1) await page.keyboard.press('Shift+N');
  await expect(historyRows(page)).toHaveCount(21);
  await expectCurrent(page, 20);
  await expect(historyRows(page).last()).toBeInViewport();
  await expect(historyRows(page).first()).not.toBeInViewport();

  await page.keyboard.press('Control+z');
  await expectCurrent(page, 19);
  await undoButton(page).click();
  await expectCurrent(page, 18);
  await page.keyboard.press('Control+y');
  await expectCurrent(page, 19);

  await panelButton(page, 'Rewind').click();
  await expectCurrent(page, 0);
  await expect(historyRows(page).first()).toBeInViewport();
  await expect(historyRows(page).last()).not.toBeInViewport();

  // A collapsed panel is up to date when it is expanded again.
  const header = historyPanel(page).getByRole('button', { name: 'History', exact: true });
  await header.click();
  await redoAll(page);
  await header.click();
  await expectCurrent(page, 20);
  await expect(historyRows(page).last()).toBeInViewport();
});

/** Fast-forward through the header's Redo and the shortcut while the panel
 *  is collapsed. */
async function redoAll(page: Page): Promise<void> {
  await redoButton(page).click();
  for (let i = 0; i < 19; i += 1) await page.keyboard.press('Control+y');
  await expect(redoButton(page)).toBeDisabled();
}

test('each document tab keeps its own history (AC-09)', async ({ page }) => {
  await installPicker(page);
  await pixelDoc(page);
  await pixelStroke(page, 0.5);
  await expectRows(page, ['New', 'Pencil']);
  await menu(page, 'File', 'New…');
  await page
    .getByRole('dialog', { name: 'New document' })
    .getByRole('button', { name: 'Create' })
    .click();
  await expectRows(page, ['New']);
  await page.getByRole('button', { name: 'Add layer' }).click();
  await expectRows(page, ['New', 'Add Layer']);
  await page.getByRole('tab').first().click();
  await expectRows(page, ['New', 'Pencil']);
  await page.getByRole('tab').nth(1).click();
  await expectRows(page, ['New', 'Add Layer']);
});

test('the list is one tab stop with listbox / option roles; arrows, Home and End move the focus without jumping (AC-10)', async ({
  page,
}) => {
  await pixelDoc(page);
  for (const y of [0.2, 0.5, 0.8]) await pixelStroke(page, y);
  const list = historyList(page);
  await expect(list).toHaveAttribute('tabindex', '0');
  for (let i = 0; i < 4; i += 1) {
    await expect(historyRows(page).nth(i)).toHaveAttribute('tabindex', '-1');
  }
  await expect(historyRows(page).nth(3)).toHaveAttribute('aria-selected', 'true');
  await expect(historyRows(page).nth(0)).toHaveAttribute('aria-selected', 'false');

  // Tab reaches the list from the panel's last button.
  // (Redo and Fast-forward are disabled, so the next stop is the list.)
  await panelButton(page, 'Undo').focus();
  await page.keyboard.press('Tab');
  await expect(list).toBeFocused();
  const focusedRow = async () => {
    const id = await list.getAttribute('aria-activedescendant');
    return historyRows(page).evaluateAll((rows, want) => rows.findIndex((r) => r.id === want), id);
  };
  await expect.poll(focusedRow).toBe(3);
  await page.keyboard.press('ArrowUp');
  await expect.poll(focusedRow).toBe(2);
  await expectCurrent(page, 3);
  await page.keyboard.press('Home');
  await expect.poll(focusedRow).toBe(0);
  await page.keyboard.press('ArrowUp');
  await expect.poll(focusedRow).toBe(0);
  await page.keyboard.press('End');
  await expect.poll(focusedRow).toBe(3);
  await page.keyboard.press('ArrowDown');
  await expect.poll(focusedRow).toBe(3);
  await page.keyboard.press('Home');
  await page.keyboard.press('ArrowDown');
  await expect.poll(focusedRow).toBe(1);
  await expectCurrent(page, 3);
  await page.keyboard.press('Enter');
  await expectCurrent(page, 1);
  // The next Tab leaves the list: the rows are no tab stops.
  await page.keyboard.press('Tab');
  await expect(list).not.toBeFocused();
  await expect(historyRows(page).nth(2)).not.toBeFocused();
});

test('requests are ignored while a stroke is in progress and the panel is inert while an effect is open (AC-11)', async ({
  page,
}) => {
  await pixelDoc(page);
  await pixelStroke(page, 0.3);
  await page.getByTitle('Pencil (B)').click();
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('no canvas');
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.7);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.7, { steps: 3 });
  await expectRows(page, ['New', 'Pencil', 'Pencil']);
  // A click on the start entry mid-stroke does nothing — not even later.
  await historyRows(page).first().dispatchEvent('click');
  await panelButton(page, 'Rewind').dispatchEvent('click');
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.7, { steps: 3 });
  await page.mouse.up();
  await page.waitForTimeout(200);
  await expectCurrent(page, 2);
  await expect(undoButton(page)).toBeEnabled();

  await menu(page, 'Adjustments', 'Invert Colors');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  const inert = await historyList(page).evaluate((el) => el.closest('[inert]') !== null);
  expect(inert).toBe(true);
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toHaveCount(0);
  await expectCurrent(page, 2);
});

test('a jump that fails stops at the last state reached and reports in the status bar (AC-12)', async ({
  page,
}) => {
  await pixelDoc(page);
  await pixelStroke(page, 0.5);
  await page.keyboard.press('m');
  await drag(page, pixelCanvas(page), [0.05, 0.3], [0.6, 0.7]);
  await page.keyboard.press('v');
  await drag(page, pixelCanvas(page), [0.3, 0.5], [0.3, 0.9]);
  await expectRows(page, ['New', 'Pencil', 'Move Selected Pixels']);
  await historyRows(page).first().click();
  await expectCurrent(page, 0);
  // Pixel selections are no steps: without one, the move cannot be redone.
  await page.keyboard.press('Control+d');
  await historyRows(page).last().click();
  await expectCurrent(page, 1);
  await expect(page.getByText(/^History jump stopped: /)).toBeVisible();
  await expectRows(page, ['New', 'Pencil', 'Move Selected Pixels']);
  await expect(historyRows(page).last()).toHaveCSS('opacity', '0.5');
  await expect(redoButton(page)).toBeEnabled();
});

test('a jump of many steps redraws the canvas once (AC-13)', async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as unknown as { canvasWrites: number };
    w.canvasWrites = 0;
    const put = CanvasRenderingContext2D.prototype.putImageData;
    CanvasRenderingContext2D.prototype.putImageData = function (
      this: CanvasRenderingContext2D,
      ...args: Parameters<typeof put>
    ) {
      if (this.canvas.getAttribute('aria-label') === 'Pincel canvas') w.canvasWrites += 1;
      return put.apply(this, args);
    } as typeof put;
  });
  await pixelDoc(page);
  const canvas = pixelCanvas(page);
  const start = await settledShot(canvas);
  for (let i = 0; i < 8; i += 1) await pixelStroke(page, 0.1 + i * 0.1);
  await expectCurrent(page, 8);
  await settledShot(canvas);
  const writes = () =>
    page.evaluate(() => (window as unknown as { canvasWrites: number }).canvasWrites);
  const before = await writes();
  await historyRows(page).first().click();
  await expectShot(canvas, start);
  await page.waitForTimeout(300);
  expect((await writes()) - before).toBe(1);
});

test('a jump writes no file and leaves the dirty state of as many undos (AC-14)', async ({
  page,
}) => {
  await installPicker(page);
  await pixelDoc(page);
  const title = (dirty: boolean) =>
    page.getByTitle(dirty ? 'pincel.aseprite — unsaved changes' : 'pincel.aseprite', {
      exact: true,
    });
  await expect(title(false)).toBeVisible();
  await pixelStroke(page, 0.3);
  await pixelStroke(page, 0.6);
  await expect(title(true)).toBeVisible();
  await historyRows(page).first().click();
  await expectCurrent(page, 0);
  await expect(title(false)).toBeVisible();
  await historyRows(page).nth(1).click();
  await expect(title(true)).toBeVisible();
  expect(await picker(page)).toEqual({ calls: 0, written: [] });
});

/** Rows in the app's `autosave_meta` store. */
async function autosaveRows(page: Page): Promise<number> {
  return page.evaluate(
    () =>
      new Promise<number>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const count = open.result
            .transaction('autosave_meta')
            .objectStore('autosave_meta')
            .count();
          count.onsuccess = () => {
            open.result.close();
            resolve(count.result);
          };
          count.onerror = () => reject(count.error);
        };
      }),
  );
}

test('a jump autosaves as the same undos and redos would (AC-14)', async ({ page }) => {
  await page.clock.install();
  await pixelDoc(page);
  await pixelStroke(page, 0.3);
  // Back at the state the document was created in: clean, nothing to keep.
  await historyRows(page).first().click();
  await expectCurrent(page, 0);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  expect(await autosaveRows(page)).toBe(0);
  // Forward again: unsaved work, snapshotted like after a Redo.
  await historyRows(page).nth(1).click();
  await expectCurrent(page, 1);
  await page.clock.fastForward(31_000);
  await expect.poll(() => autosaveRows(page)).toBe(1);
});

test('Space on a focused row jumps and starts no Space-pan (AC-18)', async ({ page }) => {
  await pixelDoc(page);
  await pixelStroke(page, 0.5);
  await historyList(page).focus();
  await page.keyboard.press('Home');
  await page.keyboard.down(' ');
  await expectCurrent(page, 0);
  await expect(pixelCanvas(page)).toHaveCSS('cursor', 'crosshair');
  await page.keyboard.up(' ');
  await expect(pixelCanvas(page)).toHaveCSS('cursor', 'crosshair');
});

test('in German, title, start entry, buttons and step names are German (AC-16)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await openHistory(page, 'Verlauf');
  const panel = historyPanel(page, 'Verlauf');
  for (const name of ['Zum Anfang', 'Rückgängig', 'Wiederholen', 'Zum Ende']) {
    await expect(panel.getByRole('button', { name, exact: true })).toHaveCount(1);
  }
  await page.getByTitle('Stift (B)').click();
  await drag(page, pixelCanvas(page), [0.1, 0.5], [0.9, 0.5]);
  const list = historyList(page, 'Verlauf', 'Verlaufsschritte');
  await expect
    .poll(async () => (await list.getByRole('option').allTextContents()).map((s) => s.trim()))
    .toEqual(['Neu', 'Stift']);
});
