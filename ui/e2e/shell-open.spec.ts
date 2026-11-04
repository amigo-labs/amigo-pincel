import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  RASTER_MIME,
  drag,
  fakeFileHandles,
  fakeFs,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  rasterFile,
  type RasterKind,
  waitForEditor,
} from './helpers';

// File → mode routing of the app shell (docs/specs/app-shell.md AC-09 to
// AC-12, AC-14 for Open Recent): a file's leading bytes pick the editor,
// the extension only when no signature matches.

type Mode = 'pixel' | 'image';

/** A 32 × 24 sprite (the writer's compatibility fixture). */
const sprite = () =>
  readFile(
    join(import.meta.dirname, '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite'),
  );

/** Size of each raster sample (`rasterFile`). */
const RASTER_SIZE: Record<RasterKind, [number, number]> = {
  png: [8, 6],
  jpeg: [8, 6],
  webp: [8, 6],
  bmp: [4, 3],
  tiff: [4, 3],
  gif: [1, 1],
};

/** The status bar's document size of `mode`. */
function sizeShown(page: Page, mode: Mode, [w, h]: [number, number]) {
  return page.getByText(mode === 'pixel' ? `${w}×${h}` : `${w} × ${h} · sRGB`, { exact: true });
}

/** The editor's mode badge in the top bar. */
function badge(page: Page, mode: Mode) {
  const label = mode === 'pixel' ? 'Pixel' : 'Image';
  return page.getByRole('banner').locator('span').filter({ hasText: new RegExp(`^${label}$`) });
}

/** Waits for the editor of `mode` and checks that it is the only one. */
async function expectEditor(page: Page, mode: Mode, size: [number, number]): Promise<void> {
  await waitForEditor(page, mode);
  await expect(mode === 'pixel' ? imageCanvas(page) : pixelCanvas(page)).toHaveCount(0);
  await expect(sizeShown(page, mode, size)).toBeVisible();
}

/** Runs `open` and answers the <input type=file> chooser it raises. */
async function choose(
  page: Page,
  open: () => Promise<void>,
  file: { name: string; mimeType: string; buffer: Buffer },
): Promise<void> {
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), open()]);
  await chooser.setFiles(file);
}

/** Opens a file through the start screen's Open… (input fallback). */
async function openFromStart(page: Page, name: string, buffer: Buffer, mimeType = ''): Promise<void> {
  await gotoStart(page);
  await choose(page, () => page.getByRole('button', { name: 'Open…' }).click(), {
    name,
    mimeType,
    buffer,
  });
}

/** Drops a file onto the start screen. */
async function dropOnStart(page: Page, name: string, buffer: Buffer): Promise<void> {
  await gotoStart(page);
  await page.getByRole('main').evaluate(
    (el, [n, bytes]) => {
      const data = new DataTransfer();
      data.items.add(new File([new Uint8Array(bytes)], n));
      el.dispatchEvent(new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }));
    },
    [name, Array.from(buffer)] as const,
  );
}

/** Opens `name` through the start screen's Open… and the fake picker. */
async function pickFromStart(page: Page, name: string): Promise<void> {
  await gotoStart(page);
  await fakeFs(page, { pick: name });
  await page.getByRole('button', { name: 'Open…' }).click();
}

