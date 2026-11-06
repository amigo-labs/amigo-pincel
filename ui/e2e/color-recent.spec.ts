import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  pixelCanvas,
  setColor,
  storedPref,
} from './helpers';

// Recent colors (color-picker AC-12 – AC-15, AC-24, AC-30): a color joins
// its mode's recent row when a tool paints with it, most recent first,
// without duplicates; the row and the last colors survive a reload.

type Mode = 'pixel' | 'image';

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** The recent row's colors as their accessible names (hex), in order. */
async function recent(page: Page): Promise<string[]> {
  await page.getByRole('button', { name: 'Foreground color', exact: true }).click();
  const picker = page.getByRole('dialog', { name: 'Foreground color picker' });
  const names = await picker
    .getByRole('group', { name: 'Recent colors' })
    .getByRole('button')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
  await page.keyboard.press('Escape');
  await expect(picker).toHaveCount(0);
  return names;
}

/** R / G / B / A of a swatch, read from its picker. */
async function rgbaOf(page: Page, which: 'fg' | 'bg'): Promise<number[]> {
  const name = which === 'fg' ? 'Foreground color' : 'Background color';
  await page.getByRole('button', { name, exact: true }).click();
  const picker = page.getByRole('dialog', { name: `${name} picker` });
  const out: number[] = [];
  for (const label of ['Red', 'Green', 'Blue', 'Alpha value']) {
    out.push(
      Number(await picker.getByRole('spinbutton', { name: label, exact: true }).inputValue()),
    );
  }
  await page.keyboard.press('Escape');
  return out;
}

