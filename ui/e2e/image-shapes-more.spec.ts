import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, setColor } from './helpers';

// The six new box shapes in the editor (docs/specs/image-shapes-curves-shapes.md).

const NAMES = [
  'Rectangle',
  'Rounded Rectangle',
  'Ellipse',
  'Polygon',
  'Triangle',
  'Diamond',
  'Star',
  'Block Arrow',
  'Heart',
  'Callout',
];
const NAMES_DE = [
  'Rechteck',
  'Abgerundetes Rechteck',
  'Ellipse',
  'Polygon',
  'Dreieck',
  'Raute',
  'Stern',
  'Blockpfeil',
  'Herz',
  'Sprechblase',
];

const activeTool = (page: Page): Locator =>
  page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
const handles = (page: Page): Locator => page.locator('[data-live-handle]');
const undoButton = (page: Page): Locator => page.getByRole('button', { name: 'Undo', exact: true });
const redoButton = (page: Page): Locator => page.getByRole('button', { name: 'Redo', exact: true });

/** Picks the Shapes variant `name` in the options bar. */
async function variant(page: Page, name: string): Promise<void> {
  await page.keyboard.press('u');
  await page
    .getByRole('radiogroup', { name: 'Shapes variant' })
    .getByRole('radio', { name, exact: true })
    .click();
  await expect(activeTool(page)).toHaveAttribute('aria-label', `Shapes: ${name}`);
}

/** Page coordinates of a fractional canvas position. */
async function at(page: Page, [fx, fy]: [number, number]): Promise<[number, number]> {
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + box.width * fx, box.y + box.height * fy];
}

/** The color shown at a fractional canvas position (the overlay included). */
async function shown(page: Page, [fx, fy]: [number, number]): Promise<string> {
  const png = await imageCanvas(page).screenshot();
  return page.evaluate(
    async ({ b64, fx, fy }) => {
      const bitmap = await createImageBitmap(
        await (await fetch(`data:image/png;base64,${b64}`)).blob(),
      );
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

/** A white image with red fill and the Fill mode set. */
async function filled(page: Page, size = 200): Promise<void> {
  await createDocument(page, 'image', { width: size, height: size });
  await setColor(page, 'fg', '#000000');
  await setColor(page, 'bg', '#ff0000');
  await page.keyboard.press('u');
  await page.getByRole('combobox', { name: 'Mode', exact: true }).selectOption('fill');
}

const RED = '#ff0000';

test('U cycles the ten shapes; the flyout and the options bar list them in order with icons and names (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 64, height: 64 });
  await page.keyboard.press('u');
  const seen: string[] = [];
  for (let i = 0; i < 10; i += 1) {
    seen.push(((await activeTool(page).getAttribute('aria-label')) ?? '').replace('Shapes: ', ''));
    await page.keyboard.press('u');
  }
  expect(seen).toEqual(NAMES);
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Rectangle');

  const radios = page.getByRole('radiogroup', { name: 'Shapes variant' }).getByRole('radio');
  const label = (els: Element[]) => els.map((e) => e.getAttribute('aria-label'));
  expect(await radios.evaluateAll(label)).toEqual(NAMES);
  // Each variant has an icon of its own.
  const icons = await radios.evaluateAll((els) =>
    els.map((e) => e.querySelector('svg')?.innerHTML ?? ''),
  );
  expect(icons.every((s) => s.length > 0)).toBe(true);
  expect(new Set(icons).size).toBe(10);

  await page
    .getByRole('toolbar', { name: 'Tools' })
    .locator('[data-group="shapes"]')
    .click({ button: 'right' });
  const menu = page.getByRole('menu', { name: 'Shapes', exact: true });
  await expect(menu.getByRole('menuitemradio')).toHaveCount(10);
  expect(
    await menu
      .getByRole('menuitemradio')
      .evaluateAll((els) =>
        els.map((e) => (e.textContent ?? '').replace(/^\s*\u2713?\s*/, '').trim()),
      ),
  ).toEqual(NAMES);
  await page.keyboard.press('Escape');

  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  const de = page.getByRole('radiogroup', { name: 'Formen-Variante' }).getByRole('radio');
  expect(await de.evaluateAll(label)).toEqual(NAMES_DE);
});

test('the drag previews the shape’s own outline and leaves it live; the commit is one undo step (AC-12)', async ({
  page,
}) => {
  await filled(page);
  // A rectangle's preview runs through the box corner, a triangle's not.
  const corner: [number, number] = [0.3, 0.2];
  const before = await shown(page, corner);
  const previewAt = async (name: string): Promise<string> => {
    await variant(page, name);
    const [ax, ay] = await at(page, [0.2, 0.2]);
    const [bx, by] = await at(page, [0.8, 0.8]);
    await page.mouse.move(ax, ay);
    await page.mouse.down();
    await page.mouse.move(bx, by, { steps: 6 });
    const color = await shown(page, corner);
    await page.mouse.up();
    return color;
  };
  expect(await previewAt('Rectangle')).not.toBe(before);
  await undoButton(page).click();
  await expect(handles(page)).toHaveCount(0);
  expect(await previewAt('Triangle')).toBe(before);
  // Released: the triangle is live with its box handles.
  await expect(handles(page)).toHaveCount(8);
  expect(await shown(page, [0.5, 0.6])).toBe(RED);
  expect(await shown(page, [0.25, 0.3])).not.toBe(RED);
  await page.keyboard.press('Enter');
  await expect(handles(page)).toHaveCount(0);
  await undoButton(page).click();
  expect(await shown(page, [0.5, 0.6])).not.toBe(RED);
  await expect(undoButton(page)).toBeDisabled();
  await redoButton(page).click();
  expect(await shown(page, [0.5, 0.6])).toBe(RED);
});

test('Shift squares the box of a new shape (AC-11)', async ({ page }) => {
  await filled(page);
  await variant(page, 'Diamond');
  await page.keyboard.down('Shift');
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.8, 0.5]);
  await page.keyboard.up('Shift');
  // Squared to 0.6 × 0.6: the diamond reaches down to 0.8.
  expect(await shown(page, [0.5, 0.7])).toBe(RED);
});

