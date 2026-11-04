import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  dragFiles,
  fakeFileHandles,
  fakeFs,
  fakeFsFile,
  fakeFsLog,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  rasterFile,
  waitForEditor,
  type DraggedFile,
} from './helpers';

// File access across hosts (docs/specs/file-access.md): the File System
// Access pickers, the <input type=file> / download fallbacks and the
// recent-files registry. The FSA pickers are faked (`fakeFileHandles`):
// headless Chromium cannot drive the real ones.

type Mode = 'pixel' | 'image';

/** A 32 × 24 sprite (the writer's compatibility fixture). */
const sprite = () =>
  readFile(
    join(
      import.meta.dirname,
      '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite',
    ),
  );

/** Every extension Open… accepts (AC-01). */
const DOC_EXTENSIONS = [
  '.aseprite',
  '.ase',
  '.ora',
  '.png',
  '.jpg',
  '.jpeg',
  '.webp',
  '.bmp',
  '.gif',
  '.tif',
  '.tiff',
  '.psd',
];

/** True when `bytes` carry the Aseprite header magic. */
const isAseprite = (bytes: Buffer) => bytes[4] === 0xe0 && bytes[5] === 0xa5;

/** True when `bytes` are an OpenRaster file. */
const isOra = (bytes: Buffer) =>
  bytes.subarray(30, 54).toString('latin1') === 'mimetypeimage/openraster';

/** The status bar's document size of `mode`. */
function sizeShown(page: Page, mode: Mode, [w, h]: [number, number]) {
  return page.getByText(mode === 'pixel' ? `${w}×${h}` : `${w} × ${h} · sRGB`, { exact: true });
}

/** The unsaved marker of the top bar for document `name`. */
const unsaved = (page: Page, name: string) => page.getByTitle(`${name} — unsaved changes`);

/** One edit that makes the document dirty. */
async function edit(page: Page, mode: Mode): Promise<void> {
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(
    page,
    mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page),
    [0.2, 0.5],
    [0.8, 0.5],
  );
}

/** Opens `name` through the start screen's Open… and the fake picker. */
async function pickFromStart(page: Page, name: string): Promise<void> {
  await gotoStart(page);
  await fakeFs(page, { pick: name });
  await page.getByRole('button', { name: 'Open…' }).click();
}

/** A row of the recent-files registry. */
interface RecentRow {
  name: string;
  mode: Mode;
  path: string | null;
  openedAt: number;
}

/** The recent-files registry, read straight from IndexedDB, newest first. */
async function recentRows(page: Page): Promise<RecentRow[]> {
  return page.evaluate(
    () =>
      new Promise<RecentRow[]>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const db = open.result;
          if (!db.objectStoreNames.contains('recent_files')) {
            resolve([]);
            return;
          }
          const all = db.transaction('recent_files').objectStore('recent_files').getAll();
          all.onerror = () => reject(all.error);
          all.onsuccess = () =>
            resolve(
              (all.result as RecentRow[])
                .map(({ name, mode, path, openedAt }) => ({ name, mode, path, openedAt }))
                .sort((a, b) => b.openedAt - a.openedAt),
            );
        };
      }),
  );
}

/** The registry as `name/mode` pairs, newest first. */
const recentNames = async (page: Page) =>
  (await recentRows(page)).map((r) => `${r.name}/${r.mode}`);

/** The start screen's Recent list. */
const recentList = (page: Page) => page.getByRole('list', { name: 'Recent files' });

/** The shell's error notice that contains `text`. */
const notice = (page: Page, text: string | RegExp) =>
  page.getByRole('alert').filter({ hasText: text });

/** Dismisses the shell notice holding `text` and checks that it is gone. */
async function dismiss(page: Page, text: string | RegExp): Promise<void> {
  await notice(page, text).getByRole('button', { name: 'Dismiss' }).click();
  await expect(notice(page, text)).toHaveCount(0);
}

/** Opens `name` from an open editor's File ▸ Open… with the fake picker. */
async function pickInEditor(page: Page, name: string): Promise<void> {
  await fakeFs(page, { pick: name });
  await menu(page, 'File', 'Open…');
}

/** Dispatches a drag event carrying `files` (or only text) at the start screen. */
const dragAtStart = (
  page: Page,
  type: 'dragover' | 'dragleave' | 'drop',
  files: DraggedFile[] | 'text',
) => dragFiles(page.getByRole('main'), type, files);

/** The names of the fake disk's in-place writes so far. */
const writes = async (page: Page) => (await fakeFsLog(page)).writes;

/** The picker calls of kind `picker` so far. */
const pickerCalls = async (page: Page, picker: 'open' | 'save') =>
  (await fakeFsLog(page)).pickers.filter((p) => p.picker === picker);

