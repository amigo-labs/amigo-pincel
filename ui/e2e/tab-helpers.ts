import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, type Locator, type Page } from '@playwright/test';
import { drag, imageCanvas, pixelCanvas } from './helpers';

// Shared helpers of the document-tabs specs (docs/specs/document-tabs.md):
// a stubbed save picker, files to drop, and locators for the tab bar.
//
// The open picker is removed (Open… falls back to the `<input>` chooser)
// and the save picker is a stub whose answers each test queues: write,
// cancel, fail, hold until the test releases it, or hold and then fail.

export type Mode = 'pixel' | 'image';

type PickerMode = 'write' | 'cancel' | 'fail' | 'hold' | 'holdFail';

interface PickerStub {
  /** Answers for the next picker calls; `write` once the queue is empty. */
  queue: PickerMode[];
  calls: number;
  written: string[];
  release: (() => void) | null;
}

type StubWindow = Window & {
  __picker: PickerStub;
  __pincelDiagnostics?: { openDocuments: Partial<Record<Mode, () => number>> };
};

/** Installs the save-picker stub before the app's scripts run. */
export async function installPicker(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const stub: PickerStub = { queue: [], calls: 0, written: [], release: null };
    const w = window as unknown as StubWindow & {
      showOpenFilePicker?: unknown;
      showSaveFilePicker?: unknown;
    };
    w.__picker = stub;
    delete w.showOpenFilePicker;
    w.showSaveFilePicker = async (opts: { suggestedName?: string }) => {
      stub.calls += 1;
      const mode = stub.queue.shift() ?? 'write';
      if (mode === 'cancel') throw new DOMException('The user aborted a request.', 'AbortError');
      const name = opts.suggestedName ?? 'file';
      return {
        kind: 'file',
        name,
        queryPermission: async () => 'granted',
        requestPermission: async () => 'granted',
        isSameEntry: async () => false,
        createWritable: async () => {
          if (mode === 'fail') throw new Error('disk full');
          return {
            write: async () => {
              if (mode === 'hold' || mode === 'holdFail') {
                await new Promise<void>((resolve) => (stub.release = resolve));
              }
              if (mode === 'holdFail') throw new Error('disk full');
            },
            close: async () => {
              stub.written.push(name);
            },
          };
        },
      };
    };
  });
}

/** A valid 1×1 PNG. */
export const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
  'base64',
);

const SPRITE = readFile(
  join(
    import.meta.dirname,
    '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite',
  ),
);

export const DEFAULT_NAME = { pixel: 'pincel.aseprite', image: 'image.ora' } as const;
export const EXT = { pixel: 'aseprite', image: 'png' } as const;

export interface DropFile {
  name: string;
  bytes: Buffer;
}

/** A file of `mode` named `stem`. */
export async function fileOf(mode: Mode, stem: string): Promise<DropFile> {
  return { name: `${stem}.${EXT[mode]}`, bytes: mode === 'pixel' ? await SPRITE : PNG_1X1 };
}

/** The other mode. */
export function other(mode: Mode): Mode {
  return mode === 'pixel' ? 'image' : 'pixel';
}

/** The other mode's file named `stem`. */
export async function otherFile(mode: Mode, stem: string): Promise<DropFile> {
  return fileOf(other(mode), stem);
}

const RASTER = /\.(png|jpe?g|webp|bmp|gif|tiff?)$/i;
const PROJECT = /\.(aseprite|ase|ora|psd)$/i;

/**
 * Drops `files` onto the shown editor, in this order, to open them. A drop
 * of raster images onto an open editor asks first whether to add them as
 * layers (file-access AC-20); this answers "Open as document", which opens
 * each as a tab (file-access AC-23, document-tabs AC-19).
 */
export async function dropFiles(page: Page, files: DropFile[]): Promise<void> {
  const asks =
    (await tabs(page).count()) > 0 &&
    files.some((f) => RASTER.test(f.name)) &&
    !files.some((f) => PROJECT.test(f.name));
  await dispatchDrop(page, files);
  if (!asks) return;
  const dialog = page.getByRole('dialog', { name: 'Dropped images' });
  await dialog.getByRole('button', { name: 'Open as document' }).click();
  await expect(dialog).toBeHidden();
}