test('another box shape converts the live one as one step and keeps it live (AC-19)', async ({
  page,
}) => {
  await filled(page);
  await variant(page, 'Triangle');
  await drag(page, imageCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await expect(handles(page)).toHaveCount(8);
  // Triangle → Diamond with U: the left edge midpoint fills in.
  expect(await shown(page, [0.25, 0.5])).not.toBe(RED);
  await page.keyboard.press('u');
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Diamond');
  await expect.poll(() => shown(page, [0.25, 0.5])).toBe(RED);
  await expect(handles(page)).toHaveCount(8);
  // One step: Undo is the triangle again, still live; Redo the diamond.
  await undoButton(page).click();
  await expect.poll(() => shown(page, [0.25, 0.5])).not.toBe(RED);
  await expect(handles(page)).toHaveCount(8);
  await redoButton(page).click();
  await expect.poll(() => shown(page, [0.25, 0.5])).toBe(RED);
  // Diamond → Rectangle and → Heart in the options bar, and the flyout.
  const radio = (name: string) =>
    page
      .getByRole('radiogroup', { name: 'Shapes variant' })
      .getByRole('radio', { name, exact: true });
  await radio('Rectangle').click();
  await expect.poll(() => shown(page, [0.22, 0.22])).toBe(RED);
  await radio('Heart').click();
  await expect.poll(() => shown(page, [0.22, 0.22])).not.toBe(RED);
  await expect.poll(() => shown(page, [0.5, 0.6])).toBe(RED);
  await page
    .getByRole('toolbar', { name: 'Tools' })
    .locator('[data-group="shapes"]')
    .click({ button: 'right' });
  await page
    .getByRole('menu', { name: 'Shapes', exact: true })
    .getByRole('menuitemradio', { name: 'Callout' })
    .click();
  await expect(activeTool(page)).toHaveAttribute('aria-label', 'Shapes: Callout');
  await expect(handles(page)).toHaveCount(8);
  // Conversions in a row merge into one step (image-live-objects AC-03):
  // one Undo is the diamond again.
  await undoButton(page).click();
  await expect.poll(() => shown(page, [0.25, 0.5])).toBe(RED);
  await expect.poll(() => shown(page, [0.22, 0.22])).not.toBe(RED);
  await expect(handles(page)).toHaveCount(8);
});

test('star options and a Block Arrow handle drag redraw the live shape (AC-20)', async ({
  page,
}) => {
  await filled(page);
  await variant(page, 'Star');
  await drag(page, imageCanvas(page), [0.1, 0.1], [0.9, 0.9]);
  await expect(handles(page)).toHaveCount(8);
  // A fat star (inner radius 90 %) fills a point between two tips.
  const between: [number, number] = [
    0.5 + 0.3 * Math.cos(-Math.PI / 2 + Math.PI / 5),
    0.5 + 0.3 * Math.sin(-Math.PI / 2 + Math.PI / 5),
  ];
  expect(await shown(page, between)).not.toBe(RED);
  await page.getByRole('slider', { name: /^Inner radius/ }).fill('90');
  await expect.poll(() => shown(page, between)).toBe(RED);
  await undoButton(page).click();
  await expect.poll(() => shown(page, between)).not.toBe(RED);
  await expect(handles(page)).toHaveCount(8);
  await page.keyboard.press('Enter');

  // A long arrow, dragged left to right: it points right.
  await variant(page, 'Block Arrow');
  await drag(page, imageCanvas(page), [0.3, 0.45], [0.7, 0.55]);
  await expect(handles(page)).toHaveCount(8);
  // The start corner's edge (left, w) dragged past the right edge to 0.9:
  // the start corner moves with it, the arrow now points left.
  const w = await page.locator('[data-live-handle="w"]').boundingBox();
  if (!w) throw new Error('no w handle');
  const [tx] = await at(page, [0.9, 0.5]);
  await page.mouse.move(w.x + w.width / 2, w.y + w.height / 2);
  await page.mouse.down();
  await page.mouse.move(tx, w.y + w.height / 2, { steps: 8 });
  await page.mouse.up();
  // Left-pointing over 0.7–0.9: its head (0.7–0.8) is tall at x = 0.79;
  // a right-pointing arrow would have its thin shaft there.
  await expect.poll(() => shown(page, [0.79, 0.54])).toBe(RED);
  await expect.poll(() => shown(page, [0.88, 0.54])).not.toBe(RED);
  await expect(handles(page)).toHaveCount(8);
});
