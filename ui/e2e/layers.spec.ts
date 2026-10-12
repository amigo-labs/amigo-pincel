import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, pixelCanvas } from './helpers';

// The layers panel both modes share (spec §9.2): rename from the menu,
// lock, drag-to-reorder on the grip, thumbnails.

/** Layer names in panel order (top-most first). */
async function layerNames(page: Page): Promise<string[]> {
  const list = page.getByRole('list', { name: 'Layer list' });
  return list.getByRole('button', { name: /^Drag to reorder / }).evaluateAll((els) =>
    els.map((el) => (el.getAttribute('aria-label') ?? '').replace('Drag to reorder ', '')),
  );
}

/** Drags the grip of layer `from` onto the row of layer `to`. */
async function dragLayer(page: Page, from: string, to: string): Promise<void> {
  const grip = page.getByRole('button', { name: `Drag to reorder ${from}` });
  const target = page.getByRole('button', { name: `Drag to reorder ${to}` });
  const a = await grip.boundingBox();
  const b = await target.boundingBox();
  if (!a || !b) throw new Error('layer rows not visible');
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 6 });
  await page.mouse.up();
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: Layer ▸ Rename… renames the active layer`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const [first] = await layerNames(page);
    await menu(page, 'Layer', 'Rename…');
    const dialog = page.getByRole('dialog', { name: 'Rename layer' });
    await dialog.getByLabel('Name').fill('Ground');
    await dialog.getByRole('button', { name: 'Rename' }).click();
    await expect(dialog).toBeHidden();
    await expect.poll(() => layerNames(page)).toEqual(['Ground']);
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect.poll(() => layerNames(page)).toEqual([first]);
  });

  test(`${mode}: drag the grip to reorder layers`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await page.getByRole('button', { name: 'Add layer' }).click();
    await page.getByRole('button', { name: 'Add layer' }).click();
    const before = await layerNames(page);
    expect(before).toHaveLength(3);
    const [top, , bottom] = before as [string, string, string];
    await dragLayer(page, bottom, top);
    await expect.poll(() => layerNames(page)).toEqual([bottom, before[0], before[1]]);
    // The whole drag is one undo step.
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect.poll(() => layerNames(page)).toEqual(before);
  });

  test(`${mode}: a locked layer refuses paint`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const [name] = (await layerNames(page)) as [string];
    await page.getByRole('button', { name: `Lock ${name}` }).click();
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const depth = await undo.isEnabled();
    const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(page, canvas, [0.2, 0.2], [0.8, 0.6]);
    await expect(page.getByText(`“${name}” is locked`)).toBeVisible();
    await page.getByRole('button', { name: `Unlock ${name}` }).click();
    await drag(page, canvas, [0.2, 0.2], [0.8, 0.6]);
    await expect(page.getByText(`“${name}” is locked`)).toHaveCount(0);
    expect(depth || (await undo.isEnabled())).toBe(true);
  });
}