/** One edit that makes the document dirty. */
async function edit(page: Page, mode: Mode): Promise<void> {
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(page, mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
}

test.describe('input fallback', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
    });
  });

  test('AC-09: start screen Open… puts a sprite into the Pixel editor', async ({ page }) => {
    await openFromStart(page, 'hero.aseprite', await sprite());
    await expectEditor(page, 'pixel', [32, 24]);
  });

  // OpenRaster from the start screen's Open…: ora.spec.ts › image mode:
  // Save writes an OpenRaster file that reopens with its layers.
  for (const kind of Object.keys(RASTER_SIZE) as RasterKind[]) {
    test(`AC-09: start screen Open… puts a ${kind} into the Image editor`, async ({ page }) => {
      await gotoStart(page);
      const bytes = await rasterFile(page, kind);
      await openFromStart(page, `picture.${kind}`, bytes, RASTER_MIME[kind]);
      await expectEditor(page, 'image', RASTER_SIZE[kind]);
    });
  }

  test('AC-09: Image mode File ▸ Open… routes a sprite to Pixel and an image to Image', async ({
    page,
  }) => {
    await gotoStart(page);
    const gif = await rasterFile(page, 'gif');
    await openFromStart(page, 'photo.jpeg', await rasterFile(page, 'jpeg'), 'image/jpeg');
    await expectEditor(page, 'image', [8, 6]);
    await choose(page, () => menu(page, 'File', 'Open…'), {
      name: 'hero.aseprite',
      mimeType: '',
      buffer: await sprite(),
    });
    await expectEditor(page, 'pixel', [32, 24]);

    await openFromStart(page, 'photo.jpeg', await rasterFile(page, 'jpeg'), 'image/jpeg');
    await expectEditor(page, 'image', [8, 6]);
    await choose(page, () => menu(page, 'File', 'Open…'), {
      name: 'anim.gif',
      mimeType: 'image/gif',
      buffer: gif,
    });
    await expectEditor(page, 'image', [1, 1]);
  });

  test('AC-09: a file dropped on the start screen opens in its editor', async ({ page }) => {
    await dropOnStart(page, 'hero.aseprite', await sprite());
    await expectEditor(page, 'pixel', [32, 24]);
    await gotoStart(page);
    const tiff = await rasterFile(page, 'tiff');
    await dropOnStart(page, 'scan.tif', tiff);
    await expectEditor(page, 'image', [4, 3]);
  });

  test('AC-10: the leading bytes decide, the extension only without a signature', async ({
    page,
  }) => {
    await gotoStart(page);
    const png = await rasterFile(page, 'png');
    // A PNG named like a sprite is an image; a sprite named like a PNG is a sprite.
    await openFromStart(page, 'x.aseprite', png);
    await expectEditor(page, 'image', [8, 6]);
    await openFromStart(page, 'hero.png', await sprite());
    await expectEditor(page, 'pixel', [32, 24]);
    // No known signature: the extension routes (the editor then reports
    // that it cannot read the bytes).
    await openFromStart(page, 'broken.aseprite', Buffer.from('not a sprite at all'));
    await expect(badge(page, 'pixel')).toBeVisible();
    await expect(imageCanvas(page)).toHaveCount(0);
    await openFromStart(page, 'broken.gif', Buffer.from('not an image at all'));
    await expect(badge(page, 'image')).toBeVisible();
    await expect(pixelCanvas(page)).toHaveCount(0);
  });

  test('AC-11: Pixel File ▸ Open… keeps sprites and PNGs, hands other images over', async ({
    page,
  }) => {
    await gotoStart(page);
    const png = await rasterFile(page, 'png');
    const jpeg = await rasterFile(page, 'jpeg');
    await openFromStart(page, 'hero.aseprite', await sprite());
    await expectEditor(page, 'pixel', [32, 24]);
    const layers = page.getByRole('list', { name: 'Layer list' }).getByRole('listitem');
    await expect(layers).toHaveCount(5);

    // A PNG stays in Pixel mode as a one-layer sprite.
    await choose(page, () => menu(page, 'File', 'Open…'), {
      name: 'shot.png',
      mimeType: 'image/png',
      buffer: png,
    });
    await expect(sizeShown(page, 'pixel', [8, 6])).toBeVisible();
    await expect(badge(page, 'pixel')).toBeVisible();
    await expect(layers).toHaveCount(1);

    // A sprite stays too.
    await choose(page, () => menu(page, 'File', 'Open…'), {
      name: 'hero.aseprite',
      mimeType: '',
      buffer: await sprite(),
    });
    await expect(sizeShown(page, 'pixel', [32, 24])).toBeVisible();
    await expect(layers).toHaveCount(5);

    // Anything else goes to the Image editor.
    await choose(page, () => menu(page, 'File', 'Open…'), {
      name: 'photo.jpg',
      mimeType: 'image/jpeg',
      buffer: jpeg,
    });
    await expectEditor(page, 'image', [8, 6]);
  });

  test('AC-12: an unknown file is named in a dismissable notice and nothing changes', async ({
    page,
  }) => {
    const junk = Buffer.from('just some notes');
    const notice = page.getByRole('alert').filter({ hasText: 'notes.txt' });

    // From the start screen: it stays.
    await openFromStart(page, 'notes.txt', junk, 'text/plain');
    await expect(notice).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
    await notice.getByRole('button', { name: 'Dismiss' }).click();
    await expect(notice).toHaveCount(0);

    // Dropped: the same.
    await dropOnStart(page, 'notes.txt', junk);
    await expect(notice).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();

    // From an open document: it stays as it was.
    await gotoStart(page);
    await openFromStart(page, 'photo.jpeg', await rasterFile(page, 'jpeg'), 'image/jpeg');
    await expectEditor(page, 'image', [8, 6]);
    // An edit that must survive; the unsaved-changes guard asks first.
    await edit(page, 'image');
    const openUnknown = async () => {
      await menu(page, 'File', 'Open…');
      await page.getByRole('button', { name: 'Discard changes' }).click();
    };
    await choose(page, openUnknown, { name: 'notes.txt', mimeType: 'text/plain', buffer: junk });
    await expect(notice).toBeVisible();
    await expect(sizeShown(page, 'image', [8, 6])).toBeVisible();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
    await expect(page.getByTitle('photo.jpeg — unsaved changes')).toBeVisible();
    await notice.getByRole('button', { name: 'Dismiss' }).click();
    await expect(notice).toHaveCount(0);

    // The Pixel editor's own File ▸ Open…: the same notice, the sprite stays.
    await gotoStart(page);
    await openFromStart(page, 'hero.aseprite', await sprite(), 'application/octet-stream');
    await expectEditor(page, 'pixel', [32, 24]);
    await edit(page, 'pixel');
    const openUnknownInPixel = async () => {
      await menu(page, 'File', 'Open…');
      await page.getByRole('button', { name: 'Discard changes' }).click();
    };
    await choose(page, openUnknownInPixel, {
      name: 'notes.txt',
      mimeType: 'text/plain',
      buffer: junk,
    });
    await expect(notice).toBeVisible();
    await expect(sizeShown(page, 'pixel', [32, 24])).toBeVisible();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
  });
});