test.describe('File System Access', () => {
  test('AC-01: Open… shows the picker restricted to the document types', async ({ page }) => {
    await gotoStart(page);
    await fakeFileHandles(page, {});
    const expectOpenPicker = async () => {
      await expect.poll(async () => (await pickerCalls(page, 'open')).length).toBe(1);
      const [call] = await pickerCalls(page, 'open');
      expect(call?.multiple).toBe(false);
      const accepted = (call?.types ?? []).flatMap((t) => Object.values(t.accept).flat());
      expect(accepted.sort()).toEqual([...DOC_EXTENSIONS].sort());
    };

    // The start screen's Open… (the picker is cancelled).
    await gotoStart(page);
    await page.getByRole('button', { name: 'Open…' }).click();
    await expectOpenPicker();

    // File ▸ Open… of both editors.
    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode);
      await menu(page, 'File', 'Open…');
      await expectOpenPicker();
    }
  });

  test('AC-03: a cancelled open picker changes nothing and shows no error', async ({ page }) => {
    await gotoStart(page);
    await fakeFileHandles(page, {
      'hero.aseprite': await sprite(),
      'photo.png': await rasterFile(page, 'png'),
    });
    const alert = page.getByRole('alert');

    // Start screen: still there, and Open… works again.
    await gotoStart(page);
    await page.getByRole('button', { name: 'Open…' }).click();
    await expect.poll(async () => (await pickerCalls(page, 'open')).length).toBe(1);
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Open…' })).toBeEnabled();
    await expect(alert).toHaveCount(0);

    // An open document stays as it was, in both editors.
    for (const [name, mode, size] of [
      ['hero.aseprite', 'pixel', [32, 24]],
      ['photo.png', 'image', [8, 6]],
    ] as const) {
      await pickFromStart(page, name);
      await waitForEditor(page, mode);
      await fakeFs(page, { pick: null });
      await menu(page, 'File', 'Open…');
      await expect.poll(async () => (await pickerCalls(page, 'open')).length).toBe(2);
      await expect(sizeShown(page, mode, [...size])).toBeVisible();
      await expect(page.getByTitle(name, { exact: true })).toBeVisible();
      await expect(alert).toHaveCount(0);
      await expect(page.getByText(/failed/i)).toHaveCount(0);
    }
  });

  for (const mode of ['pixel', 'image'] as const) {
    const [file, size] =
      mode === 'pixel' ? (['hero.aseprite', [32, 24]] as const) : (['photo.png', [8, 6]] as const);

    test(`AC-05: ${mode}: Save writes back to the opened file, Save As asks for a location`, async ({
      page,
    }) => {
      await gotoStart(page);
      await fakeFileHandles(page, {
        'hero.aseprite': await sprite(),
        'photo.png': await rasterFile(page, 'png'),
      });
      await pickFromStart(page, file);
      await waitForEditor(page, mode);
      await expect(sizeShown(page, mode, [...size])).toBeVisible();

      await edit(page, mode);
      await expect(unsaved(page, file)).toBeVisible();
      await menu(page, 'File', 'Save');
      await expect.poll(() => writes(page)).toEqual([file]);
      await expect(unsaved(page, file)).toHaveCount(0);
      expect(await pickerCalls(page, 'save')).toHaveLength(0);

      // Save As asks even though the document has a file.
      await fakeFs(page, { savePick: null });
      await menu(page, 'File', 'Save As…');
      if (mode === 'image') {
        await page
          .getByRole('dialog', { name: 'Save As' })
          .getByRole('button', { name: 'Save' })
          .click();
      }
      await expect.poll(async () => (await pickerCalls(page, 'save')).length).toBe(1);
      expect(await writes(page)).toEqual([file]);
    });

    test(`AC-06: ${mode}: the first Save offers the picker, later Saves write to the chosen file`, async ({
      page,
    }) => {
      await gotoStart(page);
      await fakeFileHandles(page, {});
      const [suggested, chosen, accept, isProject] =
        mode === 'pixel'
          ? (['pincel.aseprite', 'sprite.aseprite', ['.aseprite', '.ase'], isAseprite] as const)
          : (['image.ora', 'art.ora', ['.ora'], isOra] as const);
      await createDocument(page, mode);
      await fakeFs(page, { savePick: chosen });
      await edit(page, mode);
      await menu(page, 'File', 'Save');
      await expect.poll(() => writes(page)).toEqual([chosen]);
      const [call] = await pickerCalls(page, 'save');
      expect(call?.suggestedName).toBe(suggested);
      expect((call?.types ?? []).flatMap((t) => Object.values(t.accept).flat())).toEqual(accept);
      expect(isProject(await fakeFsFile(page, chosen))).toBe(true);
      await expect(page.getByTitle(chosen, { exact: true })).toBeVisible();

      await edit(page, mode);
      await expect(unsaved(page, chosen)).toBeVisible();
      await menu(page, 'File', 'Save');
      await expect.poll(() => writes(page)).toEqual([chosen, chosen]);
      await expect(unsaved(page, chosen)).toHaveCount(0);
      expect(await pickerCalls(page, 'save')).toHaveLength(1);
    });

    test(`${mode}: a cancelled Save or Save As leaves the document unsaved`, async ({ page }) => {
      await gotoStart(page);
      await fakeFileHandles(page, {});
      const name = mode === 'pixel' ? 'pincel.aseprite' : 'image.ora';
      await createDocument(page, mode);
      await fakeFs(page, { savePick: null });
      await edit(page, mode);
      await expect(unsaved(page, name)).toBeVisible();

      await menu(page, 'File', 'Save');
      await expect.poll(async () => (await pickerCalls(page, 'save')).length).toBe(1);
      await expect(unsaved(page, name)).toBeVisible();

      await menu(page, 'File', 'Save As…');
      if (mode === 'image') {
        await page
          .getByRole('dialog', { name: 'Save As' })
          .getByRole('button', { name: 'Save' })
          .click();
      }
      await expect.poll(async () => (await pickerCalls(page, 'save')).length).toBe(2);
      await expect(unsaved(page, name)).toBeVisible();
      expect(await writes(page)).toEqual([]);
    });

    test(`AC-07: ${mode}: a denied write permission falls back to the save picker`, async ({
      page,
    }) => {
      await gotoStart(page);
      await fakeFileHandles(page, {
        'hero.aseprite': await sprite(),
        'photo.png': await rasterFile(page, 'png'),
      });
      await pickFromStart(page, file);
      await waitForEditor(page, mode);
      const copy = mode === 'pixel' ? 'copy.aseprite' : 'copy.png';
      await fakeFs(page, { denyWrite: [file], savePick: copy });
      await edit(page, mode);
      await menu(page, 'File', 'Save');
      await expect.poll(() => writes(page)).toEqual([copy]);
      const [call] = await pickerCalls(page, 'save');
      expect(call?.suggestedName).toBe(file);
    });
  }
});

