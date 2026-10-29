import { readFile } from 'node:fs/promises';
import { crc32, deflateSync } from 'node:zlib';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu, waitForEditor } from './helpers';

// File ▸ Export ▸ Animation… (spec animated-export). Downloads go through
// the fallback: the File System Access pickers would open native dialogs.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

/** A 3-frame sprite with a tag `tag1` over frames 1–2 (indices 0–1). */
async function threeFrames(page: Page, size = { width: 8, height: 8 }): Promise<void> {
  await createDocument(page, 'pixel', size);
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  await timeline.getByLabel('Tag editor').getByLabel('Tag to').fill('2');
  await timeline.getByLabel('Tag editor').getByLabel('Tag to').press('Enter');
}

async function openDialog(page: Page) {
  await menu(page, 'File', 'Export', 'Animation…');
  const dialog = page.getByRole('dialog', { name: 'Export animation' });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Clicks Export and returns the download's name and bytes. */
async function exportDownload(page: Page): Promise<{ name: string; bytes: Buffer }> {
  const dialog = page.getByRole('dialog', { name: 'Export animation' });
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    dialog.getByRole('button', { name: 'Export' }).click(),
  ]);
  return { name: download.suggestedFilename(), bytes: await readFile(await download.path()) };
}

/** `num_frames` of the APNG `acTL` chunk, or `null` without one. */
function actlFrames(png: Buffer): number | null {
  let at = 8;
  while (at + 8 <= png.length) {
    const len = png.readUInt32BE(at);
    const type = png.toString('latin1', at + 4, at + 8);
    if (type === 'acTL') return png.readUInt32BE(at + 8);
    at += 12 + len;
  }
  return null;
}

/** The NETSCAPE2.0 loop count of a GIF, or `null` without the block. */
function netscapeLoops(gif: Buffer): number | null {
  const at = gif.indexOf('NETSCAPE2.0', 0, 'latin1');
  return at < 0 ? null : gif.readUInt16LE(at + 13);
}

function pngChunk(type: string, data: Buffer): Buffer {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
  return Buffer.concat([head, data, crc]);
}

