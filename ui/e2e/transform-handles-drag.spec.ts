import { expect, test } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu } from './helpers';
import { expectCurrent, openHistory, rowNames } from './history-helpers';
import {
  applied,
  box,
  boxCorners,
  centre,
  dragBy,
  outlineShot,
  pivot,
  readout,
  readoutAngle,
  scene,
  shot,
} from './transform-helpers';

// The Move tools' transform box during and after a drag
// (docs/specs/image-transform-handles.md): the box drawn turned, the
// outline following a drag before the release, a rotation started outside
// the box, and the pivot's undo step. The other editor tests are in
// transform-handles.spec.ts.

/** `a` in (-180, 180]. */
const normalize = (a: number): number => {
  const r = a % 360;
  return r <= -180 ? r + 360 : r > 180 ? r - 360 : r;
};

test('after a rotation the box is drawn turned by the angle (AC-01)', async ({ page }) => {
  await scene(page);
  const start = await boxCorners(page);
  expect(start).toHaveLength(4);
  const [a, b] = start;
  if (!a || !b) throw new Error('no corners');
  // Upright before the transform: the top edge is horizontal.
  expect(Math.abs(b[1] - a[1])).toBeLessThan(0.01);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 60, at[1]], 0, 60, 'right');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  const angle = await readoutAngle(page);
  expect(Math.abs(angle)).toBeGreaterThan(10);
  const turned = await boxCorners(page);
  expect(turned).toHaveLength(4);
  const [p, q] = turned;
  if (!p || !q) throw new Error('no corners');
  const edge = (Math.atan2(q[1] - p[1], q[0] - p[0]) * 180) / Math.PI;
  // The readout rounds to 0.1°.
  expect(Math.abs(normalize(edge - angle))).toBeLessThan(0.2);
});

test('no box without a selection or with one that selects no pixel (AC-01, AC-17)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await page.keyboard.press('v');
  await page.keyboard.press('v');
  const pixels = page.getByRole('button', { name: 'Move: Move Selected Pixels' });
  await expect(pixels).toHaveAttribute('aria-pressed', 'true');
  await expect(box(page)).toHaveCount(0);

  // A small selection that Contract empties.
  await page.keyboard.press('m');
  await drag(page, imageCanvas(page), [0.45, 0.45], [0.55, 0.55]);
  await menu(page, 'Select', 'Contract…');
  const dialog = page.getByRole('dialog', { name: 'Contract…' });
  await dialog.getByLabel('Radius (px)').fill('20');
  await dialog.getByRole('button', { name: 'Apply' }).click();
  await expect(dialog).toBeHidden();
  // From another tool, V picks Move Layer, a second V Move Selected Pixels.
  await page.keyboard.press('v');
  await page.keyboard.press('v');
  await expect(pixels).toHaveAttribute('aria-pressed', 'true');
  await expect(box(page)).toHaveCount(0);
  // A drag starts no transform.
  await drag(page, imageCanvas(page), [0.5, 0.5], [0.6, 0.6]);
  await expect(box(page)).toHaveCount(0);
  await expect(readout(page)).toHaveCount(0);
  await page.keyboard.press('v');
  await expect(page.getByRole('button', { name: 'Move: Move Selection' })).toHaveAttribute('aria-pressed', 'true');
  await expect(box(page)).toHaveCount(0);
});

for (const [name, pixels] of [
  ['Move Selected Pixels', true],
  ['Move Selection', false],
] as const) {
  test(`the outline follows a ${name} drag before the release (AC-02, AC-23)`, async ({ page }) => {
    // Still marching ants, so two shots of one outline are equal.
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await scene(page);
    if (!pixels) {
      await page.keyboard.press('v');
      await expect(page.getByRole('button', { name: 'Move: Move Selection' })).toHaveAttribute(
        'aria-pressed',
        'true',
      );
    }
    await openHistory(page);
    const rows = await rowNames(page);
    const sent = await applied(page);
    const outline = await outlineShot(page);
    const canvas = await shot(page);
    const at = await centre(pivot(page));
    const from: [number, number] = [at[0] + 15, at[1] + 15];
    await page.mouse.move(from[0], from[1]);
    await page.mouse.down();
    for (let i = 1; i <= 4; i++) await page.mouse.move(from[0] + 6 * i, from[1] + 3 * i);
    await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
    // Mid-drag the outline has left its place ...
    await expect.poll(async () => (await outlineShot(page)).equals(outline)).toBe(false);
    const first = await outlineShot(page);
    // ... and keeps following the pointer.
    for (let i = 5; i <= 8; i++) await page.mouse.move(from[0] + 6 * i, from[1] + 3 * i);
    await expect.poll(async () => (await outlineShot(page)).equals(first)).toBe(false);
    // The document has not changed yet.
    expect(await applied(page)).toBe(sent);
    expect(await rowNames(page)).toEqual(rows);
    // Move Selection moves no pixel, not even in the preview.
    if (!pixels) expect((await shot(page)).equals(canvas)).toBe(true);
    await page.mouse.up();
    await expect.poll(() => applied(page)).toBe(sent + 1);
    await expect.poll(async () => (await rowNames(page)).length).toBe(rows.length + 1);
  });
}

test('a left-drag from outside the box turns it once it moves past 4 px (AC-05)', async ({
  page,
}) => {
  await scene(page);
  await openHistory(page);
  const rows = await rowNames(page);
  const sent = await applied(page);
  const at = await centre(pivot(page));
  const c = await imageCanvas(page).boundingBox();
  if (!c) throw new Error('no canvas');
  // Level with the pivot, left of the box (it spans 30 – 70 % of the canvas).
  const r = at[0] - (c.x + c.width * 0.08);
  await page.mouse.move(at[0] - r, at[1]);
  await page.mouse.down();
  // Within the 4 px click slop nothing turns yet.
  await page.mouse.move(at[0] - r, at[1] - 2);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  // A 45° sweep clockwise around the pivot.
  for (let i = 1; i <= 6; i++) {
    const a = Math.PI + (Math.PI / 4) * (i / 6);
    await page.mouse.move(at[0] + r * Math.cos(a), at[1] + r * Math.sin(a));
  }
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await page.mouse.up();
  await expect.poll(() => applied(page)).toBe(sent + 1);
  await expect.poll(() => rowNames(page)).toEqual([...rows, 'Move Selected Pixels']);
  expect(Math.abs((await readoutAngle(page)) - 45)).toBeLessThan(1);
});

test('a pivot drag is one undo step and Undo puts the pivot back (AC-06)', async ({ page }) => {
  await scene(page);
  await openHistory(page);
  const sent = await applied(page);
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 15, at[1] + 15], 20, 0);
  await expect.poll(() => applied(page)).toBe(sent + 1);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  const rows = await rowNames(page);
  const moved = await centre(pivot(page));
  await dragBy(page, moved, 15, 10);
  await expect.poll(() => rowNames(page)).toEqual([...rows, 'Move Selected Pixels']);
  const dragged = await centre(pivot(page));
  expect(Math.hypot(dragged[0] - moved[0] - 15, dragged[1] - moved[1] - 10)).toBeLessThan(1);
  await page.keyboard.press('Control+z');
  await expectCurrent(page, rows.length - 1);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  await expect
    .poll(async () => {
      const p = await centre(pivot(page));
      return Math.hypot(p[0] - moved[0], p[1] - moved[1]);
    })
    .toBeLessThan(1);
});