test.describe('recent files', () => {
  test('AC-10: opens and saves through a handle are recorded once per document', async ({
    page,
  }) => {
    await gotoStart(page);
    await fakeFileHandles(page, {
      'hero.aseprite': await sprite(),
      'villain.aseprite': await sprite(),
      'photo.png': await rasterFile(page, 'png'),
    });
    await pickFromStart(page, 'photo.png');
    await waitForEditor(page, 'image');
    await expect.poll(() => recentNames(page)).toEqual(['photo.png/image']);
    await pickFromStart(page, 'hero.aseprite');
    await waitForEditor(page, 'pixel');
    await expect.poll(() => recentNames(page)).toEqual(['hero.aseprite/pixel', 'photo.png/image']);
    // The Pixel editor's own Open….
    await pickInEditor(page, 'villain.aseprite');
    await expect
      .poll(() => recentNames(page))
      .toEqual(['villain.aseprite/pixel', 'hero.aseprite/pixel', 'photo.png/image']);

    // Re-opened through Pixel's Open Recent: moved to the top, not added again.
    await menu(page, 'File', 'Open Recent', 'hero.aseprite');
    await expect
      .poll(() => recentNames(page))
      .toEqual(['hero.aseprite/pixel', 'villain.aseprite/pixel', 'photo.png/image']);
    // Saved again: still one entry, refreshed.
    const [opened] = await recentRows(page);
    await edit(page, 'pixel');
    await menu(page, 'File', 'Save');
    await expect.poll(() => writes(page)).toEqual(['hero.aseprite']);
    await expect
      .poll(async () => (await recentRows(page))[0]?.openedAt)
      .toBeGreaterThan(opened?.openedAt ?? Infinity);
    expect(await recentNames(page)).toEqual([
      'hero.aseprite/pixel',
      'villain.aseprite/pixel',
      'photo.png/image',
    ]);

    // An Image-mode document saved through the save picker, then again.
    await createDocument(page, 'image');
    await fakeFs(page, { savePick: 'art.ora' });
    await edit(page, 'image');
    await menu(page, 'File', 'Save');
    await expect.poll(() => recentNames(page)).toHaveLength(4);
    expect((await recentNames(page))[0]).toBe('art.ora/image');
    await edit(page, 'image');
    await menu(page, 'File', 'Save');
    await expect.poll(() => writes(page)).toEqual(['art.ora', 'art.ora']);
    expect(await recentNames(page)).toEqual([
      'art.ora/image',
      'hero.aseprite/pixel',
      'villain.aseprite/pixel',
      'photo.png/image',
    ]);
  });

  test('AC-11: the registry keeps the 8 newest entries, newest first', async ({ page }) => {
    await gotoStart(page);
    const names = Array.from({ length: 9 }, (_, i) => `s${i + 1}.aseprite`);
    const bytes = await sprite();
    await fakeFileHandles(page, Object.fromEntries(names.map((n) => [n, bytes])));
    await createDocument(page, 'pixel');
    for (const [i, name] of names.entries()) {
      await pickInEditor(page, name);
      await expect.poll(async () => (await recentRows(page))[0]?.name).toBe(name);
      expect(await recentRows(page)).toHaveLength(Math.min(i + 1, 8));
    }
    const newest = names.slice(1).reverse();
    expect((await recentRows(page)).map((r) => r.name)).toEqual(newest);
    await menu(page, 'File', 'Open Recent');
    await expect(
      page.getByRole('menu', { name: 'Open Recent', exact: true }).getByRole('menuitem'),
    ).toHaveText(newest);
  });

  test('AC-13: the start screen lists recents with their mode and re-opens them', async ({
    page,
  }) => {
    await gotoStart(page);
    await fakeFileHandles(page, {
      'hero.aseprite': await sprite(),
      'photo.png': await rasterFile(page, 'png'),
    });
    await gotoStart(page);
    await expect(recentList(page)).toHaveCount(0);
    await pickFromStart(page, 'hero.aseprite');
    await waitForEditor(page, 'pixel');
    await pickFromStart(page, 'photo.png');
    await waitForEditor(page, 'image');

    await gotoStart(page);
    await expect(page.getByRole('heading', { name: 'Recent', exact: true })).toBeVisible();
    await expect(recentList(page).getByRole('listitem')).toHaveText([
      /^photo\.png\s*Image$/,
      /^hero\.aseprite\s*Pixel$/,
    ]);
    await recentList(page)
      .getByRole('button', { name: /^photo\.png/ })
      .click();
    await waitForEditor(page, 'image');
    await expect(sizeShown(page, 'image', [8, 6])).toBeVisible();
    await gotoStart(page);
    await recentList(page)
      .getByRole('button', { name: /^hero\.aseprite/ })
      .click();
    await waitForEditor(page, 'pixel');
    await expect(sizeShown(page, 'pixel', [32, 24])).toBeVisible();
  });

  test('AC-14: a denied read permission names the file and opens nothing', async ({ page }) => {
    await gotoStart(page);
    await fakeFileHandles(page, {
      'hero.aseprite': await sprite(),
      'villain.aseprite': await sprite(),
      'photo.png': await rasterFile(page, 'png'),
    });
    await pickFromStart(page, 'hero.aseprite');
    await waitForEditor(page, 'pixel');
    await pickFromStart(page, 'photo.png');
    await waitForEditor(page, 'image');
    const denied = 'hero.aseprite: permission denied';

    // The start screen's Recent list.
    await gotoStart(page);
    await fakeFs(page, { denyRead: ['hero.aseprite'] });
    await recentList(page)
      .getByRole('button', { name: /^hero\.aseprite/ })
      .click();
    await expect(notice(page, denied)).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
    await expect(pixelCanvas(page)).toHaveCount(0);
    await dismiss(page, denied);

    // Image mode's File ▸ Open Recent.
    await recentList(page)
      .getByRole('button', { name: /^photo\.png/ })
      .click();
    await waitForEditor(page, 'image');
    await menu(page, 'File', 'Open Recent', 'hero.aseprite');
    await expect(notice(page, denied)).toBeVisible();
    await expect(sizeShown(page, 'image', [8, 6])).toBeVisible();
    await expect(page.getByTitle('photo.png', { exact: true })).toBeVisible();
    await dismiss(page, denied);

    // Pixel mode's File ▸ Open Recent.
    await pickInEditor(page, 'villain.aseprite');
    await waitForEditor(page, 'pixel');
    await expect(page.getByTitle('villain.aseprite', { exact: true })).toBeVisible();
    await menu(page, 'File', 'Open Recent', 'hero.aseprite');
    await expect(notice(page, denied)).toBeVisible();
    await expect(page.getByTitle('villain.aseprite', { exact: true })).toBeVisible();
    await dismiss(page, denied);
  });

  test('AC-12: no IndexedDB: Open Recent is absent in both modes', async ({ page }) => {
    await page.addInitScript(() => {
      Object.defineProperty(window, 'indexedDB', { value: undefined, configurable: true });
    });
    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode);
      await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
      const file = page.getByRole('menu', { name: 'File', exact: true });
      await expect(file.getByRole('menuitem', { name: 'Open…', exact: true })).toBeVisible();
      await expect(file.getByRole('menuitem', { name: 'Open Recent' })).toHaveCount(0);
      await page.keyboard.press('Escape');
    }
  });
});