/** An RGBA8 PNG of `w × h` whose pixel `i` is `pixel(i)`. */
function rgbaPng(w: number, h: number, pixel: (i: number) => number[]): Buffer {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const raw: number[] = [];
  for (let y = 0; y < h; y++) {
    raw.push(0);
    for (let x = 0; x < w; x++) raw.push(...pixel(y * w + x));
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', deflateSync(Buffer.from(raw))),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

/** A minimal 4×4 RGBA `.aseprite` with two empty frames, one layer and a
 *  forward tag `walk` over both frames that repeats `repeats` times. */
function asepriteWithTag(repeats: number): Buffer {
  const str = (s: string) => {
    const b = Buffer.alloc(2 + s.length);
    b.writeUInt16LE(s.length, 0);
    b.write(s, 2, 'latin1');
    return b;
  };
  const chunk = (type: number, data: Buffer) => {
    const head = Buffer.alloc(6);
    head.writeUInt32LE(6 + data.length, 0);
    head.writeUInt16LE(type, 4);
    return Buffer.concat([head, data]);
  };
  const layer = Buffer.alloc(18);
  layer.writeUInt16LE(3, 0); // visible | editable
  layer.writeUInt8(255, 12); // opacity
  const tags = Buffer.alloc(10);
  tags.writeUInt16LE(1, 0);
  const tag = Buffer.alloc(17);
  tag.writeUInt16LE(0, 0); // from
  tag.writeUInt16LE(1, 2); // to
  tag.writeUInt8(0, 4); // forward
  tag.writeUInt16LE(repeats, 5);
  const frame = (chunks: Buffer[]) => {
    const body = Buffer.concat(chunks);
    const head = Buffer.alloc(16);
    head.writeUInt32LE(16 + body.length, 0);
    head.writeUInt16LE(0xf1fa, 4);
    head.writeUInt16LE(chunks.length, 6);
    head.writeUInt16LE(100, 8);
    head.writeUInt32LE(chunks.length, 12);
    return Buffer.concat([head, body]);
  };
  const frames = Buffer.concat([
    frame([
      chunk(0x2004, Buffer.concat([layer, str('Layer 1')])),
      chunk(0x2018, Buffer.concat([tags, tag, str('walk')])),
    ]),
    frame([]),
  ]);
  const header = Buffer.alloc(128);
  header.writeUInt32LE(128 + frames.length, 0);
  header.writeUInt16LE(0xa5e0, 4);
  header.writeUInt16LE(2, 6); // frames
  header.writeUInt16LE(4, 8);
  header.writeUInt16LE(4, 10);
  header.writeUInt16LE(32, 12); // RGBA
  header.writeUInt32LE(1, 14);
  header.writeUInt16LE(100, 18);
  header.writeUInt8(1, 34); // pixel width
  header.writeUInt8(1, 35); // pixel height
  return Buffer.concat([header, frames]);
}

test('the dialog offers format, frames, scale and loop and estimates the output', async ({
  page,
}) => {
  await threeFrames(page);
  const dialog = await openDialog(page);
  await expect(dialog.getByLabel('Format').locator('option')).toHaveText([
    'APNG (animated PNG)',
    'GIF',
  ]);
  await expect(dialog.getByLabel('Frames').locator('option')).toHaveText([
    'All frames (3)',
    'tag1 (2)',
  ]);
  await expect(dialog.getByLabel('Scale').locator('option')).toHaveText(
    Array.from({ length: 10 }, (_, i) => `×${i + 1}`),
  );
  await expect(dialog.getByRole('radio', { name: 'Forever' })).toBeChecked();
  await expect(dialog.getByLabel('Play count')).toBeDisabled();
  await expect(dialog.getByText('Frames: 3 · size: 8×8 px')).toBeVisible();

  await dialog.getByLabel('Scale').selectOption('3');
  await expect(dialog.getByText('Frames: 3 · size: 24×24 px')).toBeVisible();
  await dialog.getByLabel('Frames').selectOption('tag1');
  await expect(dialog.getByText('Frames: 2 · size: 24×24 px')).toBeVisible();
  // A tag with repeat count 0 keeps "forever".
  await expect(dialog.getByRole('radio', { name: 'Forever' })).toBeChecked();
});

test('APNG export downloads <stem>.png with an acTL frame count', async ({ page }) => {
  await threeFrames(page);
  await openDialog(page);
  const { name, bytes } = await exportDownload(page);
  expect(name).toBe('pincel.png');
  expect([...bytes.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  expect(actlFrames(bytes)).toBe(3);
  await expect(page.getByText(/exported animation · \d+ bytes/)).toBeVisible();
});

test('GIF export downloads <stem>.gif with the NETSCAPE loop count', async ({ page }) => {
  await threeFrames(page);
  let dialog = await openDialog(page);
  await dialog.getByLabel('Format').selectOption('gif');
  const forever = await exportDownload(page);
  expect(forever.name).toBe('pincel.gif');
  expect(forever.bytes.toString('latin1', 0, 6)).toBe('GIF89a');
  expect(netscapeLoops(forever.bytes)).toBe(0);

  dialog = await openDialog(page);
  await dialog.getByLabel('Format').selectOption('gif');
  await dialog.getByRole('radio', { name: 'Play a fixed number of times' }).check();
  await dialog.getByLabel('Play count').fill('3');
  const thrice = await exportDownload(page);
  expect(netscapeLoops(thrice.bytes)).toBe(2);

  dialog = await openDialog(page);
  await dialog.getByLabel('Format').selectOption('gif');
  await dialog.getByRole('radio', { name: 'Play a fixed number of times' }).check();
  await dialog.getByLabel('Play count').fill('1');
  const once = await exportDownload(page);
  expect(netscapeLoops(once.bytes)).toBeNull();
});

test('GIF notices warn about approximated colours and on/off transparency', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 20, height: 20 });
  // 300 distinct opaque colours plus one half-transparent pixel.
  const png = rgbaPng(20, 20, (i) =>
    i === 399 ? [10, 20, 30, 128] : i < 300 ? [i % 256, Math.floor(i / 256) * 90, 7, 255] : [0, 0, 0, 0],
  );
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    menu(page, 'File', 'Import PNG as Layer…'),
  ]);
  await chooser.setFiles({ name: 'many.png', mimeType: 'image/png', buffer: png });
  await expect(page.getByText('imported many.png')).toBeVisible();

  const dialog = await openDialog(page);
  const approximated = dialog.getByText('Some frames have more than 256 colours');
  const alpha = dialog.getByText('GIF transparency is on/off only');
  await expect(approximated).toBeHidden();
  await expect(alpha).toBeHidden();
  await dialog.getByLabel('Format').selectOption('gif');
  await expect(approximated).toBeVisible();
  await expect(alpha).toBeVisible();
  const { name, bytes } = await exportDownload(page);
  expect(name).toBe('pincel.gif');
  expect(bytes.toString('latin1', 0, 6)).toBe('GIF89a');
});

test('the loop choice defaults to the tag repeat count', async ({ page }) => {
  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({
    name: 'walker.aseprite',
    mimeType: 'application/octet-stream',
    buffer: asepriteWithTag(4),
  });
  await waitForEditor(page, 'pixel');
  const dialog = await openDialog(page);
  await expect(dialog.getByRole('radio', { name: 'Forever' })).toBeChecked();
  await dialog.getByLabel('Frames').selectOption('walk');
  await expect(dialog.getByRole('radio', { name: 'Play a fixed number of times' })).toBeChecked();
  await expect(dialog.getByLabel('Play count')).toHaveValue('4');
  await dialog.getByLabel('Frames').selectOption('');
  await expect(dialog.getByRole('radio', { name: 'Forever' })).toBeChecked();
  const { name, bytes } = await exportDownload(page);
  expect(name).toBe('walker.png');
  expect(actlFrames(bytes)).toBe(2);
});

test('an output over 16384 px fails in the status bar without a download', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 1700, height: 4 });
  let downloads = 0;
  page.on('download', () => (downloads += 1));
  const dialog = await openDialog(page);
  await dialog.getByLabel('Scale').selectOption('10');
  await expect(dialog.getByText('Frames: 1 · size: 17000×40 px')).toBeVisible();
  await dialog.getByRole('button', { name: 'Export' }).click();
  await expect(page.getByText(/Animation export failed: .*16384/)).toBeVisible();
  expect(downloads).toBe(0);
});