test.describe('recent files', () => {
  /** Opens a sprite and an image once each, so both are recents. */
  async function recordRecents(page: Page): Promise<void> {
    await gotoStart(page);
    await fakeFileHandles(page, {
      'hero.aseprite': await sprite(),
      'photo.png': await rasterFile(page, 'png'),
    });
    await pickFromStart(page, 'hero.aseprite');
    await expectEditor(page, 'pixel', [32, 24]);
    await pickFromStart(page, 'photo.png');
    await expectEditor(page, 'image', [8, 6]);
  }

  test('AC-09: Open Recent routes a sprite to Pixel and an image to Image', async ({ page }) => {
    await recordRecents(page);

    // From the Image editor's File ▸ Open Recent.
    await menu(page, 'File', 'Open Recent', 'hero.aseprite');
    await expectEditor(page, 'pixel', [32, 24]);
    // From the Pixel editor's File ▸ Open Recent.
    await menu(page, 'File', 'Open Recent', 'photo.png');
    await expectEditor(page, 'image', [8, 6]);
    // From the start screen's recent list.
    await gotoStart(page);
    const list = page.getByRole('list', { name: 'Recent files' });
    await list.getByRole('button', { name: /^hero\.aseprite/ }).first().click();
    await expectEditor(page, 'pixel', [32, 24]);
    await gotoStart(page);
    await list.getByRole('button', { name: /^photo\.png/ }).first().click();
    await expectEditor(page, 'image', [8, 6]);
  });

  for (const mode of ['pixel', 'image'] as const) {
    test(`AC-14: ${mode}: Open Recent asks before discarding unsaved changes`, async ({ page }) => {
      await recordRecents(page);
      const [here, there] =
        mode === 'pixel' ? ['hero.aseprite', 'photo.png'] : ['photo.png', 'hero.aseprite'];
      if (mode === 'pixel') {
        await menu(page, 'File', 'Open Recent', here);
        await expectEditor(page, 'pixel', [32, 24]);
      }
      await edit(page, mode);
      const confirm = page.getByRole('alertdialog', { name: 'Unsaved changes' });

      await menu(page, 'File', 'Open Recent', there);
      await expect(confirm).toBeVisible();
      await confirm.getByRole('button', { name: 'Cancel' }).click();
      await expect(confirm).toHaveCount(0);
      await expect(badge(page, mode)).toBeVisible();
      await expect(page.getByTitle(`${here} — unsaved changes`)).toBeVisible();
      await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();

      await menu(page, 'File', 'Open Recent', there);
      await confirm.getByRole('button', { name: 'Discard changes' }).click();
      const other: Mode = mode === 'pixel' ? 'image' : 'pixel';
      await expectEditor(page, other, other === 'pixel' ? [32, 24] : [8, 6]);
    });
  }
});
