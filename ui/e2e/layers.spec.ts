import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  waitForEditor,
} from './helpers';

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

// docs/specs/pixel-layers-frames.md: the Pixel-mode panel and Layer menu.

/** The `undo n / redo m` readout of the Pixel status bar. */
const depths = (page: Page, undo: number, redo = 0) =>
  page.getByText(`undo ${undo} / redo ${redo}`, { exact: true });

/** The drag grip of layer `name`, which also takes the reorder keys. */
const grip = (page: Page, name: string) =>
  page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true });

test('pixel: a new layer is named by the stack size, becomes active and takes the next stroke (AC-02)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect.poll(() => layerNames(page)).toEqual(['Layer 1']);
  const canvas = pixelCanvas(page);
  await page.mouse.move(0, 0);
  const blank = await canvas.screenshot();

  await page.getByRole('button', { name: 'Add layer' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Layer 2', 'Layer 1']);
  await expect(layerRow(page, 'Layer 2')).toHaveClass(/layer-active/);
  await expect(layerRow(page, 'Layer 1')).not.toHaveClass(/layer-active/);

  // Layer ▸ New Layer counts every layer of the stack, groups included.
  await page.getByRole('button', { name: 'Add group' }).click();
  await menu(page, 'Layer', 'New Layer');
  await expect.poll(() => layerNames(page)).toEqual(['Layer 4', 'Group 1', 'Layer 2', 'Layer 1']);
  await expect(layerRow(page, 'Layer 4')).toHaveClass(/layer-active/);
  await expect(layerRow(page, 'Group 1')).not.toHaveClass(/layer-active/);

  // The next stroke lands in the new layer: hiding it hides the stroke.
  await page.getByTitle('Pencil (B)').click();
  await drag(page, canvas, [0.2, 0.5], [0.8, 0.5]);
  await page.mouse.move(0, 0);
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(false);
  await page.getByRole('button', { name: 'Hide Layer 4', exact: true }).click();
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(true);
});

test('pixel: Delete is unavailable while the stack holds one layer (AC-06)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const button = page.getByRole('button', { name: 'Delete layer' });
  const menuEntry = async () => {
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Layer', exact: true }).click();
    const item = page
      .getByRole('menu', { name: 'Layer', exact: true })
      .getByRole('menuitem', { name: 'Delete Layer', exact: true });
    await expect(item).toBeVisible();
    return item;
  };

  await expect.poll(() => layerNames(page)).toHaveLength(1);
  await expect(button).toBeDisabled();
  await expect(await menuEntry()).toBeDisabled();
  await page.keyboard.press('Escape');

  await page.getByRole('button', { name: 'Add layer' }).click();
  await expect(button).toBeEnabled();
  await expect(await menuEntry()).toBeEnabled();
  await page.keyboard.press('Escape');

  await button.click();
  await expect.poll(() => layerNames(page)).toHaveLength(1);
  await expect(button).toBeDisabled();
  await expect(await menuEntry()).toBeDisabled();
});

test('pixel: inline rename commits the trimmed name on Enter; Escape, blank and unchanged do nothing (AC-08)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect.poll(() => layerNames(page)).toEqual(['Layer 1']);
  const rename = async (name: string) => {
    await page.getByRole('button', { name: `Rename ${name}`, exact: true }).click();
    const field = page.getByRole('textbox', { name: `Rename ${name}` });
    // The field opens focused with the old name selected.
    await expect(field).toBeFocused();
    return field;
  };

  let field = await rename('Layer 1');
  await field.fill('Ground');
  await field.press('Escape');
  await expect(field).toHaveCount(0);
  await expect.poll(() => layerNames(page)).toEqual(['Layer 1']);
  await expect(depths(page, 0)).toBeVisible();

  for (const typed of ['   ', 'Layer 1', '  Layer 1 ']) {
    field = await rename('Layer 1');
    await field.fill(typed);
    await field.press('Enter');
    await expect(field).toHaveCount(0);
    await expect.poll(() => layerNames(page)).toEqual(['Layer 1']);
    await expect(depths(page, 0)).toBeVisible();
  }

  field = await rename('Layer 1');
  await field.fill('  Ground  ');
  await field.press('Enter');
  await expect.poll(() => layerNames(page)).toEqual(['Ground']);
  await expect(depths(page, 1)).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Layer 1']);
  await expect(depths(page, 0, 1)).toBeVisible();
});