/** Dispatches a drop of `files` onto the shown editor, answering nothing. */
export async function dispatchDrop(page: Page, files: DropFile[]): Promise<void> {
  await page.evaluate(
    (list) => {
      const dt = new DataTransfer();
      for (const f of list) dt.items.add(new File([new Uint8Array(f.data)], f.name));
      const target =
        document.querySelector('[data-editor]:not([inert]) canvas') ??
        document.querySelector('canvas') ??
        document.body;
      for (const type of ['dragenter', 'dragover', 'drop']) {
        target.dispatchEvent(
          new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true }),
        );
      }
    },
    files.map((f) => ({ name: f.name, data: Array.from(f.bytes) })),
  );
}

/** The shown editor (the active tab's); the other mode's stays hidden. */
export function shownEditor(page: Page): Locator {
  return page.locator('[data-editor]:not([inert])');
}

/** The mounted editor of `mode`, shown or hidden. */
export function editorOf(page: Page, mode: Mode): Locator {
  return page.locator(`[data-editor="${mode}"]`);
}

export function tabBar(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' });
}

export function tabs(page: Page): Locator {
  return tabBar(page).getByRole('tab');
}

export function tab(page: Page, name: string): Locator {
  return tabBar(page).getByRole('tab', { name: new RegExp(`^${name.replace('.', '\\.')}( |$)`) });
}

export function prompt(page: Page): Locator {
  return page.getByRole('alertdialog', { name: 'Unsaved changes' });
}

/** The tab names in bar order. */
export async function tabNames(page: Page): Promise<string[]> {
  return tabs(page).evaluateAll((els) =>
    els.map((el) => el.querySelector('span')?.textContent?.trim() ?? ''),
  );
}

export async function expectActive(page: Page, name: string): Promise<void> {
  await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true');
  // The top bar names the active document (its tooltip adds the unsaved state).
  const title = new RegExp(`^${name.replace('.', '\\.')}( — unsaved changes)?$`);
  await expect(page.getByRole('banner').getByTitle(title)).toBeVisible();
}

export async function expectDirty(page: Page, name: string, dirty: boolean): Promise<void> {
  await expect(tab(page, name).getByText('unsaved changes')).toHaveCount(dirty ? 1 : 0);
}

export function canvas(page: Page, mode: Mode): Locator {
  return mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
}

/** Paints one stroke into the active tab, which is of `mode`. */
export async function paint(page: Page, mode: Mode): Promise<void> {
  if (mode === 'pixel') await shownEditor(page).getByTitle('Pencil (B)').click();
  await drag(page, canvas(page, mode), [0.3, 0.5], [0.7, 0.5]);
}

/** Opens tabs of `mode` named `stem`, … by drop. */
export async function openTabs(page: Page, mode: Mode, stems: string[]): Promise<void> {
  for (const stem of stems) {
    await dropFiles(page, [await fileOf(mode, stem)]);
    await expectActive(page, `${stem}.${EXT[mode]}`);
  }
}

export async function picker(page: Page): Promise<{ calls: number; written: string[] }> {
  return page.evaluate(() => {
    const { calls, written } = (window as unknown as StubWindow).__picker;
    return { calls, written: [...written] };
  });
}

export async function queuePicker(page: Page, answers: PickerMode[]): Promise<void> {
  await page.evaluate((a) => {
    (window as unknown as StubWindow).__picker.queue = a;
  }, answers);
}

/** Waits for a held picker write, then lets it go on. */
export async function releasePicker(page: Page): Promise<void> {
  await waitForHeldWrite(page);
  await page.evaluate(() => {
    const stub = (window as unknown as StubWindow).__picker;
    stub.release?.();
    stub.release = null;
  });
}

/** Waits until a picker write is held (`hold`, `holdFail`). */
export async function waitForHeldWrite(page: Page): Promise<void> {
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
}

/** The open-document count `mode`'s core publishes (AC-16); `null` before
 *  its core was ever loaded in this page. */
export async function openDocuments(page: Page, mode: Mode): Promise<number | null> {
  return page.evaluate(
    (m) => (window as unknown as StubWindow).__pincelDiagnostics?.openDocuments[m]?.() ?? null,
    mode,
  );
}

export function zoomLabel(page: Page): Locator {
  return page.getByRole('group', { name: 'Zoom' }).getByRole('button', { name: 'Fit to window' });
}
