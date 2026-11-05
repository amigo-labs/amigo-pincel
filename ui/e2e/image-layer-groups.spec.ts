import { expect, test, type Page } from '@playwright/test';
import {
  clearClipboard,
  copiedImage,
  createDocument,
  drag,
  imageCanvas,
  menu,
  pixelOf,
  type ClipImage,
} from './helpers';

// Layer groups in Image mode (docs/specs/image-layer-groups.md): New Group,
// drag and Alt+←/→ into and out of groups, group visibility and lock,
// collapse, Merge Group, delete. The composite is read through Edit ▸ Copy
// Merged (the clipboard); the panel through its rows' labels and depth.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const RED = [255, 0, 0, 255];
const CLEAR = [0, 0, 0, 0];

/** Layer names in panel order (top-most first), collapsed rows left out. */
async function layerNames(page: Page): Promise<string[]> {
  const list = page.getByRole('list', { name: 'Layer list' });
  return list
    .getByRole('button', { name: /^Drag to reorder / })
    .evaluateAll((els) =>
      els.map((el) => (el.getAttribute('aria-label') ?? '').replace('Drag to reorder ', '')),
    );
}

/** The panel row of layer `name`. */
function layerRow(page: Page, name: string) {
  return page.getByRole('list', { name: 'Layer list' }).locator('li', {
    has: page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true }),
  });
}

/** Makes layer `name` active through its name button (which also shows the
 *  group badge, so its accessible name only starts with the layer name). */
async function selectRow(page: Page, name: string): Promise<void> {
  await layerRow(page, name)
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click();
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

/** The document composite, read through Edit ▸ Copy Merged. */
async function composite(page: Page): Promise<ClipImage> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy Merged');
  return copiedImage(page);
}

/** A 16×16 Image document whose only layer is filled red; returns its name. */
async function redDocument(page: Page): Promise<string> {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect.poll(() => layerNames(page)).toHaveLength(1);
  const [name] = (await layerNames(page)) as [string];
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await page.keyboard.press('g');
  await imageCanvas(page).click({ position: { x: 4, y: 4 } });
  return name;
}

const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

test('image: Layer ▸ New Group adds an active empty group; Undo removes it (AC-01)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await menu(page, 'Layer', 'New Group');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
  await expect(layerRow(page, 'Group 1')).toHaveClass(/layer-active/);
  await expect(layerRow(page, 'Group 1')).toContainText('group');
  await undoButton(page).click();
  await expect.poll(() => layerNames(page)).toEqual([name]);
});

test('image: drag a layer into a group and back out, one undo step each (AC-02)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);

  await dropOnRow(page, name, 'Group 1', 0.5);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);

  await dropOnRow(page, name, 'Group 1', 0.1);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '0');
  await expect.poll(() => layerNames(page)).toEqual([name, 'Group 1']);

  await undoButton(page).click();
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
  await undoButton(page).click();
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '0');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
});

test('image: Alt+→ moves a layer into the group above, Alt+← out again (AC-03)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
  // Put the layer above the group, so the group is the row above it.
  await dropOnRow(page, name, 'Group 1', 0.1);
  await expect.poll(() => layerNames(page)).toEqual([name, 'Group 1']);
  // Add a second group below the layer: Alt+→ takes the nearest group
  // listed above at the same depth, so with none above it does nothing.
  const grip = page.getByRole('button', { name: `Drag to reorder ${name}`, exact: true });
  await grip.focus();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '0');

  await page.getByRole('button', { name: 'Add group' }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 2', name, 'Group 1']);
  await grip.focus();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
  await expect.poll(() => layerNames(page)).toEqual(['Group 2', name, 'Group 1']);

  await grip.focus();
  await page.keyboard.press('Alt+ArrowLeft');
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '0');
  await expect.poll(() => layerNames(page)).toEqual([name, 'Group 2', 'Group 1']);

  await undoButton(page).click();
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
});

