import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, menu } from './helpers';
import {
  chooseSave,
  colorsPanel,
  loadPalette,
  noFilePickers,
  openPaletteMenu,
  savePalette,
  statusBar,
  stripPng,
  swatchNames,
  swatches,
} from './palette-helpers';

// Image mode's Colors panel palette: Load palette…, Save palette, the
// formats and their errors (docs/specs/image-palette-gradient.md AC-01 –
// AC-08, AC-23, AC-24).

const HEX_TXT = '﻿; Lospec\r\n#FF0044\r\n00ff00  \r\n\r\n// blue\r\n0000ff\r\n123456\r\n';
const NAMED_GPL = 'GIMP Palette\nName: Test\n# c\n0 0 0 Black\n255 255 255\n10 20 30 Deep sea\n';
const ALPHA_PAL = 'JASC-PAL\r\n0100\r\n3\r\n255 0 68\r\n0 255 0 128\r\n1 2 3\r\n';

test.describe('with the file chooser and downloads', () => {
  test.beforeEach(async ({ page }) => {
    await noFilePickers(page);
  });

  test('the Colors panel has the palette menu, its Save submenu and Add color (AC-01)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await expect(
      colorsPanel(page).getByRole('button', { name: 'Add color', exact: true }),
    ).toBeVisible();
    const palMenu = await openPaletteMenu(page);
    const items = palMenu.getByRole('menuitem');
    await expect(items).toHaveCount(3);
    for (const [i, name] of [
      'Load palette…',
      'Save palette',
      'Reset to default palette',
    ].entries()) {
      await expect(items.nth(i)).toHaveAccessibleName(name);
    }
    const save = palMenu.getByRole('menuitem', { name: 'Save palette', exact: true });
    await expect(save).toHaveAttribute('aria-haspopup', 'menu');
    await save.click();
    await expect(page.getByRole('menu', { name: 'Save palette' }).getByRole('menuitem')).toHaveText(
      ['GIMP .gpl', 'Lospec .hex', 'JASC .pal', 'PNG strip (1 px)', 'PNG strip (8 px)'],
    );
  });

  test('loading a .txt file with hex lines replaces the palette and says how many (AC-02, AC-03)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await expect(swatches(page)).toHaveCount(48);
    await loadPalette(page, 'lospec.txt', Buffer.from(HEX_TXT));
    await expect(swatches(page)).toHaveCount(4);
    expect(await swatchNames(page)).toEqual(['#ff0044', '#00ff00', '#0000ff', '#123456']);
    await expect(statusBar(page)).toContainText('loaded 4 palette colours');
  });

  test('a file loads the same colors in the same order as Pixel mode imports it (AC-03)', async ({
    page,
  }) => {
    const files: [string, Buffer, number][] = [
      ['named.gpl', Buffer.from(NAMED_GPL), 3],
      ['lospec.txt', Buffer.from(HEX_TXT), 4],
      ['alpha.pal', Buffer.from(ALPHA_PAL), 3],
    ];
    await createDocument(page, 'pixel', { width: 8, height: 8 });
    files.push([
      'strip.png',
      await stripPng(page, [
        [9, 9, 9, 0],
        [0, 0, 255, 255],
        [255, 0, 0, 255],
        [0, 0, 255, 255],
        [0, 255, 0, 128],
      ]),
      3,
    ]);
    // Pixel mode's Palette ▸ Import of each file: RGB from the swatch titles.
    const pixel: string[][] = [];
    const pixelPanel = page.getByRole('region', { name: 'Palette' });
    const pixelSwatches = pixelPanel
      .getByRole('group', { name: 'Palette colors' })
      .getByRole('button');
    for (const [name, buffer, count] of files) {
      await pixelPanel.getByRole('button', { name: 'Palette options' }).click();
      const [chooser] = await Promise.all([
        page.waitForEvent('filechooser'),
        page.getByRole('menuitem', { name: 'Import palette…' }).click(),
      ]);
      await chooser.setFiles({ name, mimeType: 'application/octet-stream', buffer });
      await expect(page.getByText(`imported ${count} palette colours`)).toBeVisible();
      await expect(pixelSwatches).toHaveCount(count);
      pixel.push(
        await pixelSwatches.evaluateAll((els) =>
          els.map((el) => /#[0-9a-f]{6}/.exec(el.getAttribute('title') ?? '')?.[0] ?? ''),
        ),
      );
    }

    await menu(page, 'File', 'New…');
    const dialog = page.getByRole('dialog', { name: 'New document' });
    await dialog.getByRole('radio', { name: /^Image/ }).click();
    await dialog.getByRole('button', { name: 'Create' }).click();
    await expect(swatches(page)).toHaveCount(48);
    const image: string[][] = [];
    for (const [name, buffer, count] of files) {
      await loadPalette(page, name, buffer);
      await expect(statusBar(page)).toContainText(`loaded ${count} palette colours`);
      await expect(swatches(page)).toHaveCount(count);
      image.push(await swatchNames(page));
    }
    const hex = (names: string[]) => names.map((n) => /#[0-9a-f]{6,8}$/.exec(n)?.[0] ?? '');
    expect(image.map(hex).map((l) => l.map((h) => h.slice(0, 7)))).toEqual(pixel);
    // Alpha comes from the file: the .pal's fourth column, the PNG's pixels.
    expect(hex(image[2] ?? [])).toEqual(['#ff0044', '#00ff0080', '#010203']);
    expect(hex(image[3] ?? [])).toEqual(['#0000ff', '#ff0000', '#00ff0080']);
  });

  test('an unreadable file keeps the palette and names the reason (AC-04)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await loadPalette(page, 'two.hex', Buffer.from('ff0044\n00ff00\n'));
    await expect(swatches(page)).toHaveCount(2);
    const strip = await stripPng(page, [
      [1, 2, 3, 255],
      [4, 5, 6, 255],
    ]);
    const photo = await stripPng(
      page,
      Array.from({ length: 300 }, (_, i) => [i & 0xff, i >> 8, 7, 255]),
    );
    const cases: [string, Buffer, string | RegExp][] = [
      ['bad.hex', Buffer.from('ff0000\nnope\n'), 'line 2: not a colour'],
      [
        'bad.gpl',
        Buffer.from('GIMP Palette\n1 2 300\n'),
        'line 2: expected `R G B [name]` with values 0–255',
      ],
      ['short.pal', Buffer.from('JASC-PAL\n'), 'not a JASC palette (missing `0100` or count line)'],
      ['big.hex', Buffer.from('000000\n'.repeat(257)), 'palette has more than 256 colours'],
      ['empty.hex', Buffer.from('; nothing\n'), 'no colours found'],
      ['cut.png', strip.subarray(0, 30), /png decode failed/],
      ['photo.png', photo, 'not a palette image (more than 256 colours)'],
      ['random.bin', Buffer.from([0x00, 0x9c, 0xff, 0x10, 0x0a, 0x41]), 'line 1: not a colour'],
    ];
    for (const [name, buffer, reason] of cases) {
      await loadPalette(page, name, buffer);
      const text =
        typeof reason === 'string'
          ? `Load palette failed: ${reason}`
          : new RegExp(`Load palette failed: ${reason.source}`);
      await expect(statusBar(page), name).toContainText(text);
      expect(await swatchNames(page), name).toEqual(['#ff0044', '#00ff00']);
    }
  });

  test('each Save palette entry writes its format; loading it back gives the colors (AC-06, AC-07)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await loadPalette(page, 'alpha.pal', Buffer.from(ALPHA_PAL));
    await expect(swatches(page)).toHaveCount(3);
    const opaque = ['#ff0044', '#00ff00', '#010203'];
    const withAlpha = ['#ff0044', '#00ff0080', '#010203'];
    const saves = [
      ['GIMP .gpl', 'palette.gpl', opaque],
      ['Lospec .hex', 'palette.hex', opaque],
      ['JASC .pal', 'palette.pal', opaque],
      ['PNG strip (1 px)', 'palette.png', withAlpha],
      ['PNG strip (8 px)', 'palette.png', withAlpha],
    ] as const;
    const saved: Buffer[] = [];
    for (const [entry, file] of saves) {
      const download = await savePalette(page, entry);
      expect(download.suggestedFilename(), entry).toBe(file);
      await expect(statusBar(page)).toContainText('saved 3 palette colours');
      saved.push(await readFile(await download.path()));
      // A load in between, so each save's message is its own.
      await loadPalette(page, 'alpha.pal', Buffer.from(ALPHA_PAL));
      await expect(statusBar(page)).toContainText('loaded 3 palette colours');
    }
    const text = saved.map((b) => b.toString('utf8'));
    expect(text[0]?.startsWith('GIMP Palette\nName: palette\n')).toBe(true);
    expect(text[1]).toBe('ff0044\n00ff00\n010203\n');
    expect(text[2]?.split('\r\n').slice(0, 6)).toEqual([
      'JASC-PAL',
      '0100',
      '3',
      '255 0 68',
      '0 255 0',
      '1 2 3',
    ]);
    for (const [i, cell] of [
      [3, 1],
      [4, 8],
    ] as const) {
      const size = await page.evaluate(
        async (b) => {
          const bmp = await createImageBitmap(new Blob([new Uint8Array(b)], { type: 'image/png' }));
          return [bmp.width, bmp.height];
        },
        Array.from(saved[i] ?? []),
      );
      expect(size).toEqual([3 * cell, cell]);
    }
    for (const [i, [, file, expected]] of saves.entries()) {
      await loadPalette(page, 'two.hex', Buffer.from('ffffff\n000000\n'));
      await expect(swatches(page)).toHaveCount(2);
      await loadPalette(page, file, saved[i] ?? Buffer.alloc(0));
      await expect(swatches(page)).toHaveCount(3);
      expect(await swatchNames(page), file).toEqual([...expected]);
    }
  });

  test('.gpl entry names survive load and save and show on the swatches (AC-08)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await loadPalette(page, 'named.gpl', Buffer.from(NAMED_GPL));
    expect(await swatchNames(page)).toEqual(['Black #000000', '#ffffff', 'Deep sea #0a141e']);
    await expect(swatches(page).nth(2)).toHaveAttribute('title', 'Deep sea #0a141e');
    const gpl = (await readFile(await (await savePalette(page, 'GIMP .gpl')).path())).toString();
    expect(gpl).toContain('  0   0   0\tBlack\n');
    expect(gpl).toContain(' 10  20  30\tDeep sea\n');
    await loadPalette(page, 'again.gpl', Buffer.from(gpl));
    expect(await swatchNames(page)).toEqual(['Black #000000', '#ffffff', 'Deep sea #0a141e']);
  });
});

