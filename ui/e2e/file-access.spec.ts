import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
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
} from './helpers';

// File access across hosts (docs/specs/file-access.md): the File System
// Access pickers, the <input type=file> / download fallbacks and the
// recent-files registry. The FSA pickers are faked (`fakeFileHandles`):
// headless Chromium cannot drive the real ones.

type Mode = 'pixel' | 'image';

/** A 32 × 24 sprite (the writer's compatibility fixture). */
const sprite = () =>
  readFile(
    join(import.meta.dirname, '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite'),
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
  await drag(page, mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
}

/** Opens `name` through the start screen's Open… and the fake picker. */
async function pickFromStart(page: Page, name: string): Promise<void> {
  await gotoStart(page);
  await fakeFs(page, { pick: name });
  await page.getByRole('button', { name: 'Open…' }).click();
}

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
        await page.getByRole('dialog', { name: 'Save As' }).getByRole('button', { name: 'Save' }).click();
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

  test('AC-08: pixel: Save reads "Save (download)" and there is no Save As…', async ({ page }) => {
    await createDocument(page, 'pixel');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
    const file = page.getByRole('menu', { name: 'File', exact: true });
    await expect(file.getByRole('menuitem', { name: 'Save (download)', exact: true })).toBeVisible();
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
async function tauriDialogs(page: Page, answers: { pick?: string | null; savePick?: string | null }) {
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
        await page.getByRole('dialog', { name: 'Save As' }).getByRole('button', { name: 'Save' }).click();
      }
      await expect.poll(async () => (await tauriCalls(page, 'plugin:dialog|save')).length).toBe(1);
      expect(await tauriCalls(page, 'write_file_bytes')).toHaveLength(1);
    });
  }
});

test('AC-08: pixel: with File System Access, Save and Save As… are listed', async ({ page }) => {
  await createDocument(page, 'pixel');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
  const file = page.getByRole('menu', { name: 'File', exact: true });
  await expect(file.getByRole('menuitem', { name: 'Save', exact: true })).toBeVisible();
  await expect(file.getByRole('menuitem', { name: 'Save As…', exact: true })).toBeVisible();
  await expect(file.getByRole('menuitem', { name: 'Save (download)', exact: true })).toHaveCount(0);
});