test.describe('start screen', () => {
  test('AC-15: a dragged file highlights the start screen and opens when dropped', async ({
    page,
  }) => {
    await gotoStart(page);
    const main = page.getByRole('main');
    const highlighted = /(^|\s)bg-pc-panel(\s|$)/;
    const hero = [{ name: 'hero.aseprite', bytes: Array.from(await sprite()) }];
    await expect(main).not.toHaveClass(highlighted);

    await dragAtStart(page, 'dragover', hero);
    await expect(main).toHaveClass(highlighted);
    await dragAtStart(page, 'dragleave', hero);
    await expect(main).not.toHaveClass(highlighted);
    // Dragged text is no file: no drop state.
    await dragAtStart(page, 'dragover', 'text');
    await expect(main).not.toHaveClass(highlighted);

    await dragAtStart(page, 'dragover', hero);
    await dragAtStart(page, 'drop', hero);
    await waitForEditor(page, 'pixel');
    await expect(sizeShown(page, 'pixel', [32, 24])).toBeVisible();
    await expect(page.getByTitle('hero.aseprite', { exact: true })).toBeVisible();
  });

  test('AC-16: a failed read names the failure in a dismissable notice, nothing changes', async ({
    page,
  }) => {
    await gotoStart(page);
    await fakeFileHandles(page, {
      'hero.aseprite': await sprite(),
      'villain.aseprite': await sprite(),
      'photo.png': await rasterFile(page, 'png'),
    });
    await pickFromStart(page, 'hero.aseprite');
    await waitForEditor(page, 'pixel');
    const start = page.getByRole('heading', { name: 'Pincel', exact: true });
    const picked = 'Open failed: hero.aseprite is gone';
    const recent = 'Opening hero.aseprite failed: hero.aseprite is gone';

    // Picked on the start screen.
    await gotoStart(page);
    await fakeFs(page, { failRead: ['hero.aseprite'], pick: 'hero.aseprite' });
    await page.getByRole('button', { name: 'Open…' }).click();
    await expect(notice(page, picked)).toBeVisible();
    await expect(start).toBeVisible();
    await dismiss(page, picked);

    // A recent on the start screen.
    await recentList(page)
      .getByRole('button', { name: /^hero\.aseprite/ })
      .click();
    await expect(notice(page, recent)).toBeVisible();
    await expect(start).toBeVisible();
    await dismiss(page, recent);

    // Dropped on the start screen.
    await page.getByRole('main').evaluate((el) => {
      const read = File.prototype.arrayBuffer;
      File.prototype.arrayBuffer = function (this: File) {
        return this.name === 'broken.png'
          ? Promise.reject(new DOMException('broken.png is unreadable', 'NotReadableError'))
          : read.call(this);
      };
      const data = new DataTransfer();
      data.items.add(new File([new Uint8Array([1, 2, 3])], 'broken.png'));
      el.dispatchEvent(
        new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }),
      );
    });
    const dropped = 'Drop failed: broken.png is unreadable';
    await expect(notice(page, dropped)).toBeVisible();
    await expect(start).toBeVisible();
    await dismiss(page, dropped);

    // From an open document, in both editors: it stays as it was.
    await fakeFs(page, { pick: 'photo.png' });
    await page.getByRole('button', { name: 'Open…' }).click();
    await waitForEditor(page, 'image');
    await pickInEditor(page, 'hero.aseprite');
    await expect(notice(page, picked)).toBeVisible();
    await expect(page.getByTitle('photo.png', { exact: true })).toBeVisible();
    await dismiss(page, picked);
    await menu(page, 'File', 'Open Recent', 'hero.aseprite');
    await expect(notice(page, recent)).toBeVisible();
    await expect(page.getByTitle('photo.png', { exact: true })).toBeVisible();
    await dismiss(page, recent);

    await pickInEditor(page, 'villain.aseprite');
    await waitForEditor(page, 'pixel');
    await pickInEditor(page, 'hero.aseprite');
    await expect(notice(page, picked)).toBeVisible();
    await expect(page.getByTitle('villain.aseprite', { exact: true })).toBeVisible();
    await dismiss(page, picked);
    await menu(page, 'File', 'Open Recent', 'hero.aseprite');
    await expect(notice(page, recent)).toBeVisible();
    await expect(page.getByTitle('villain.aseprite', { exact: true })).toBeVisible();
    await dismiss(page, recent);
  });
});

