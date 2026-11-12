import { readFile } from 'node:fs/promises';
import { expect, test, type Download, type Locator, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu, switchLanguage, waitForEditor } from './helpers';

// Pixel-mode PNG frame and sprite-sheet export (docs/specs/pixel-export.md).
// Force the download / <input type=file> fallbacks: the File System Access
// pickers would open native dialogs the test cannot drive.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

/** The Pixel-mode status bar message. */
function status(page: Page, text: string | RegExp): Locator {
  return page.locator('span.text-pc-muted').getByText(text);
}

/** Collects every download the page starts, in order. */
function collectDownloads(page: Page): Download[] {
  const downloads: Download[] = [];
  page.on('download', (d) => downloads.push(d));
  return downloads;
}

/** Runs `action` and returns the download it triggers. */
async function download(
  page: Page,
  action: () => Promise<void>,
): Promise<{ name: string; bytes: Buffer }> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return { name: d.suggestedFilename(), bytes: await readFile(await d.path()) };
}

/**
 * Opens the PNG export dialog with `open` and confirms it at the
 * preselected scale (docs/specs/scaled-export.md AC-01).
 */
async function confirmPng(page: Page, open: () => Promise<void>): Promise<void> {
  await open();
  await page
    .getByRole('dialog', { name: 'Export PNG' })
    .getByRole('button', { name: 'Export' })
    .click();
}

/** Width and height from a PNG's IHDR. */
function pngSize(png: Buffer): [number, number] {
  return [png.readUInt32BE(16), png.readUInt32BE(20)];
}

/** An 8 × 8 sprite with five frames, `tag1` over frames 1–2 and `tag2`
 *  over frames 3–5. */
async function fiveFramesTwoTags(page: Page): Promise<void> {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const timeline = page.getByRole('region', { name: 'Timeline' });
  for (let i = 0; i < 4; i += 1) {
    await timeline.getByRole('button', { name: 'Add frame' }).click();
  }
  await expect(timeline.getByRole('button', { name: 'Frame 5', exact: true })).toBeVisible();
  const tagTo = timeline.getByLabel('Tag editor').getByLabel('Tag to');
  for (const [from, to] of [
    ['1', '2'],
    ['3', '5'],
  ] as const) {
    await timeline.getByRole('button', { name: `Frame ${from}`, exact: true }).click();
    await timeline.getByRole('button', { name: 'Add tag' }).click();
    await tagTo.fill(to);
    await tagTo.press('Enter');
  }
}

/** A 4 × 4 RGBA `.aseprite` with one layer and two frames: frame 1 holds
 *  an opaque cel, frame 2 a cel linked to its own frame — a broken link
 *  `compose()` refuses (linked-cels-group-compose.md AC-06). */
function asepriteWithLinkedCel(): Buffer {
  const chunk = (type: number, data: Buffer) => {
    const head = Buffer.alloc(6);
    head.writeUInt32LE(6 + data.length, 0);
    head.writeUInt16LE(type, 4);
    return Buffer.concat([head, data]);
  };
  const name = Buffer.alloc(2 + 7);
  name.writeUInt16LE(7, 0);
  name.write('Layer 1', 2, 'latin1');
  const layer = Buffer.alloc(18);
  layer.writeUInt16LE(3, 0); // visible | editable
  layer.writeUInt8(255, 12); // opacity
  const celHead = (type: number) => {
    const head = Buffer.alloc(16);
    head.writeUInt8(255, 6); // opacity
    head.writeUInt16LE(type, 7);
    return head;
  };
  const raw = Buffer.alloc(4 + 4 * 4 * 4, 0xc0);
  raw.writeUInt16LE(4, 0);
  raw.writeUInt16LE(4, 2);
  const link = Buffer.alloc(2);
  link.writeUInt16LE(1, 0); // frame position 1: the linking frame itself
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
      chunk(0x2004, Buffer.concat([layer, name])),
      chunk(0x2005, Buffer.concat([celHead(0), raw])),
    ]),
    frame([chunk(0x2005, Buffer.concat([celHead(1), link]))]),
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

test('sprite sheet export writes the PNG and the Aseprite JSON', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  await timeline.getByLabel('Tag editor').getByLabel('Tag to').fill('2');
  await timeline.getByLabel('Tag editor').getByLabel('Tag to').press('Enter');

  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  await dialog.getByLabel('Columns').fill('3');
  await expect(dialog.getByText('3 × 1 cells · 24×8 px')).toBeVisible();

  const downloads: string[] = [];
  let jsonText = '';
  const both = new Promise<void>((resolve) => {
    page.on('download', async (d) => {
      if (d.suggestedFilename().endsWith('.json'))
        jsonText = await readFile(await d.path(), 'utf8');
      downloads.push(d.suggestedFilename());
      if (downloads.length === 2) resolve();
    });
  });
  await dialog.getByRole('button', { name: 'Export' }).click();
  await both;
  const json = JSON.parse(jsonText) as {
    frames: { duration: number }[];
    meta: {
      image: string;
      size: { w: number; h: number };
      frameTags: { name: string; to: number }[];
    };
  };
  expect(json.frames).toHaveLength(3);
  expect(json.frames[0]?.duration).toBe(100);
  expect(json.meta.size).toEqual({ w: 24, h: 8 });
  expect(json.meta.image).toBe('pincel.png');
  expect(json.meta.frameTags[0]).toMatchObject({ name: 'tag1', to: 1 });
  expect(downloads.sort()).toEqual(['pincel.json', 'pincel.png']);
});

