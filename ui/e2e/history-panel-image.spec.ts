import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, pixelCanvas, putClipboardImage } from './helpers';
import {
  expectCurrent,
  expectRows,
  expectShot,
  historyRows,
  nth,
  openHistory,
  rowNames,
  settledShot,
} from './history-helpers';
import { dropFiles, expectActive, fileOf, type Mode } from './tab-helpers';

// docs/specs/history-panel.md: Image mode, and the start entry of both
// modes. The panel itself is covered in `history-panel.spec.ts`.

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

async function imageDoc(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await openHistory(page);
}

test('Image steps are named after their tools and menu commands; a row click jumps (AC-02, AC-04, AC-05)', async ({
  page,
}) => {
  await imageDoc(page);
  const canvas = imageCanvas(page);
  const shots = [await settledShot(canvas)];
  await drag(page, canvas, [0.1, 0.3], [0.9, 0.3]);
  shots.push(await settledShot(canvas));
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+d');
  await page.getByRole('button', { name: 'Add layer' }).click();
  await drag(page, canvas, [0.1, 0.7], [0.9, 0.7]);
  await menu(page, 'Layer', 'Merge Down');
  await expectRows(page, [
    'New',
    'Brush',
    'Select All',
    'Deselect',
    'Add Layer',
    'Brush',
    'Merge Down',
  ]);
  await expectCurrent(page, 6);

  await historyRows(page).nth(1).click();
  await expectCurrent(page, 1);
  await expectShot(canvas, nth(shots, 1));
  for (const i of [2, 3, 4, 5, 6]) {
    await expect(historyRows(page).nth(i)).toHaveCSS('opacity', '0.5');
  }
  await historyRows(page).first().click();
  await expectShot(canvas, nth(shots, 0));
  await expect(page.getByRole('button', { name: 'Undo', exact: true }).first()).toBeDisabled();

  // A new edit drops the redo rows (AC-06).
  await page.keyboard.press('Control+a');
  await expectRows(page, ['New', 'Select All']);
  await expectCurrent(page, 1);
});

test('Image: a click mid-stroke is ignored (AC-11)', async ({ page }) => {
  await imageDoc(page);
  const canvas = imageCanvas(page);
  await drag(page, canvas, [0.1, 0.3], [0.9, 0.3]);
  const box = await canvas.boundingBox();
  if (!box) throw new Error('no canvas');
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.7);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.7, { steps: 3 });
  await historyRows(page).first().dispatchEvent('click');
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.7, { steps: 3 });
  await page.mouse.up();
  await page.waitForTimeout(200);
  await expectRows(page, ['New', 'Brush', 'Brush']);
  await expectCurrent(page, 2);
});

test('Image: each document tab keeps its own history (AC-09)', async ({ page }) => {
  await imageDoc(page);
  await drag(page, imageCanvas(page), [0.1, 0.3], [0.9, 0.3]);
  await dropFiles(page, [await fileOf('image', 'b')]);
  await expectActive(page, 'b.png');
  await expectRows(page, ['Open']);
  await page.keyboard.press('Control+a');
  await expectRows(page, ['Open', 'Select All']);
  await page.getByRole('tab').first().click();
  await expectRows(page, ['New', 'Brush']);
  await page.getByRole('tab').nth(1).click();
  await expectRows(page, ['Open', 'Select All']);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the start entry names a new and an opened document (AC-03)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await openHistory(page);
    await expectRows(page, ['New']);
    await dropFiles(page, [await fileOf(mode, 'a')]);
    await expectActive(page, `a.${mode === 'pixel' ? 'aseprite' : 'png'}`);
    await expectRows(page, ['Open']);
  });

  test.describe('clipboard', () => {
    test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

    test(`${mode}: the start entry of Paste as New Image is "Paste" (AC-03)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await openHistory(page);
      await putClipboardImage(page, 8, 8, '#00ff00');
      await page.keyboard.press('ControlOrMeta+Alt+v');
      await expectActive(page, `Pasted image.${mode === 'pixel' ? 'aseprite' : 'png'}`);
      await expectRows(page, ['Paste']);
    });
  });

  test(`${mode}: a recovered document starts with "Recovered" (AC-03)`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.6]);
    await page.clock.fastForward(31_000);
    await page.waitForTimeout(500);
    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
    await dialog.getByRole('button', { name: 'Recover' }).first().click();
    await expect(dialog).toHaveCount(0);
    await openHistory(page);
    await expectRows(page, ['Recovered']);
  });

  test(`${mode}: once the 100-step limit drops a step the start entry is "Oldest available state" (AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await openHistory(page);
    const step = async (i: number) => {
      if (mode === 'image') {
        await page.keyboard.press(i % 2 === 0 ? 'Control+a' : 'Control+d');
      } else {
        const box = await pixelCanvas(page).boundingBox();
        if (!box) throw new Error('no canvas');
        await page.mouse.click(box.x + ((i % 16) + 0.5) * (box.width / 16), box.y + 2);
      }
    };
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    for (let i = 0; i < 100; i += 1) await step(i);
    await expect(historyRows(page)).toHaveCount(101);
    expect((await rowNames(page))[0]).toBe('New');
    await step(100);
    await expect.poll(async () => (await rowNames(page))[0]).toBe('Oldest available state');
    await expect(historyRows(page)).toHaveCount(101);
  });
}
