import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, pixelCanvas } from './helpers';

// The layers panel both modes share (spec §9.2): rename from the menu,
// lock, drag-to-reorder on the grip, thumbnails; in Pixel mode, dragging
// layers into and out of groups.

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
  // Measure only what is on screen: a panel that is still rendering hands
  // back a null box and the drag starts from nowhere.
  await grip.waitFor({ state: 'visible' });
  await target.waitFor({ state: 'visible' });
  const a = await grip.boundingBox();
  const b = await target.boundingBox();
  if (!a || !b) throw new Error('layer rows not visible');
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 6 });
  await page.mouse.up();
}

/** The panel row of layer `name`. */
function layerRow(page: Page, name: string) {
  return page
    .getByRole('list', { name: 'Layer list' })
    .locator('li', { has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }) });
}

/** Drags layer `from` onto row `to`, released at height `fy` (0 = top) of the row. */
async function dropOnRow(page: Page, from: string, to: string, fy: number): Promise<void> {
  const grip = page.getByRole('button', { name: `Drag to reorder ${from}`, exact: true });
  const row = layerRow(page, to);
  await grip.waitFor({ state: 'visible' });
  await row.waitFor({ state: 'visible' });
  const a = await grip.boundingBox();
  const b = await row.boundingBox();
  if (!a || !b) throw new Error('layer rows not visible');
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height * fy, { steps: 6 });
  await page.mouse.up();
}

test('pixel: drag a layer into a group and back out', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await expect.poll(() => layerNames(page)).toHaveLength(1);
  const [layer] = (await layerNames(page)) as [string];
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', layer]);

  // Onto the middle of the group row: the layer goes in, indented.
  await dropOnRow(page, layer, 'Group 1', 0.5);
  await expect(layerRow(page, layer)).toHaveAttribute('data-depth', '1');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', layer]);

  // Onto the top edge of the group row: out again, above the group.
  await dropOnRow(page, layer, 'Group 1', 0.1);
  await expect(layerRow(page, layer)).toHaveAttribute('data-depth', '0');
  await expect.poll(() => layerNames(page)).toEqual([layer, 'Group 1']);

  // Each drop is one undo step.
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(layerRow(page, layer)).toHaveAttribute('data-depth', '1');
});

test('image: duplicate, opacity, blend mode and visibility show in the panel and undo', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await expect.poll(() => layerNames(page)).toHaveLength(1);
  const [name] = (await layerNames(page)) as [string];
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  const props = page.getByRole('group', { name: 'Layer properties' });
  const opacity = page.getByLabel('Layer opacity');
  const blend = page.getByLabel('Layer blend mode');

  // Duplicate: a second row appears; one Undo removes it.
  await page.getByRole('button', { name: 'Duplicate layer' }).click();
  await expect.poll(() => layerNames(page)).toHaveLength(2);
  await undo.click();
  await expect.poll(() => layerNames(page)).toEqual([name]);

  // Opacity: the slider and its readout follow; one Undo restores 100 %.
  await expect(opacity).toHaveValue('100');
  await opacity.fill('40');
  await expect(opacity).toHaveValue('40');
  await expect(props).toContainText('40%');
  await undo.click();
  await expect(opacity).toHaveValue('100');
  await expect(props).toContainText('100%');

  // Blend mode: the select shows it; one Undo restores Normal.
  await expect(blend).toHaveValue('normal');
  await blend.selectOption('multiply');
  await expect(blend).toHaveValue('multiply');
  await undo.click();
  await expect(blend).toHaveValue('normal');

  // Visibility: the eye button flips to "Show"; one Undo brings it back.
  await page.getByRole('button', { name: `Hide ${name}` }).click();
  await expect(page.getByRole('button', { name: `Show ${name}` })).toBeVisible();
  await undo.click();
  await expect(page.getByRole('button', { name: `Hide ${name}` })).toBeVisible();
  await expect(undo).toBeDisabled();
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: Layer ▸ Rename… renames the active layer`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await expect.poll(() => layerNames(page)).toHaveLength(1);
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
    const add = page.getByRole('button', { name: 'Add layer' });
    await add.click();
    await expect.poll(() => layerNames(page)).toHaveLength(2);
    await add.click();
    await expect.poll(() => layerNames(page)).toHaveLength(3);
    const before = await layerNames(page);
    const [top, , bottom] = before as [string, string, string];
    await dragLayer(page, bottom, top);
    await expect.poll(() => layerNames(page)).toEqual([bottom, before[0], before[1]]);
    // The whole drag is one undo step.
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect.poll(() => layerNames(page)).toEqual(before);
  });

  test(`${mode}: a locked layer refuses paint`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await expect.poll(() => layerNames(page)).toHaveLength(1);
    const [name] = (await layerNames(page)) as [string];
    // Exact: "Unlock …" contains "lock …", and role names match case-insensitively.
    const lock = page.getByRole('button', { name: `Lock ${name}`, exact: true });
    const unlock = page.getByRole('button', { name: `Unlock ${name}`, exact: true });
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const redo = page.getByRole('button', { name: 'Redo', exact: true });
    await lock.click();
    await expect(unlock).toBeVisible();
    const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(page, canvas, [0.2, 0.2], [0.8, 0.6]);
    await expect(page.getByText(`“${name}” is locked`)).toBeVisible();

    // The refused stroke left no undo step: Undo reverts the lock itself.
    await undo.click();
    await expect(lock).toBeVisible();
    await redo.click();
    await expect(unlock).toBeVisible();

    // Unlocked, the layer accepts paint again: Undo now reverts the stroke
    // and leaves the layer unlocked.
    await unlock.click();
    await expect(lock).toBeVisible();
    await drag(page, canvas, [0.2, 0.2], [0.8, 0.6]);
    await expect(page.getByText(`“${name}” is locked`)).toHaveCount(0);
    await undo.click();
    await expect(lock).toBeVisible();
    await undo.click();
    await expect(unlock).toBeVisible();
  });
}
