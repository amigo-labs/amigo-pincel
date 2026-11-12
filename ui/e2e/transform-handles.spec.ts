import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  fakeFileHandles,
  fakeFs,
  fakeFsLog,
  imageCanvas,
  menu,
  setColor,
} from './helpers';
import { tabs } from './tab-helpers';

// The Move tools' transform handles in the editor
// (docs/specs/image-transform-handles.md). The core side (results depend
// only on the final box, cancel, undo) is tested in
// crates/pincel-image-core/tests/live_transform.rs; the box geometry in
// transform-box.spec.ts.

const box = (page: Page): Locator => page.locator('[data-transform-box]');
const handle = (page: Page, id: string): Locator => page.locator(`[data-transform-handle="${id}"]`);
const pivot = (page: Page): Locator => page.locator('[data-transform-pivot]');
const readout = (page: Page): Locator => page.locator('[data-transform-readout]');
const undo = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });

/** A 64 × 64 image with a black stroke, its middle selected, and Move
 *  Selected Pixels picked. */
async function scene(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await setColor(page, 'fg', '#000000');
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await page.keyboard.press('m');
  await drag(page, imageCanvas(page), [0.3, 0.3], [0.7, 0.7]);
  await page.keyboard.press('v');
  await page.keyboard.press('v');
  await expect(page.getByRole('button', { name: 'Move: Move Selected Pixels' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
}

async function centre(l: Locator): Promise<[number, number]> {
  const b = await l.boundingBox();
  if (!b) throw new Error('not visible');
  return [b.x + b.width / 2, b.y + b.height / 2];
}

/** Drags from `from` by `(dx, dy)` screen px with `button`, in steps. */
async function dragBy(
  page: Page,
  from: [number, number],
  dx: number,
  dy: number,
  button: 'left' | 'right' = 'left',
): Promise<void> {
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down({ button });
  for (let i = 1; i <= 6; i++)
    await page.mouse.move(from[0] + (dx * i) / 6, from[1] + (dy * i) / 6);
  await page.mouse.up({ button });
}

/** The canvas pixels: an element screenshot also captures what floats over
 *  it, so the selection's marching ants and the box are hidden for it. */
const shot = (page: Page): Promise<Buffer> =>
  imageCanvas(page).screenshot({
    style:
      '[data-testid="selection-overlay"], [data-transform-box], [data-transform-handle], [data-transform-pivot] { visibility: hidden !important; }',
  });

test('a selection shows the box with eight handles and a pivot for both Move tools only (AC-01)', async ({
  page,
}) => {
  await scene(page);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  await expect(page.locator('[data-transform-handle]')).toHaveCount(8);
  await expect(pivot(page)).toHaveCount(1);
  await page.keyboard.press('v');
  await expect(box(page)).toHaveCount(1);
  await page.keyboard.press('b');
  await expect(box(page)).toHaveCount(0);
});

test('dragging inside moves the content during the drag; the readout follows (AC-02, AC-07)', async ({
  page,
}) => {
  await scene(page);
  const before = await shot(page);
  const at = await centre(pivot(page));
  // Off the pivot, inside the box.
  const from: [number, number] = [at[0] + 20, at[1] + 20];
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down();
  for (let i = 1; i <= 6; i++) await page.mouse.move(from[0] + 8 * i, from[1]);
  // Mid-drag: the transform is live and the canvas already shows the move.
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(false);
  await expect(readout(page)).toBeVisible();
  await page.mouse.up();
  await expect(readout(page)).toContainText('W ');
});

test('a corner handle scales and a right-drag turns; the readout says so (AC-03, AC-05, AC-07)', async ({
  page,
}) => {
  await scene(page);
  await dragBy(page, await centre(handle(page, 'corner2')), 40, 40);
  const scaled = await readout(page).textContent();
  expect(scaled).not.toBeNull();
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 60, at[1]], 0, 60, 'right');
  await expect(readout(page)).not.toContainText(' 0°');
});

test('moving the pivot changes no pixel (AC-06)', async ({ page }) => {
  await scene(page);
  const before = await shot(page);
  await dragBy(page, await centre(pivot(page)), 15, 10);
  expect((await shot(page)).equals(before)).toBe(true);
});

test('Enter commits as one undo step; Undo restores the pixels (AC-08, AC-09)', async ({
  page,
}) => {
  await scene(page);
  const before = await shot(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 30, 0);
  await dragBy(page, await centre(handle(page, 'corner2')), 20, 20);
  await page.keyboard.press('Enter');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  const after = await shot(page);
  expect(after.equals(before)).toBe(false);
  // One step: a single Undo goes back to before the transform.
  await undo(page).click();
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
});

test('a click outside the box commits; another tool commits (AC-08)', async ({ page }) => {
  await scene(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  const canvas = await imageCanvas(page).boundingBox();
  if (!canvas) throw new Error('no canvas');
  // A click (no movement) far outside the box, on the stage.
  await page.mouse.click(canvas.x + 3, canvas.y + 3);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');

  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await page.keyboard.press('b');
  await page.keyboard.press('v');
  await page.keyboard.press('v');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
});

test('Escape cancels: pixels and the undo state as before (AC-10)', async ({ page }) => {
  await scene(page);
  const before = await shot(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 30, 10);
  await dragBy(page, await centre(handle(page, 'e')), 20, 0);
  await page.keyboard.press('Escape');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
});

test('Undo while live steps back one drag and keeps it live (AC-21)', async ({ page }) => {
  await scene(page);
  const before = await shot(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  const first = await shot(page);
  await dragBy(page, await centre(handle(page, 'corner2')), 25, 25);
  await page.keyboard.press('Control+z');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await expect.poll(async () => (await shot(page)).equals(first)).toBe(true);
  await page.keyboard.press('Control+z');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
});

test('changing the quality while live re-applies the transform (AC-12)', async ({ page }) => {
  await scene(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 60, at[1]], 0, 50, 'right');
  const bilinear = await shot(page);
  await page.getByLabel('Quality').selectOption({ label: 'Nearest' });
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await expect.poll(async () => (await shot(page)).equals(bilinear)).toBe(false);
});

test('a locked layer refuses Move Selected Pixels, not Move Selection (AC-16)', async ({
  page,
}) => {
  await scene(page);
  await page
    .getByRole('button', { name: /^Lock / })
    .first()
    .click();
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  await expect(page.getByText(/is locked/)).toBeVisible();
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  await page.keyboard.press('v');
  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
});

test('saving while live keeps the transform; switching tabs commits it (AC-18)', async ({
  page,
}) => {
  await fakeFileHandles(page, {});
  await scene(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await fakeFs(page, { savePick: 'art.ora' });
  await menu(page, 'File', 'Save');
  await expect.poll(async () => (await fakeFsLog(page)).writes).toContain('art.ora');
  // The file holds the transform as applied; it stays live.
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');

  // Another tab commits it in its own document first.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expect(tabs(page)).toHaveCount(2);
  await tabs(page).first().click();
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
});