test.describe('clipboard sizing', () => {
  test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

  // Holds every clipboard read until `releaseClipboard()`; `prefillReady`
  // counts the decoded images, after which a prefill would have landed.
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const w = window as unknown as {
        releaseClipboard: () => void;
        heldReads: () => number;
        prefillReady: number;
      };
      const held: (() => void)[] = [];
      const read = navigator.clipboard.read.bind(navigator.clipboard);
      navigator.clipboard.read = () => new Promise<void>((r) => held.push(r)).then(read);
      w.releaseClipboard = () => held.splice(0).forEach((r) => r());
      w.heldReads = () => held.length;
      w.prefillReady = 0;
      const decode = window.createImageBitmap.bind(window);
      window.createImageBitmap = ((...args: Parameters<typeof createImageBitmap>) =>
        decode(...args).finally(() => (w.prefillReady += 1))) as typeof createImageBitmap;
    });
  });

  /** Releases the held clipboard reads and waits until their prefill is due. */
  async function releaseClipboard(page: Page): Promise<void> {
    const ready = () =>
      page.evaluate(() => (window as unknown as { prefillReady: number }).prefillReady);
    // The app starts its read asynchronously after the card opens: wait
    // until it is held, or the release would come first and free nothing.
    await expect
      .poll(() =>
        page.evaluate(() => (window as unknown as { heldReads: () => number }).heldReads()),
      )
      .toBeGreaterThan(0);
    const before = await ready();
    await page.evaluate(() =>
      (window as unknown as { releaseClipboard: () => void }).releaseClipboard(),
    );
    await expect.poll(ready).toBeGreaterThan(before);
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => r(null))));
  }

  /** Puts a `width` × `height` PNG on the clipboard (test-side canvas). */
  async function copyImage(page: Page, width: number, height: number): Promise<void> {
    await page.evaluate(
      async ([w, h]) => {
        const canvas = new OffscreenCanvas(w, h);
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('no 2d context');
        ctx.fillRect(0, 0, w, h);
        const blob = await canvas.convertToBlob({ type: 'image/png' });
        await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
      },
      [width, height] as const,
    );
  }

  test('AC-19: a size typed before the clipboard read finishes is kept', async ({ page }) => {
    await gotoStart(page);
    await copyImage(page, 123, 45);
    const head = page.getByRole('button', { name: 'New image' });
    const card = page.getByRole('form', { name: 'New image' });

    // Untouched, the card takes the clipboard size once the read finishes.
    await head.click();
    await expect(card.getByLabel('Width')).toHaveValue('800');
    await releaseClipboard(page);
    await expect(card.getByLabel('Width')).toHaveValue('123');
    await expect(card.getByLabel('Height')).toHaveValue('45');

    // A width typed while the read is pending stays, and so does the height.
    await head.click();
    await head.click();
    await card.getByLabel('Width').fill('300');
    await releaseClipboard(page);
    await expect(card.getByLabel('Width')).toHaveValue('300');
    await expect(card.getByLabel('Height')).toHaveValue('600');
    await card.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');

    // File ▸ New…: a height typed while the read is pending.
    await menu(page, 'File', 'New…');
    const dialog = page.getByRole('dialog', { name: 'New document' });
    await dialog.getByLabel('Height').fill('500');
    await releaseClipboard(page);
    await expect(dialog.getByLabel('Width')).toHaveValue('800');
    await expect(dialog.getByLabel('Height')).toHaveValue('500');
  });
});