test('pixel: the panel lists the layer tree top-most first, indented, with kind badges (AC-09)', async ({
  page,
}) => {
  await page.addInitScript(() => {
    delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
  });
  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({
    name: 'hero.aseprite',
    mimeType: 'application/octet-stream',
    buffer: await readFile(
      join(
        import.meta.dirname,
        '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite',
      ),
    ),
  });
  await waitForEditor(page, 'pixel');

  // The file's stack, bottom to top: Background, Characters { Hero },
  // Hidden Notes, Tiles (tilemap).
  await expect
    .poll(() => layerNames(page))
    .toEqual(['Tiles', 'Hidden Notes', 'Characters', 'Hero', 'Background']);
  await expect(layerRow(page, 'Characters')).toHaveAttribute('data-depth', '0');
  await expect(layerRow(page, 'Hero')).toHaveAttribute('data-depth', '1');
  await expect(layerRow(page, 'Background')).toHaveAttribute('data-depth', '0');
  const badge = (name: string, kind: string) =>
    layerRow(page, name).getByText(kind, { exact: true });
  await expect(badge('Characters', 'group')).toBeVisible();
  await expect(badge('Tiles', 'tilemap')).toBeVisible();
  for (const plain of ['Hidden Notes', 'Hero', 'Background']) {
    await expect(badge(plain, 'group')).toHaveCount(0);
    await expect(badge(plain, 'tilemap')).toHaveCount(0);
  }

  // One level of indent per enclosing group: a group inside Characters,
  // Hero inside that group.
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toContain('Group 2');
  await dropOnRow(page, 'Group 2', 'Characters', 0.5);
  await expect(layerRow(page, 'Group 2')).toHaveAttribute('data-depth', '1');
  await dropOnRow(page, 'Hero', 'Group 2', 0.5);
  await expect
    .poll(() => layerNames(page))
    .toEqual(['Tiles', 'Hidden Notes', 'Characters', 'Group 2', 'Hero', 'Background']);
  await expect(layerRow(page, 'Hero')).toHaveAttribute('data-depth', '2');
  const indent = async (name: string) =>
    parseFloat(await layerRow(page, name).evaluate((el) => getComputedStyle(el).paddingLeft));
  const one = await indent('Group 2');
  expect(await indent('Characters')).toBe(0);
  expect(one).toBeGreaterThan(0);
  expect(await indent('Hero')).toBeCloseTo(2 * one);
});

test('pixel: arrow keys on a grip move the layer, Alt+→ / Alt+← in and out of a group (AC-12)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByRole('button', { name: 'Add layer' }).click();
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', 'Layer 2', 'Layer 1']);

  await grip(page, 'Layer 1').focus();
  await page.keyboard.press('ArrowUp');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', 'Layer 1', 'Layer 2']);
  await grip(page, 'Layer 1').focus();
  await page.keyboard.press('ArrowDown');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', 'Layer 2', 'Layer 1']);

  // Into the nearest group above at the same depth, as its top child.
  await grip(page, 'Layer 2').focus();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(layerRow(page, 'Layer 2')).toHaveAttribute('data-depth', '1');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', 'Layer 2', 'Layer 1']);
  await grip(page, 'Layer 1').focus();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(layerRow(page, 'Layer 1')).toHaveAttribute('data-depth', '1');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', 'Layer 1', 'Layer 2']);

  // Out of the group: just above it.
  await grip(page, 'Layer 2').focus();
  await page.keyboard.press('Alt+ArrowLeft');
  await expect(layerRow(page, 'Layer 2')).toHaveAttribute('data-depth', '0');
  await expect.poll(() => layerNames(page)).toEqual(['Layer 2', 'Group 1', 'Layer 1']);
  await expect(layerRow(page, 'Layer 1')).toHaveAttribute('data-depth', '1');
});

test('pixel: a locked group locks its layers without setting their own flag (AC-17)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', 'Layer 1']);
  await grip(page, 'Layer 1').focus();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(layerRow(page, 'Layer 1')).toHaveAttribute('data-depth', '1');

  await page.getByRole('button', { name: 'Lock Group 1', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Unlock Group 1', exact: true })).toBeVisible();
  // The child's own flag stays off; its icon reads locked, by its group.
  const childLock = page.getByRole('button', { name: 'Lock Layer 1', exact: true });
  await expect(childLock).toHaveAttribute('aria-pressed', 'false');
  await expect(childLock).toHaveAttribute('title', 'Locked by its group');

  // It refuses paint like a locked layer: a notice, no undo step.
  await layerRow(page, 'Layer 1').getByRole('button', { name: 'Layer 1', exact: true }).click();
  await page.getByTitle('Pencil (B)').click();
  await expect(depths(page, 3)).toBeVisible();
  await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await expect(page.getByText('Layer “Layer 1” is locked')).toBeVisible();
  await expect(depths(page, 3)).toBeVisible();

  // Unlocking the group lets the layer take paint again.
  await page.getByRole('button', { name: 'Unlock Group 1', exact: true }).click();
  await expect(childLock).not.toHaveAttribute('title', 'Locked by its group');
  await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await expect(depths(page, 5)).toBeVisible();
  await expect(page.getByText('Layer “Layer 1” is locked')).toHaveCount(0);
});