/** Replaces the save picker: `cancel` throws the user's abort, `fail`
 *  hands back a file whose writing fails. Counts the calls. */
async function savePicker(page: Page, mode: 'cancel' | 'fail'): Promise<void> {
  await page.addInitScript((m) => {
    const w = window as unknown as {
      showSaveFilePicker: () => Promise<unknown>;
      showOpenFilePicker: () => Promise<unknown>;
      pickerCalls: number;
    };
    w.pickerCalls = 0;
    const abort = () => new DOMException('The user aborted a request.', 'AbortError');
    w.showOpenFilePicker = async () => {
      w.pickerCalls += 1;
      throw abort();
    };
    w.showSaveFilePicker = async () => {
      w.pickerCalls += 1;
      if (m === 'cancel') throw abort();
      return {
        kind: 'file',
        name: 'palette.gpl',
        createWritable: async () => {
          throw new Error('disk full');
        },
      };
    };
  }, mode);
}

const pickerCalls = (page: Page) =>
  page.evaluate(() => (window as unknown as { pickerCalls: number }).pickerCalls);

test('dismissing the file picker of Load palette… changes nothing and says nothing (AC-05)', async ({
  page,
}) => {
  await savePicker(page, 'cancel');
  await createDocument(page, 'image', { width: 16, height: 16 });
  const before = await statusBar(page).textContent();
  await openPaletteMenu(page);
  await page.getByRole('menuitem', { name: 'Load palette…' }).click();
  await expect.poll(() => pickerCalls(page)).toBe(1);
  await expect(page.getByRole('menu', { name: 'Palette options' })).toHaveCount(0);
  await expect(swatches(page)).toHaveCount(48);
  expect(await statusBar(page).textContent()).toBe(before);
});

test('dismissing the save dialog writes nothing and says nothing (AC-23)', async ({ page }) => {
  await savePicker(page, 'cancel');
  let downloads = 0;
  page.on('download', () => (downloads += 1));
  await createDocument(page, 'image', { width: 16, height: 16 });
  const before = await statusBar(page).textContent();
  await chooseSave(page, 'GIMP .gpl');
  await expect.poll(() => pickerCalls(page)).toBe(1);
  await expect(page.getByRole('menu', { name: 'Palette options' })).toHaveCount(0);
  expect(await statusBar(page).textContent()).toBe(before);
  expect(downloads).toBe(0);
});

test('a failed write says so and keeps the palette (AC-24)', async ({ page }) => {
  await savePicker(page, 'fail');
  await createDocument(page, 'image', { width: 16, height: 16 });
  await chooseSave(page, 'Lospec .hex');
  await expect(statusBar(page)).toContainText('Save palette failed: disk full');
  await expect(swatches(page)).toHaveCount(48);
  await expect(swatches(page).first()).toHaveAccessibleName('#000000');
});