test.describe('without File System Access', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
      delete w.showSaveFilePicker;
      delete w.showOpenFilePicker;
    });
  });

  test('AC-03: a cancelled file chooser changes nothing and shows no error', async ({ page }) => {
    await gotoStart(page);
    const open = page.getByRole('button', { name: 'Open…' });
    for (let i = 0; i < 2; i += 1) {
      const [chooser] = await Promise.all([page.waitForEvent('filechooser'), open.click()]);
      await chooser.element().evaluate((input) => input.dispatchEvent(new Event('cancel')));
      // Settled: Open… is enabled again and raises a new chooser next round.
      await expect(open).toBeEnabled();
      await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
      await expect(page.getByRole('alert')).toHaveCount(0);
    }
  });

  test('AC-04: pixel: Save downloads the sprite under its name and it reads as saved', async ({
    page,
  }) => {
    const download = async (name: string) => {
      const [d] = await Promise.all([
        page.waitForEvent('download'),
        menu(page, 'File', 'Save (download)'),
      ]);
      expect(d.suggestedFilename()).toBe(name);
      expect(isAseprite(await readFile(await d.path()))).toBe(true);
      await expect(unsaved(page, name)).toHaveCount(0);
    };

    // A new sprite.
    await createDocument(page, 'pixel');
    await edit(page, 'pixel');
    await expect(unsaved(page, 'pincel.aseprite')).toBeVisible();
    await download('pincel.aseprite');

    // An opened sprite keeps its name; an opened PNG saves as a sibling .aseprite.
    for (const [name, buffer, saved] of [
      ['hero.aseprite', await sprite(), 'hero.aseprite'],
      ['shot.png', await rasterFile(page, 'png'), 'shot.aseprite'],
    ] as const) {
      await gotoStart(page);
      const [chooser] = await Promise.all([
        page.waitForEvent('filechooser'),
        page.getByRole('button', { name: 'Open…' }).click(),
      ]);
      await chooser.setFiles({ name, mimeType: '', buffer });
      if (name.endsWith('.png')) {
        // A PNG from the start screen opens in Image mode; Pixel's own Open… keeps it.
        await waitForEditor(page, 'image');
        await createDocument(page, 'pixel');
        const [again] = await Promise.all([
          page.waitForEvent('filechooser'),
          menu(page, 'File', 'Open…'),
        ]);
        await again.setFiles({ name, mimeType: 'image/png', buffer });
        await expect(sizeShown(page, 'pixel', [8, 6])).toBeVisible();
      } else {
        await waitForEditor(page, 'pixel');
      }
      await edit(page, 'pixel');
      await expect(unsaved(page, saved)).toBeVisible();
      await download(saved);
    }
  });

  test('AC-12: Open Recent is absent in both modes', async ({ page }) => {
    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode);
      await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
      const file = page.getByRole('menu', { name: 'File', exact: true });
      await expect(file.getByRole('menuitem', { name: 'Open…', exact: true })).toBeVisible();
      await expect(file.getByRole('menuitem', { name: 'Open Recent' })).toHaveCount(0);
      await page.keyboard.press('Escape');
    }
  });

  test('AC-08: pixel: Save reads "Save (download)" and there is no Save As…', async ({ page }) => {
    await createDocument(page, 'pixel');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
    const file = page.getByRole('menu', { name: 'File', exact: true });
    await expect(
      file.getByRole('menuitem', { name: 'Save (download)', exact: true }),
    ).toBeVisible();
    await expect(file.getByRole('menuitem', { name: 'Save', exact: true })).toHaveCount(0);
    await expect(file.getByRole('menuitem', { name: 'Save As…', exact: true })).toHaveCount(0);
  });
});