test('PNG (current frame) is named after the document and frame and reports its size', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });

  // One frame: `<document name>.png`, from the menu.
  const single = await download(page, () =>
    confirmPng(page, () => menu(page, 'File', 'Export', 'PNG (current frame)')),
  );
  expect(single.name).toBe('pincel.png');
  expect(pngSize(single.bytes)).toEqual([8, 8]);
  await expect(status(page, `exported PNG · ${single.bytes.length} bytes`)).toBeVisible();

  // Several frames: `<document name>-<n>.png` with the 1-based frame.
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();
  const second = await download(page, () =>
    confirmPng(page, () => page.keyboard.press('Control+e')),
  );
  expect(second.name).toBe('pincel-2.png');
  await expect(status(page, `exported PNG · ${second.bytes.length} bytes`)).toBeVisible();

  await timeline.getByRole('button', { name: 'Frame 3', exact: true }).click();
  const third = await download(page, () =>
    confirmPng(page, () => menu(page, 'File', 'Export', 'PNG (current frame)')),
  );
  expect(third.name).toBe('pincel-3.png');
});

test('a failing export reports its reason in the status bar and saves nothing', async ({
  page,
}) => {
  await gotoStart(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({
    name: 'broken.aseprite',
    mimeType: 'application/octet-stream',
    buffer: asepriteWithLinkedCel(),
  });
  await waitForEditor(page, 'pixel');
  const downloads = collectDownloads(page);
  await page
    .getByRole('region', { name: 'Timeline' })
    .getByRole('button', { name: 'Frame 2', exact: true })
    .click();

  await confirmPng(page, () => menu(page, 'File', 'Export', 'PNG (current frame)'));
  await expect(status(page, /^Export failed: .*linked cel/)).toBeVisible();

  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  await dialog.getByRole('button', { name: 'Export' }).click();
  await expect(status(page, /^Sprite-sheet export failed: .*linked cel/)).toBeVisible();
  expect(downloads).toHaveLength(0);
});

test('the sprite sheet dialog opens with every frame, the tags, √n columns and JSON on', async ({
  page,
}) => {
  await fiveFramesTwoTags(page);
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  for (const open of [
    () => menu(page, 'File', 'Export', 'Sprite Sheet…'),
    () => page.keyboard.press('Control+Alt+E'),
  ]) {
    await open();
    await expect(dialog).toBeVisible();
    const frames = dialog.getByLabel('Frames');
    await expect(frames.locator('option')).toHaveText(['All frames (5)', 'tag1 (2)', 'tag2 (3)']);
    await expect(frames).toHaveValue('');
    await expect(dialog.getByLabel('Columns')).toHaveValue('3');
    await expect(dialog.getByLabel('Write JSON data')).toBeChecked();
    await expect(dialog.getByText('3 × 2 cells · 24×16 px')).toBeVisible();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toBeHidden();
  }
});

test('a tag exports only its frames under <document name>-<tag>', async ({ page }) => {
  await fiveFramesTwoTags(page);
  const downloads = collectDownloads(page);
  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  await dialog.getByLabel('Frames').selectOption('tag2');
  await expect(dialog.getByText('3 × 1 cells · 24×8 px')).toBeVisible();
  await dialog.getByRole('button', { name: 'Export' }).click();
  await expect(status(page, /^exported sprite sheet/)).toBeVisible();

  expect(downloads.map((d) => d.suggestedFilename())).toEqual([
    'pincel-tag2.png',
    'pincel-tag2.json',
  ]);
  const [png, json] = await Promise.all(downloads.map(async (d) => readFile(await d.path())));
  expect(pngSize(png!)).toEqual([24, 8]);
  const data = JSON.parse(json!.toString('utf8')) as {
    frames: unknown[];
    meta: { image: string; frameTags: { name: string; from: number; to: number }[] };
  };
  expect(data.frames).toHaveLength(3);
  expect(data.meta.image).toBe('pincel-tag2.png');
  expect(data.meta.frameTags).toEqual([expect.objectContaining({ name: 'tag2', from: 0, to: 2 })]);
});

test('without the JSON checkbox only the PNG is saved', async ({ page }) => {
  await fiveFramesTwoTags(page);
  const downloads = collectDownloads(page);
  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  await dialog.getByLabel('Write JSON data').uncheck();
  await dialog.getByRole('button', { name: 'Export' }).click();
  // The status line follows the last file the export saves.
  await expect(status(page, /^exported sprite sheet/)).toBeVisible();
  expect(downloads.map((d) => d.suggestedFilename())).toEqual(['pincel.png']);
});

test('Cancel and Escape close the sprite sheet dialog without exporting', async ({ page }) => {
  await fiveFramesTwoTags(page);
  const downloads = collectDownloads(page);
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });

  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();

  await page.keyboard.press('Control+Alt+E');
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();

  // A real export afterwards is the only one that arrives.
  await menu(page, 'File', 'Export', 'Sprite Sheet…');
  await dialog.getByRole('button', { name: 'Export' }).click();
  await expect(status(page, /^exported sprite sheet/)).toBeVisible();
  expect(downloads.map((d) => d.suggestedFilename())).toEqual(['pincel.png', 'pincel.json']);
});

// The grid line of the sprite sheet dialog is translated like the rest of
// the dialog (AGENTS.md §5.4, docs/specs/pixel-export.md AC-16).
test('the sprite sheet grid line follows the German language', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await page
    .getByRole('region', { name: 'Timeline' })
    .getByRole('button', { name: 'Add frame' })
    .click();
  await switchLanguage(page, 'Deutsch');
  await menu(page, 'Datei', 'Exportieren', 'Sprite-Sheet…');
  const dialog = page.getByRole('dialog', { name: 'Sprite-Sheet exportieren' });
  await dialog.getByLabel('Spalten').fill('2');
  await expect(dialog.getByText('2 × 1 Zellen · 16×8 px')).toBeVisible();
});
