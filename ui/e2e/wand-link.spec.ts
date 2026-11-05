import { expect, test, type Locator, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, createDocument, menu, pixelCanvas } from './helpers';

// The Magic Wand link: after a wand click, committing new wand settings
// recomputes that click from the same point (docs/specs/pixel-selection-move.md
// AC-25, AC-26). The selection is read through Copy: the clipboard holds its
// bounding box, or the whole layer without a selection.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const PX = 16;

/** Screen point of the centre of sprite pixel `(x, y)`. */
async function mid(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + ((x + 0.5) * box.width) / PX, box.y + ((y + 0.5) * box.height) / PX];
}

/** The selection's bounding box size (the whole layer without a selection). */
async function size(page: Page): Promise<[number, number]> {
  // Copy from a settings field would copy its text, not the selection.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await clearClipboard(page);
  await page.keyboard.press('ControlOrMeta+c');
  const img = await copiedImage(page);
  return [img.width, img.height];
}

async function drag(page: Page, from: [number, number], to: [number, number]): Promise<void> {
  await page.mouse.move(...from);
  await page.mouse.down();
  await page.mouse.move(...to, { steps: 8 });
  await page.mouse.up();
}

/**
 * A 16×16 sprite split by a red line at x = 8: a contiguous wand click on the
 * left half selects 8×16, a global one (or tolerance 255) the whole canvas.
 */
async function splitSprite(page: Page): Promise<void> {
  await createDocument(page, 'pixel', { width: PX, height: PX });
  await page.getByLabel('Foreground color', { exact: true }).fill('#ff0000');
  await page.keyboard.press('b');
  await drag(page, await mid(page, 8, 0), await mid(page, 8, 15));
  await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
  await page.keyboard.press('w');
}

const tolerance = (page: Page): Locator => page.getByLabel('Wand tolerance', { exact: true });
const contiguous = (page: Page): Locator => page.getByRole('checkbox', { name: 'Contiguous' });
const sample = (page: Page): Locator => page.getByLabel('Wand sample source', { exact: true });

/** Whether anything is selected: Select ▸ Deselect is enabled. */
async function hasSelection(page: Page): Promise<boolean> {
  await menu(page, 'Select');
  const enabled = await page.getByRole('menuitem', { name: 'Deselect', exact: true }).isEnabled();
  await page.keyboard.press('Escape');
  return enabled;
}

async function wandAt(page: Page, x: number, y: number, modifier?: 'Shift'): Promise<void> {
  if (modifier) await page.keyboard.down(modifier);
  await page.mouse.click(...(await mid(page, x, y)));
  if (modifier) await page.keyboard.up(modifier);
}

test.describe('pixel', () => {
  test('committing a new tolerance, Contiguous or sample recomputes the linked click, without an undo entry (AC-25)', async ({
    page,
  }) => {
    await splitSprite(page);
    await wandAt(page, 2, 2);
    expect(await size(page)).toEqual([8, 16]);

    // Enter commits the tolerance: 255 reaches across the line.
    await tolerance(page).fill('255');
    await tolerance(page).press('Enter');
    expect(await size(page)).toEqual([16, 16]);
    // Leaving the field commits too; values clamp as for a click.
    await tolerance(page).fill('-40');
    await tolerance(page).press('Tab');
    expect(await size(page)).toEqual([8, 16]);
    // An arrow key steps and commits: 254 still stops at the line, 255 not.
    await tolerance(page).fill('254');
    await tolerance(page).press('Enter');
    expect(await size(page)).toEqual([8, 16]);
    await tolerance(page).press('ArrowUp');
    await expect(tolerance(page)).toHaveValue('255');
    expect(await size(page)).toEqual([16, 16]);
    // An empty entry is ignored.
    await tolerance(page).fill('');
    await tolerance(page).press('Enter');
    expect(await size(page)).toEqual([16, 16]);
    await tolerance(page).fill('0');
    await tolerance(page).press('Enter');
    expect(await size(page)).toEqual([8, 16]);

    // Contiguous off: every transparent pixel, on both sides of the line.
    await contiguous(page).uncheck();
    expect(await size(page)).toEqual([16, 16]);
    await contiguous(page).check();
    expect(await size(page)).toEqual([8, 16]);
    // The composite samples the same pixels here; the link follows it.
    await sample(page).selectOption('composite');
    expect(await size(page)).toEqual([8, 16]);

    // Selection changes add no undo entry (AC-13).
    await expect(page.getByText('undo 1 / redo 0')).toBeVisible();
  });

  test('the linked click keeps its combine mode against the selection from before it (AC-25)', async ({
    page,
  }) => {
    await splitSprite(page);
    // A 2×2 marquee at x 12–13, then Shift (add) wand on the left half.
    await page.keyboard.press('m');
    await drag(page, await mid(page, 12, 0), await mid(page, 13, 1));
    await page.keyboard.press('w');
    await wandAt(page, 2, 2, 'Shift');
    expect(await size(page)).toEqual([14, 16]);

    await contiguous(page).uncheck();
    expect(await size(page)).toEqual([16, 16]);
    // Back on: recomputed against the marquee alone, not the global result.
    await contiguous(page).check();
    expect(await size(page)).toEqual([14, 16]);
  });

  test('another tool, a selection change, Undo or a click that changes nothing ends the link (AC-26)', async ({
    page,
  }) => {
    await splitSprite(page);

    // Another tool, even with the wand chosen again.
    await wandAt(page, 2, 2);
    await page.keyboard.press('m');
    await page.keyboard.press('w');
    await contiguous(page).uncheck();
    expect(await size(page)).toEqual([8, 16]);
    await contiguous(page).check();

    // Another selection change (Escape deselects).
    await page.keyboard.press('Escape');
    await wandAt(page, 2, 2);
    expect(await size(page)).toEqual([8, 16]);
    await page.keyboard.press('Escape');
    await contiguous(page).uncheck();
    expect(await hasSelection(page)).toBe(false);
    await contiguous(page).check();

    // Without a link the settings apply to the next click.
    await contiguous(page).uncheck();
    await wandAt(page, 2, 2);
    expect(await size(page)).toEqual([16, 16]);
    await contiguous(page).check();
    expect(await size(page)).toEqual([8, 16]);

    // A click that leaves the selection as it is starts no link.
    await wandAt(page, 3, 3);
    await contiguous(page).uncheck();
    expect(await size(page)).toEqual([8, 16]);
    await contiguous(page).check();

    // Undo ends the link.
    await page.keyboard.press('Escape');
    await wandAt(page, 2, 2);
    await page.keyboard.press('ControlOrMeta+z');
    await expect(page.getByText('undo 0 / redo 1')).toBeVisible();
    await contiguous(page).uncheck();
    expect(await size(page)).toEqual([8, 16]);
  });
});
