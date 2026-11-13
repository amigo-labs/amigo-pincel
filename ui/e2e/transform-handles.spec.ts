import { expect, test } from '@playwright/test';
import { fakeFileHandles, fakeFs, fakeFsLog, imageCanvas, menu } from './helpers';
import { openHistory, rowNames } from './history-helpers';
import { tabs } from './tab-helpers';
import {
  applied,
  box,
  centre,
  dragBy,
  handle,
  pivot,
  readout,
  scene,
  shot,
  undo,
} from './transform-helpers';

// The Move tools' transform handles in the editor
// (docs/specs/image-transform-handles.md). The core side (results depend
// only on the final box, cancel, undo) is tested in
// crates/pincel-image-core/tests/live_transform.rs; the box geometry in
// transform-box.spec.ts.

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

test('a drag is previewed without the core and applied once on release (AC-23)', async ({
  page,
}) => {
  await scene(page);
  await openHistory(page);
  const rows = await rowNames(page);
  const before = await shot(page);
  const sent = await applied(page);
  const at = await centre(pivot(page));
  const from: [number, number] = [at[0] + 15, at[1] + 15];
  // A drag with a detour that ends 30 px to the right.
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down();
  for (const [dx, dy] of [
    [10, 20],
    [40, -15],
    [-20, 30],
    [25, 5],
    [30, 0],
  ] as const) {
    await page.mouse.move(from[0] + dx, from[1] + dy);
  }
  // Mid-drag the canvas shows the move, but the document is untouched.
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(false);
  expect(await applied(page)).toBe(sent);
  expect(await rowNames(page)).toEqual(rows);
  await page.mouse.up();
  await expect.poll(() => applied(page)).toBe(sent + 1);
  await expect.poll(async () => (await rowNames(page)).length).toBe(rows.length + 1);
  const detour = await shot(page);
  // The same end reached in one move gives the same result.
  await page.keyboard.press('Escape');
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down();
  await page.mouse.move(from[0] + 30, from[1]);
  await page.mouse.up();
  await expect.poll(() => applied(page)).toBe(sent + 2);
  await expect.poll(async () => (await shot(page)).equals(detour)).toBe(true);
});

test('Undo during a drag drops that drag (Edge cases)', async ({ page }) => {
  await scene(page);
  const before = await shot(page);
  const sent = await applied(page);
  const at = await centre(pivot(page));
  await page.mouse.move(at[0] + 15, at[1] + 15);
  await page.mouse.down();
  for (let i = 1; i <= 4; i++) await page.mouse.move(at[0] + 15 + 8 * i, at[1] + 15);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await page.keyboard.press('Control+z');
  await page.mouse.up();
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  await expect.poll(async () => (await shot(page)).equals(before)).toBe(true);
  expect(await applied(page)).toBe(sent);
});