/** A bridge call the stubbed desktop host saw (`stubTauri`). */
interface TauriCall {
  cmd: string;
  args: { path?: string; options?: { defaultPath?: string; filters?: unknown } };
}

/**
 * Stands in for the desktop app's Tauri bridge with a disk of `files` keyed
 * by path: `plugin:dialog|open` / `|save` answer `fakeTauri.pick` /
 * `.savePick` (`null` is a cancelled dialog), `read_file_bytes` and
 * `write_file_bytes` use the disk; every other command resolves `null`.
 */
async function stubTauri(page: Page, files: Record<string, Buffer>): Promise<void> {
  const bytes = Object.fromEntries(Object.entries(files).map(([p, b]) => [p, Array.from(b)]));
  await page.addInitScript((all: Record<string, number[]>) => {
    const tauri = {
      pick: null as string | null,
      savePick: null as string | null,
      calls: [] as unknown[],
      files: all,
    };
    let callback = 0;
    Object.assign(window, {
      fakeTauri: tauri,
      __TAURI_INTERNALS__: {
        transformCallback: () => (callback += 1),
        unregisterCallback: () => {},
        convertFileSrc: (p: string) => p,
        metadata: {
          currentWindow: { label: 'main' },
          currentWebview: { windowLabel: 'main', label: 'main' },
        },
        invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
          const path = typeof args.path === 'string' ? args.path : '';
          tauri.calls.push({
            cmd,
            args: cmd === 'write_file_bytes' ? { path } : JSON.parse(JSON.stringify(args)),
          });
          switch (cmd) {
            case 'plugin:dialog|open':
              return tauri.pick;
            case 'plugin:dialog|save':
              return tauri.savePick;
            case 'read_file_bytes': {
              const file = tauri.files[path];
              if (!file) throw new Error(`${path}: not found`);
              return file;
            }
            case 'write_file_bytes':
              tauri.files[path] = args.bytes as number[];
              return null;
            default:
              return null;
          }
        },
      },
    });
  }, bytes);
}