test('image: hiding a group hides its children from the composite (AC-07, AC-16)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  await dropOnRow(page, name, 'Group 1', 0.5);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
  expect(pixelOf(await composite(page), 8, 8)).toEqual(RED);

  await page.getByRole('button', { name: 'Hide Group 1', exact: true }).click();
  expect(pixelOf(await composite(page), 8, 8)).toEqual(CLEAR);
  // The child's own flag is unchanged.
  await expect(page.getByRole('button', { name: `Hide ${name}`, exact: true })).toBeVisible();

  // One undo step shows the group again.
  await undoButton(page).click();
  await expect(page.getByRole('button', { name: 'Hide Group 1', exact: true })).toBeVisible();
  expect(pixelOf(await composite(page), 8, 8)).toEqual(RED);

  // Group opacity acts on the subtree.
  await selectRow(page, 'Group 1');
  await page.getByLabel('Layer opacity').fill('50');
  const half = pixelOf(await composite(page), 8, 8);
  expect(half[3]).toBeGreaterThan(120);
  expect(half[3]).toBeLessThan(135);
});

test('image: collapsing a group hides its rows only, without an undo step (AC-17)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  await dropOnRow(page, name, 'Group 1', 0.5);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');

  await page.getByRole('button', { name: 'Collapse Group 1', exact: true }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1']);
  expect(pixelOf(await composite(page), 8, 8)).toEqual(RED);

  // Undo reverts the drag, not the collapse.
  await undoButton(page).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '0');
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1']);

  await page.getByRole('button', { name: 'Expand Group 1', exact: true }).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
});

test('image: a locked group refuses paint on its children and a group refuses paint (AC-11, AC-12)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  await dropOnRow(page, name, 'Group 1', 0.5);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');

  // Painting with the group itself active is refused.
  await selectRow(page, 'Group 1');
  await page.getByLabel('Foreground color', { exact: true }).fill('#0000ff');
  await page.keyboard.press('g');
  await imageCanvas(page).click({ position: { x: 4, y: 4 } });
  await expect(page.getByText('“Group 1” is locked')).toBeVisible();
  expect(pixelOf(await composite(page), 8, 8)).toEqual(RED);

  // Locked, the group locks its child without setting the child's flag.
  await page.getByRole('button', { name: 'Lock Group 1', exact: true }).click();
  const childLock = page.getByRole('button', { name: `Lock ${name}`, exact: true });
  await expect(childLock).toHaveAttribute('title', 'Locked by its group');
  await selectRow(page, name);
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.8, 0.6]);
  await expect(page.getByText(`“${name}” is locked`)).toBeVisible();
  await imageCanvas(page).click({ position: { x: 4, y: 4 } });
  expect(pixelOf(await composite(page), 8, 8)).toEqual(RED);
});

test('image: Layer ▸ Merge Group keeps the composite; Undo restores the group (AC-14)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add group' }).click();
  await dropOnRow(page, name, 'Group 1', 0.5);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
  await page.getByLabel('Layer opacity').fill('50');
  await selectRow(page, 'Group 1');
  const before = await composite(page);

  await menu(page, 'Layer', 'Merge Group');
  await expect.poll(() => layerNames(page)).toEqual(['Group 1']);
  await expect(layerRow(page, 'Group 1')).not.toContainText('group');
  expect((await composite(page)).data).toEqual(before.data);

  await undoButton(page).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name]);
  await expect(layerRow(page, name)).toHaveAttribute('data-depth', '1');
});

test('image: deleting a group deletes its subtree in one undo step (AC-05, AC-20)', async ({
  page,
}) => {
  const name = await redDocument(page);
  await page.getByRole('button', { name: 'Add layer' }).click();
  await expect.poll(() => layerNames(page)).toHaveLength(2);
  const [top] = (await layerNames(page)) as [string];
  await page.getByRole('button', { name: 'Add group' }).click();
  await dropOnRow(page, name, 'Group 1', 0.5);
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name, top]);

  await selectRow(page, 'Group 1');
  await page.getByRole('button', { name: 'Delete layer' }).click();
  await expect.poll(() => layerNames(page)).toEqual([top]);
  // The last raster layer cannot go.
  await expect(page.getByRole('button', { name: 'Delete layer' })).toBeDisabled();

  await undoButton(page).click();
  await expect.poll(() => layerNames(page)).toEqual(['Group 1', name, top]);
  expect(pixelOf(await composite(page), 8, 8)).toEqual(RED);
});