async function stroke(page: Page, canvas: Locator, y: number, button: 'left' | 'right' = 'left') {
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * y);
  await page.mouse.down({ button });
  await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * y, { steps: 4 });
  await page.mouse.up({ button });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: painting adds its color, most recent first, once (AC-12, AC-24)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    const canvas = canvasOf(page, mode);
    expect(await recent(page)).toEqual([]);

    await setColor(page, 'fg', '#11223380');
    await page.keyboard.press('b');
    await stroke(page, canvas, 0.2);
    expect(await recent(page)).toEqual(['#11223380']);
    // The same color again: no change.
    await stroke(page, canvas, 0.3);
    expect(await recent(page)).toEqual(['#11223380']);

    // The right button paints the background color.
    await setColor(page, 'bg', '#00ff00ff');
    await stroke(page, canvas, 0.4, 'right');
    expect(await recent(page)).toEqual(['#00ff00', '#11223380']);

    // The Eraser and the Eyedropper add nothing.
    await setColor(page, 'fg', '#0000ffff');
    await page.keyboard.press('e');
    await stroke(page, canvas, 0.2);
    await page.keyboard.press('i');
    await stroke(page, canvas, 0.4);
    expect(await recent(page)).toEqual(['#00ff00', '#11223380']);

    // A stroke beside the canvas changes no pixel and adds nothing.
    await setColor(page, 'fg', '#ff00ffff');
    await page.keyboard.press('b');
    await drag(page, canvas, [1.05, 0.5], [1.2, 0.5]);
    expect(await recent(page)).toEqual(['#00ff00', '#11223380']);

    // Paint Bucket with the foreground moves an old color to the front.
    await setColor(page, 'fg', '#11223380');
    await page.keyboard.press('g');
    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas is not visible');
    await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.9);
    expect(await recent(page)).toEqual(['#11223380', '#00ff00']);
  });

  test(`${mode}: a filled and outlined shape adds the foreground, then the background (AC-24)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await setColor(page, 'fg', '#ff0000ff');
    await setColor(page, 'bg', '#0000ffff');
    await page.keyboard.press('u');
    await page
      .getByRole('radiogroup', { name: 'Shapes variant' })
      .getByRole('radio', { name: 'Rectangle', exact: true })
      .click();
    if (mode === 'pixel') await page.getByLabel('Shape mode').selectOption('fill_outline');
    else {
      await page
        .getByRole('combobox', { name: 'Mode', exact: true })
        .selectOption('fill_and_outline');
    }
    await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.8]);
    expect(await recent(page)).toEqual(['#ff0000', '#0000ff']);
  });
}

test('image: a live line adds the foreground, a live gradient both its colors (AC-24)', async ({
  page,
}) => {
  // Line and Gradient drags create live objects (image-live-objects-ui); the
  // drag that creates one counts as painting (color-picker Decisions 2026-11-06).
  await createDocument(page, 'image', { width: 32, height: 32 });
  await setColor(page, 'fg', '#ff0000ff');
  await setColor(page, 'bg', '#0000ffff');
  await page.keyboard.press('l');
  await drag(page, imageCanvas(page), [0.2, 0.3], [0.8, 0.3]);
  expect(await recent(page)).toEqual(['#ff0000']);

  // Commit first, or the new foreground would edit the live line.
  await page.keyboard.press('Enter');
  await setColor(page, 'fg', '#00ff00ff');
  await page.keyboard.press('n');
  await drag(page, imageCanvas(page), [0.2, 0.6], [0.8, 0.6]);
  expect(await recent(page)).toEqual(['#00ff00', '#0000ff', '#ff0000']);
});

test('a recent color sets the edited color, alpha included (AC-13)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#11223380');
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.5);
  await setColor(page, 'bg', '#ffffffff');
  await page.getByRole('button', { name: 'Background color', exact: true }).click();
  const picker = page.getByRole('dialog', { name: 'Background color picker' });
  await picker
    .getByRole('group', { name: 'Recent colors' })
    .getByRole('button', { name: '#11223380' })
    .click();
  await page.keyboard.press('Escape');
  expect(await rgbaOf(page, 'bg')).toEqual([17, 34, 51, 128]);
});

test('each mode keeps its own recent colors (AC-14)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#123456ff');
  await page.keyboard.press('b');
  await stroke(page, pixelCanvas(page), 0.5);
  expect(await recent(page)).toEqual(['#123456']);
  await createDocument(page, 'image', { width: 16, height: 16 });
  expect(await recent(page)).toEqual([]);
});

test('recent colors and the last colors survive a reload, per mode (AC-15)', async ({ page }) => {
  for (const [mode, colors] of [
    ['pixel', ['#11223380', '#445566ff']],
    ['image', ['#aabbcc40', '#010203ff']],
  ] as const) {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('b');
    for (const [i, c] of colors.entries()) {
      await setColor(page, 'fg', c);
      await stroke(page, canvasOf(page, mode), 0.3 + i * 0.3);
    }
    await setColor(page, 'bg', mode === 'pixel' ? '#99887766' : '#55443322');
    await expect
      .poll(async () => ((await storedPref(page, `recentColors:${mode}`)) as unknown[])?.length)
      .toBe(2);
  }
  await page.waitForTimeout(300);

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await recent(page)).toEqual(['#445566', '#11223380']);
  expect(await rgbaOf(page, 'fg')).toEqual([0x44, 0x55, 0x66, 255]);
  expect(await rgbaOf(page, 'bg')).toEqual([0x99, 0x88, 0x77, 0x66]);
  await createDocument(page, 'image', { width: 16, height: 16 });
  expect(await recent(page)).toEqual(['#010203', '#aabbcc40']);
  expect(await rgbaOf(page, 'fg')).toEqual([1, 2, 3, 255]);
  expect(await rgbaOf(page, 'bg')).toEqual([0x55, 0x44, 0x33, 0x22]);
});

test('unreadable stored colors fall back silently (AC-30)', async ({ page }) => {
  await gotoStart(page);
  await page.evaluate(
    (entries) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          for (const [key, value] of entries) tx.objectStore('prefs').put({ key, value });
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(new Error('pref write failed'));
        };
        open.onerror = () => reject(new Error('IndexedDB unavailable'));
      }),
    [
      ['recentColors:image', ['#zzz', 7, '#11223380']],
      ['recentColors:pixel', 'not a list'],
      ['lastColor:image', 42],
      ['lastBgColor:image', '#12'],
    ] as const,
  );
  await createDocument(page, 'image', { width: 16, height: 16 });
  expect(await recent(page)).toEqual(['#11223380']);
  expect(await rgbaOf(page, 'fg')).toEqual([0, 0, 0, 255]);
  expect(await rgbaOf(page, 'bg')).toEqual([255, 255, 255, 255]);
  await expect(page.getByRole('alert')).toHaveCount(0);
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await recent(page)).toEqual([]);
});