/** Sets the answers of the stubbed desktop dialogs (`stubTauri`). */
async function tauriDialogs(
  page: Page,
  answers: { pick?: string | null; savePick?: string | null },
) {
  await page.evaluate((a) => {
    Object.assign((window as unknown as { fakeTauri: object }).fakeTauri, a);
  }, answers);
}

/** The stubbed bridge calls named `cmd` so far. */
async function tauriCalls(page: Page, cmd: string): Promise<TauriCall[]> {
  return page.evaluate(
    (c) =>
      (window as unknown as { fakeTauri: { calls: TauriCall[] } }).fakeTauri.calls.filter(
        (call) => call.cmd === c,
      ),
    cmd,
  );
}

/** The start screen of the stubbed desktop app, past its first-launch advisory. */
async function gotoDesktopStart(page: Page): Promise<void> {
  await page.goto('./');
  const advisory = page.getByRole('button', { name: 'Got it' });
  await advisory.click();
  await expect(advisory).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
}

test.describe('desktop app (Tauri bridge stubbed)', () => {
  for (const mode of ['pixel', 'image'] as const) {
    test(`AC-05: ${mode}: Save writes back to the opened path, Save As asks for a location`, async ({
      page,
    }) => {
      await gotoStart(page);
      const [path, buffer] =
        mode === 'pixel'
          ? ['/home/me/hero.aseprite', await sprite()]
          : ['/home/me/photo.png', await rasterFile(page, 'png')];
      await stubTauri(page, { [path]: buffer });
      await gotoDesktopStart(page);
      await tauriDialogs(page, { pick: path });
      await page.getByRole('button', { name: 'Open…' }).click();
      await waitForEditor(page, mode);
      expect(await tauriCalls(page, 'read_file_bytes')).toEqual([
        { cmd: 'read_file_bytes', args: { path } },
      ]);

      await edit(page, mode);
      await menu(page, 'File', 'Save');
      await expect
        .poll(async () => (await tauriCalls(page, 'write_file_bytes')).map((c) => c.args.path))
        .toEqual([path]);
      expect(await tauriCalls(page, 'plugin:dialog|save')).toHaveLength(0);

      await menu(page, 'File', 'Save As…');
      if (mode === 'image') {
        await page
          .getByRole('dialog', { name: 'Save As' })
          .getByRole('button', { name: 'Save' })
          .click();
      }
      await expect.poll(async () => (await tauriCalls(page, 'plugin:dialog|save')).length).toBe(1);
      expect(await tauriCalls(page, 'write_file_bytes')).toHaveLength(1);
    });
  }

  test('AC-10: files opened and saved by path are recorded with path and mode', async ({
    page,
  }) => {
    await gotoStart(page);
    const [hero, photo] = ['/home/me/hero.aseprite', '/home/me/photo.png'];
    await stubTauri(page, { [hero]: await sprite(), [photo]: await rasterFile(page, 'png') });
    const rows = async () =>
      (await recentRows(page)).map((r) => `${r.name}/${r.mode}/${r.path ?? 'no path'}`);

    await gotoDesktopStart(page);
    await tauriDialogs(page, { pick: hero });
    await page.getByRole('button', { name: 'Open…' }).click();
    await waitForEditor(page, 'pixel');
    await expect.poll(rows).toEqual([`hero.aseprite/pixel/${hero}`]);
    const [opened] = await recentRows(page);
    await edit(page, 'pixel');
    await menu(page, 'File', 'Save');
    await expect
      .poll(async () => (await recentRows(page))[0]?.openedAt)
      .toBeGreaterThan(opened?.openedAt ?? Infinity);
    expect(await rows()).toEqual([`hero.aseprite/pixel/${hero}`]);

    await gotoDesktopStart(page);
    await tauriDialogs(page, { pick: photo });
    await page.getByRole('button', { name: 'Open…' }).click();
    await waitForEditor(page, 'image');
    await expect.poll(rows).toEqual([`photo.png/image/${photo}`, `hero.aseprite/pixel/${hero}`]);
  });
});

test('AC-08: pixel: with File System Access, Save and Save As… are listed', async ({ page }) => {
  await createDocument(page, 'pixel');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
  const file = page.getByRole('menu', { name: 'File', exact: true });
  await expect(file.getByRole('menuitem', { name: 'Save', exact: true })).toBeVisible();
  await expect(file.getByRole('menuitem', { name: 'Save As…', exact: true })).toBeVisible();
  await expect(file.getByRole('menuitem', { name: 'Save (download)', exact: true })).toHaveCount(0);
});
