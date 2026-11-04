import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  putClipboardImage,
  waitForEditor,
} from './helpers';

// Document tabs (docs/specs/document-tabs.md): several documents of one
// mode open side by side. Same-mode documents open as tabs; another mode's
// document replaces the session after the unsaved-changes prompt. The
// desktop paths (file paths, close, desktop keys) are in
// `document-tabs-desktop.spec.ts`.
//
// The open picker is removed (Open… falls back to the `<input>` chooser)
// and the save picker is a stub whose answers each test queues: write,
// cancel, or hold until the test releases it.

type PickerMode = 'write' | 'cancel' | 'hold';

interface PickerStub {
  /** Answers for the next picker calls; `write` once the queue is empty. */
  queue: PickerMode[];
  calls: number;
  written: string[];
  release: (() => void) | null;
}

type StubWindow = Window & {
  __picker: PickerStub;
  __pincelDiagnostics?: { openDocuments: Partial<Record<'pixel' | 'image', () => number>> };
};

test.beforeEach(async ({ page }) => {
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
        createWritable: async () => ({
          write: async () => {
            if (mode === 'hold') await new Promise<void>((resolve) => (stub.release = resolve));
          },
          close: async () => {
            stub.written.push(name);
          },
        }),
      };
    };
  });
});

/** A valid 1×1 PNG. */
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
  'base64',
);

const SPRITE = readFile(
  join(
    import.meta.dirname,
    '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite',
  ),
);

const DEFAULT_NAME = { pixel: 'pincel.aseprite', image: 'image.ora' } as const;
const EXT = { pixel: 'aseprite', image: 'png' } as const;

interface DropFile {
  name: string;
  bytes: Buffer;
}

/** A same-mode file named `stem` for `mode`. */
async function fileOf(mode: 'pixel' | 'image', stem: string): Promise<DropFile> {
  return { name: `${stem}.${EXT[mode]}`, bytes: mode === 'pixel' ? await SPRITE : PNG_1X1 };
}

/** The other mode's file named `stem`. */
async function otherFile(mode: 'pixel' | 'image', stem: string): Promise<DropFile> {
  return fileOf(mode === 'pixel' ? 'image' : 'pixel', stem);
}

/** Drops `files` onto the editor's canvas area, in this order. */
async function dropFiles(page: Page, files: DropFile[]): Promise<void> {
  await page.evaluate(
    (list) => {
      const dt = new DataTransfer();
      for (const f of list) dt.items.add(new File([new Uint8Array(f.data)], f.name));
      const target = document.querySelector('canvas') ?? document.body;
      for (const type of ['dragenter', 'dragover', 'drop']) {
        target.dispatchEvent(
          new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true }),
        );
      }
    },
    files.map((f) => ({ name: f.name, data: Array.from(f.bytes) })),
  );
}

function tabBar(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' });
}

function tabs(page: Page): Locator {
  return tabBar(page).getByRole('tab');
}

function tab(page: Page, name: string): Locator {
  return tabBar(page).getByRole('tab', { name: new RegExp(`^${name.replace('.', '\\.')}( |$)`) });
}

function prompt(page: Page): Locator {
  return page.getByRole('alertdialog', { name: 'Unsaved changes' });
}

/** The tab names in bar order. */
async function tabNames(page: Page): Promise<string[]> {
  return tabs(page).evaluateAll((els) =>
    els.map((el) => el.querySelector('span')?.textContent?.trim() ?? ''),
  );
}

async function expectActive(page: Page, name: string): Promise<void> {
  await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true');
  // The top bar names the active document (its tooltip adds the unsaved state).
  const title = new RegExp(`^${name.replace('.', '\\.')}( — unsaved changes)?$`);
  await expect(page.getByRole('banner').getByTitle(title)).toBeVisible();
}

async function expectDirty(page: Page, name: string, dirty: boolean): Promise<void> {
  await expect(tab(page, name).getByText('unsaved changes')).toHaveCount(dirty ? 1 : 0);
}

function canvas(page: Page, mode: 'pixel' | 'image'): Locator {
  return mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
}

/** Paints one stroke into the active tab. */
async function paint(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(page, canvas(page, mode), [0.3, 0.5], [0.7, 0.5]);
}

/** Opens same-mode tabs `a`, `b`, … by drop, after the created first one. */
async function openTabs(page: Page, mode: 'pixel' | 'image', stems: string[]): Promise<void> {
  for (const stem of stems) {
    await dropFiles(page, [await fileOf(mode, stem)]);
    await expectActive(page, `${stem}.${mode === 'pixel' ? 'aseprite' : 'png'}`);
  }
}

async function picker(page: Page): Promise<{ calls: number; written: string[] }> {
  return page.evaluate(() => {
    const { calls, written } = (window as unknown as StubWindow).__picker;
    return { calls, written: [...written] };
  });
}

async function queuePicker(page: Page, answers: PickerMode[]): Promise<void> {
  await page.evaluate((a) => {
    (window as unknown as StubWindow).__picker.queue = a;
  }, answers);
}

async function releasePicker(page: Page): Promise<void> {
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
  await page.evaluate(() => {
    const stub = (window as unknown as StubWindow).__picker;
    stub.release?.();
    stub.release = null;
  });
}

async function openDocuments(page: Page, mode: 'pixel' | 'image'): Promise<number | null> {
  return page.evaluate(
    (m) => (window as unknown as StubWindow).__pincelDiagnostics?.openDocuments[m]?.() ?? null,
    mode,
  );
}

function zoomLabel(page: Page): Locator {
  return page.getByRole('group', { name: 'Zoom' }).getByRole('button', { name: 'Fit to window' });
}

for (const mode of ['pixel', 'image'] as const) {
  const name = (stem: string) => `${stem}.${EXT[mode]}`;

  test(`${mode}: same-mode documents open as tabs after the last one, without a prompt (AC-01, AC-02)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await expect(tabs(page)).toHaveCount(1);
    await expectActive(page, DEFAULT_NAME[mode]);
    await paint(page, mode);
    await expectDirty(page, DEFAULT_NAME[mode], true);

    // A drop, and File ▸ New… in the current mode.
    await openTabs(page, mode, ['a']);
    await menu(page, 'File', 'New…');
    await page
      .getByRole('dialog', { name: 'New document' })
      .getByRole('button', { name: 'Create' })
      .click();
    await expect(tabs(page)).toHaveCount(3);
    await expect(tabs(page).nth(2)).toHaveAttribute('aria-selected', 'true');
    await expect(prompt(page)).toHaveCount(0);
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('a'), DEFAULT_NAME[mode]]);
    // The first tab kept its unsaved changes; only one tab is selected.
    await expectDirty(page, DEFAULT_NAME[mode], true);
    await expectDirty(page, name('a'), false);
    await expect(tabBar(page).getByRole('tab', { selected: true })).toHaveCount(1);
    // Each tab has a close button.
    await expect(tabBar(page).getByRole('button', { name: /^Close / })).toHaveCount(3);
  });

  test(`${mode}: tabs switch by click and by keys, each with its own document, view and history (AC-04, AC-05, AC-06)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await openTabs(page, mode, ['a', 'b']);

    // Tab `a`: an edit, a zoom step, a selection, the Eraser.
    await tab(page, name('a')).click();
    await expectActive(page, name('a'));
    await paint(page, mode);
    await expectDirty(page, name('a'), true);
    // Zoom out: a 1 × 1 image is fitted at the largest zoom already.
    const fitted = await zoomLabel(page).textContent();
    await page.keyboard.press('-');
    await expect(zoomLabel(page)).not.toHaveText(fitted ?? '');
    const zoomA = await zoomLabel(page).textContent();
    await page.keyboard.press('Control+a');
    await page.keyboard.press('e');
    const eraser = page.getByRole('button', { name: /^Eraser/ }).first();
    await expect(eraser).toHaveAttribute('aria-pressed', 'true');

    // Alt+Page Down / Up step through the tabs and wrap around; the window
    // title follows.
    await expect(page).toHaveTitle(`● ${name('a')} – Pincel`);
    await page.keyboard.press('Alt+PageDown');
    await expectActive(page, name('b'));
    await expect(page).toHaveTitle(`${name('b')} – Pincel`);
    await page.keyboard.press('Alt+PageDown');
    await expectActive(page, DEFAULT_NAME[mode]);
    await page.keyboard.press('Alt+PageUp');
    await expectActive(page, name('b'));

    // Tab `b`: its own (clean) history and view; tool shared.
    await expectDirty(page, name('b'), false);
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
    expect(await zoomLabel(page).textContent()).not.toBe(zoomA);
    await expect(eraser).toHaveAttribute('aria-pressed', 'true');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Select', exact: true }).click();
    await expect(
      page.getByRole('menu', { name: 'Select' }).getByRole('menuitem', { name: 'Deselect' }),
    ).toBeDisabled();
    await page.keyboard.press('Escape');

    // An edit and its undo in `b` leave `a` alone (AC-06).
    await page.keyboard.press('b');
    await paint(page, mode);
    await expectDirty(page, name('b'), true);
    await page.keyboard.press('Control+z');
    await expectDirty(page, name('b'), false);
    await expectDirty(page, name('a'), true);

    // Back in `a`: zoom, selection and history as they were.
    await tab(page, name('a')).click();
    await expectActive(page, name('a'));
    await expect(zoomLabel(page)).toHaveText(zoomA ?? '');
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Select', exact: true }).click();
    await expect(
      page.getByRole('menu', { name: 'Select' }).getByRole('menuitem', { name: 'Deselect' }),
    ).toBeEnabled();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
  });

  test(`${mode}: closing a clean tab activates its right-hand neighbour; the last one shows the start screen (AC-07, AC-09)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await openTabs(page, mode, ['a', 'b', 'c']);
    // Close button on the active tab `a`: its right-hand neighbour follows.
    await tab(page, name('a')).click();
    await tabBar(page)
      .getByRole('button', { name: `Close ${name('a')}` })
      .click();
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('b'), name('c')]);
    await expectActive(page, name('b'));
    // Middle-click on the last tab: its left neighbour becomes active.
    await tab(page, name('c')).click();
    await tab(page, name('c')).click({ button: 'middle' });
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('b')]);
    await expectActive(page, name('b'));
    // File ▸ Close, then the close shortcut.
    await menu(page, 'File', 'Close');
    await expectActive(page, DEFAULT_NAME[mode]);
    await expect(prompt(page)).toHaveCount(0);
    await page.keyboard.press('Alt+w');
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
    await expect(tabBar(page)).toHaveCount(0);
  });

  test(`${mode}: closing a tab with unsaved changes asks about that document (AC-08)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await openTabs(page, mode, ['a']);
    await tab(page, DEFAULT_NAME[mode]).click();
    await paint(page, mode);
    await tab(page, name('a')).click();
    const close = tabBar(page).getByRole('button', { name: `Close ${DEFAULT_NAME[mode]}` });

    // Cancel keeps it; while the prompt is open, the tab is active.
    await close.click();
    await expect(prompt(page)).toContainText(`Save changes to "${DEFAULT_NAME[mode]}"?`);
    await expectActive(page, DEFAULT_NAME[mode]);
    await prompt(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(tabs(page)).toHaveCount(2);
    await expectDirty(page, DEFAULT_NAME[mode], true);

    // A cancelled save keeps it, unsaved.
    await queuePicker(page, ['cancel']);
    await close.click();
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    await expect.poll(async () => (await picker(page)).calls).toBe(1);
    await expect(tabs(page)).toHaveCount(2);
    await expectDirty(page, DEFAULT_NAME[mode], true);

    // Save closes it once the save succeeded.
    await close.click();
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    await expect(tabs(page)).toHaveCount(1);
    expect((await picker(page)).written).toHaveLength(1);
    await expectActive(page, name('a'));

    // Don't Save closes without saving.
    await paint(page, mode);
    await tabBar(page)
      .getByRole('button', { name: `Close ${name('a')}` })
      .click();
    await prompt(page).getByRole('button', { name: "Don't Save" }).click();
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
    expect((await picker(page)).written).toHaveLength(1);
  });

  test(`${mode}: one unsaved tab is asked about before another mode replaces the session (AC-10, AC-12)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await paint(page, mode);
    await openTabs(page, mode, ['a']);
    // An unreadable file shows its error and leaves every tab open.
    await dropFiles(page, [{ name: 'notes.txt', bytes: Buffer.from('hello') }]);
    await expect(page.getByText('notes.txt: not a supported file')).toBeVisible();
    await expect(prompt(page)).toHaveCount(0);
    await expect(tabs(page)).toHaveCount(2);

    const other = await otherFile(mode, 'other');
    await dropFiles(page, [other]);
    await expect(prompt(page)).toContainText(`Save changes to "${DEFAULT_NAME[mode]}"?`);
    await expectActive(page, DEFAULT_NAME[mode]);
    await prompt(page).getByRole('button', { name: "Don't Save" }).click();
    const next = mode === 'pixel' ? 'image' : 'pixel';
    await waitForEditor(page, next);
    expect(await tabNames(page)).toEqual([other.name]);
    await expectActive(page, other.name);
  });

  test(`${mode}: several unsaved tabs are listed in one prompt — Save All, Don't Save, Cancel (AC-11)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await paint(page, mode);
    await openTabs(page, mode, ['a', 'b']);
    await tab(page, name('a')).click();
    await paint(page, mode);
    await tab(page, name('b')).click();
    const other = await otherFile(mode, 'other');

    // Cancel (Escape) changes nothing.
    await dropFiles(page, [other]);
    await expect(prompt(page)).toContainText('Unsaved changes in 2 images');
    await expect(prompt(page).getByRole('listitem')).toHaveText([DEFAULT_NAME[mode], name('a')]);
    const saveAll = prompt(page).getByRole('button', { name: 'Save All' });
    await expect(saveAll).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(prompt(page)).toHaveCount(0);
    await expect(tabs(page)).toHaveCount(3);
    await expectDirty(page, DEFAULT_NAME[mode], true);
    await expectDirty(page, name('a'), true);

    // Save All with the second save cancelled: the first stays saved, the
    // second unsaved, the action is aborted.
    await queuePicker(page, ['write', 'cancel']);
    await dropFiles(page, [other]);
    await saveAll.click();
    await expect.poll(async () => (await picker(page)).calls).toBe(2);
    await expect(prompt(page)).toHaveCount(0);
    await expect(tabs(page)).toHaveCount(3);
    await expectDirty(page, DEFAULT_NAME[mode], false);
    await expectDirty(page, name('a'), true);
    // The second tab was active while it saved.
    await expectActive(page, name('a'));
    expect((await picker(page)).written).toEqual([DEFAULT_NAME[mode]]);

    // Save All once more saves the rest in tab order and continues.
    await paint(page, mode);
    await tab(page, DEFAULT_NAME[mode]).click();
    await paint(page, mode);
    await dropFiles(page, [other]);
    await expect(prompt(page)).toContainText('Unsaved changes in 2 images');
    await saveAll.click();
    const next = mode === 'pixel' ? 'image' : 'pixel';
    await waitForEditor(page, next);
    expect(await tabNames(page)).toEqual([other.name]);
    expect((await picker(page)).written).toEqual([
      DEFAULT_NAME[mode],
      DEFAULT_NAME[mode],
      name('a'),
    ]);
  });

  test(`${mode}: Don't Save in the list prompt discards every unsaved tab (AC-11, AC-12)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await paint(page, mode);
    await openTabs(page, mode, ['a']);
    await paint(page, mode);
    const other = await otherFile(mode, 'other');
    await dropFiles(page, [other]);
    await prompt(page).getByRole('button', { name: "Don't Save" }).click();
    await waitForEditor(page, mode === 'pixel' ? 'image' : 'pixel');
    expect(await tabNames(page)).toEqual([other.name]);
    expect((await picker(page)).calls).toBe(0);
  });

  test(`${mode}: closing tabs and leaving the editor frees their documents (AC-16)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    expect(await openDocuments(page, mode)).toBe(1);
    await openTabs(page, mode, ['a', 'b']);
    expect(await openDocuments(page, mode)).toBe(3);
    await page.keyboard.press('Alt+w');
    await expect(tabs(page)).toHaveCount(2);
    expect(await openDocuments(page, mode)).toBe(2);
    await openTabs(page, mode, ['c']);
    expect(await openDocuments(page, mode)).toBe(3);
    // Replacing the session releases them all.
    await dropFiles(page, [await otherFile(mode, 'other')]);
    await waitForEditor(page, mode === 'pixel' ? 'image' : 'pixel');
    expect(await openDocuments(page, mode)).toBe(0);
  });

  test(`${mode}: a running save keeps its tab open and lets other tabs be switched to (AC-17)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await paint(page, mode);
    await openTabs(page, mode, ['a']);
    await tab(page, DEFAULT_NAME[mode]).click();
    await queuePicker(page, ['hold']);
    await page.keyboard.press('Control+s');
    await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
    // Closing the saving tab does nothing, by any means.
    await tabBar(page)
      .getByRole('button', { name: `Close ${DEFAULT_NAME[mode]}` })
      .click();
    await page.keyboard.press('Alt+w');
    await expect(tabs(page)).toHaveCount(2);
    await expect(prompt(page)).toHaveCount(0);
    // Another tab can be switched to.
    await tab(page, name('a')).click();
    await expectActive(page, name('a'));
    // Replacing the session waits for the save, then goes ahead (nothing
    // is unsaved once it finished).
    const other = await otherFile(mode, 'other');
    await dropFiles(page, [other]);
    await page.waitForTimeout(300);
    await expect(tabs(page)).toHaveCount(2);
    await releasePicker(page);
    await waitForEditor(page, mode === 'pixel' ? 'image' : 'pixel');
    expect(await tabNames(page)).toEqual([other.name]);
    expect((await picker(page)).written).toEqual([DEFAULT_NAME[mode]]);
  });

  test(`${mode}: dropping several files opens this mode's as tabs and skips the others (AC-19)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await dropFiles(page, [
      await fileOf(mode, 'a'),
      await otherFile(mode, 'x'),
      await fileOf(mode, 'b'),
    ]);
    await expectActive(page, name('b'));
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('a'), name('b')]);
    await expect(
      page.getByText('1 dropped file(s) of the other mode were not opened'),
    ).toBeVisible();
    await expect(prompt(page)).toHaveCount(0);

    // Only the other mode's files: the first one replaces the session.
    const x = await otherFile(mode, 'x');
    const y = await otherFile(mode, 'y');
    await dropFiles(page, [x, y]);
    await waitForEditor(page, mode === 'pixel' ? 'image' : 'pixel');
    expect(await tabNames(page)).toEqual([x.name]);
    await expect(
      page.getByText('1 dropped file(s) of the other mode were not opened'),
    ).toBeVisible();
  });
}

test('pixel: each tab keeps its current layer, frame and onion skin (AC-05)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openTabs(page, 'pixel', ['a']);
  await tab(page, 'pincel.aseprite').click();
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();
  await timeline.getByLabel('Onion skin').check();
  await page.getByRole('button', { name: 'Add layer' }).click();
  const activeLayer = page.locator('li.layer-active');
  await expect(activeLayer).toContainText('Layer 2');

  await tab(page, 'a.aseprite').click();
  await expectActive(page, 'a.aseprite');
  await expect(timeline.getByLabel('Onion skin')).not.toBeChecked();
  await expect(timeline.getByRole('button', { name: 'Frame 1', exact: true })).toHaveAttribute(
    'aria-current',
    'true',
  );

  await tab(page, 'pincel.aseprite').click();
  await expect(timeline.getByLabel('Onion skin')).toBeChecked();
  await expect(timeline.getByRole('button', { name: 'Frame 2', exact: true })).toHaveAttribute(
    'aria-current',
    'true',
  );
  await expect(activeLayer).toContainText('Layer 2');
});

test('the leave-page warning counts an unsaved tab in the background (AC-13)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await paint(page, 'image');
  await openTabs(page, 'image', ['a']);
  await expectDirty(page, 'a.png', false);
  const asked = new Promise<string>((resolve) => {
    page.once('dialog', (d) => {
      resolve(d.type());
      void d.dismiss();
    });
  });
  await page.close({ runBeforeUnload: true });
  expect(await asked).toBe('beforeunload');
});

test('each unsaved tab is autosaved and recovered into its own tab (AC-14, AC-15)', async ({
  page,
}) => {
  await page.clock.install();
  await createDocument(page, 'image', { width: 16, height: 16 });
  await paint(page, 'image');
  await openTabs(page, 'image', ['a', 'b']);
  await paint(page, 'image');
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  // Saving one tab removes only its snapshot; discarding a closed tab's too.
  await page.keyboard.press('Control+s');
  await expectDirty(page, 'b.png', false);
  await tab(page, 'a.png').click();
  await paint(page, 'image');
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  await page.keyboard.press('Alt+w');
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect(tabs(page)).toHaveCount(2);
  await page.waitForTimeout(300);

  page.on('dialog', (d) => void d.accept());
  await page.reload();
  await gotoStart(page);
  await createDocument(page, 'image', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await expect(dialog).toBeVisible();
  // Only the first tab's snapshot is left.
  await expect(dialog.getByRole('button', { name: 'Recover' })).toHaveCount(1);
  await dialog.getByRole('button', { name: 'Recover' }).click();
  await expect(dialog).toHaveCount(0);
  expect(await tabNames(page)).toEqual(['image.ora', 'image.ora']);
  await expect(tabs(page).nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(tabs(page).nth(1).getByText('unsaved changes')).toHaveCount(1);
});

test('the recovery dialog stays open while snapshots remain; each Recover opens a tab (AC-15)', async ({
  page,
}) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await paint(page, 'pixel');
  await openTabs(page, 'pixel', ['a']);
  await paint(page, 'pixel');
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  page.on('dialog', (d) => void d.accept());
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await expect(dialog.getByRole('button', { name: 'Recover' })).toHaveCount(2);
  await dialog.getByRole('button', { name: 'Recover' }).first().click();
  await expect(dialog.getByRole('button', { name: 'Recover' })).toHaveCount(1);
  await expect(tabs(page)).toHaveCount(2);
  await dialog.getByRole('button', { name: 'Recover' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(tabs(page)).toHaveCount(3);
  for (const i of [1, 2])
    await expect(tabs(page).nth(i).getByText('unsaved changes')).toHaveCount(1);
});

for (const mode of ['pixel', 'image'] as const)
  test(`${mode}: the tab bar scrolls and keeps the active tab in view at every width (AC-01)`, async ({
    page,
  }) => {
    for (const width of [1280, 800, 390]) {
      await page.setViewportSize({ width, height: 800 });
      await createDocument(page, mode, { width: 16, height: 16 });
      await openTabs(page, mode, [
        'alpha-long-name',
        'bravo-long-name',
        'charlie-long-name',
        'delta-long-name',
      ]);
      await openTabs(page, mode, ['echo-long-name', 'foxtrot-long-name', 'golf-long-name']);
      const bar = tabBar(page);
      await expect(bar).toBeVisible();
      const active = tab(page, `golf-long-name.${EXT[mode]}`);
      await expect(active).toBeInViewport();
      if (width < 1280) {
        expect(await bar.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
      }
      await page.keyboard.press('Alt+PageDown');
      await expect(tab(page, DEFAULT_NAME[mode])).toBeInViewport();
    }
  });

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('the tab bar, the list prompt and File ▸ Close speak German (AC-18)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Neues Bild' }).click();
    await page
      .getByRole('form', { name: 'Neues Bild' })
      .getByRole('button', { name: 'Erstellen' })
      .click();
    await waitForEditor(page, 'image');
    await paint(page, 'image');
    await dropFiles(page, [await fileOf('image', 'a')]);
    await paint(page, 'image');
    const bar = page.getByRole('tablist', { name: 'Geöffnete Dokumente' });
    await expect(bar.getByRole('button', { name: '„a.png“ schließen' })).toHaveAttribute(
      'title',
      /^Schließen/,
    );
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Datei', exact: true }).click();
    await expect(
      page.getByRole('menu', { name: 'Datei' }).getByRole('menuitem', { name: 'Schließen' }),
    ).toBeVisible();
    await page.keyboard.press('Escape');
    await dropFiles(page, [await fileOf('pixel', 'x')]);
    const dialog = page.getByRole('alertdialog', { name: 'Ungespeicherte Änderungen' });
    await expect(dialog).toContainText('Ungespeicherte Änderungen in 2 Bildern');
    await expect(dialog.getByRole('button', { name: 'Alle speichern' })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Nicht speichern' })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeVisible();
  });
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: opening the file of an open tab activates that tab and keeps its changes (AC-03)`, async ({
    page,
  }) => {
    // A File System Access picker whose handles know their entry.
    const file = await fileOf(mode, 'same');
    await page.addInitScript(
      ({ name, data }) => {
        const handle = {
          kind: 'file',
          name,
          getFile: async () => new File([new Uint8Array(data)], name),
          isSameEntry: async (other: { name?: string }) => other.name === name,
          queryPermission: async () => 'granted',
          requestPermission: async () => 'granted',
        };
        Object.assign(window, { showOpenFilePicker: async () => [handle] });
      },
      { name: file.name, data: Array.from(file.bytes) },
    );
    await createDocument(page, mode, { width: 16, height: 16 });
    await menu(page, 'File', 'Open…');
    await expectActive(page, file.name);
    await paint(page, mode);
    await tab(page, DEFAULT_NAME[mode]).click();
    await menu(page, 'File', 'Open…');
    await expectActive(page, file.name);
    await expect(tabs(page)).toHaveCount(2);
    await expectDirty(page, file.name, true);
    await expect(prompt(page)).toHaveCount(0);
  });
}

/** The pan transform of the active tab's canvas stage. */
async function panTransform(page: Page, mode: 'pixel' | 'image'): Promise<string> {
  return canvas(page, mode).evaluate((el) => {
    for (let at: HTMLElement | null = el as HTMLElement; at; at = at.parentElement) {
      if (at.style.transform.includes('translate')) return at.style.transform;
    }
    return '';
  });
}

/** The brush size the options bar shows. */
function brushSize(page: Page, mode: 'pixel' | 'image'): Locator {
  return mode === 'pixel'
    ? page.getByLabel('Brush size', { exact: true })
    : page.getByRole('toolbar', { name: 'Tool options' }).getByRole('slider').first();
}

for (const mode of ['pixel', 'image'] as const) {
  const name = (stem: string) => `${stem}.${EXT[mode]}`;

  test(`${mode}: the canvas, status bar and palette show the active tab's document (AC-04)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await paint(page, mode);
    const footer = page.locator('footer');
    const size = mode === 'pixel' ? '16×16' : '16 × 16 · sRGB';
    await expect(footer).toContainText(size);
    const swatches = page.getByRole('group', { name: 'Palette colors' }).getByRole('button');
    const paletteFirst = mode === 'pixel' ? await swatches.count() : 0;
    // Keep focus away from the zoom control: its ring is in the screenshot.
    await page.mouse.move(0, 0);
    const first = await canvas(page, mode).screenshot();

    await openTabs(page, mode, ['a']);
    await expect(footer).not.toContainText(size);
    await page.mouse.move(0, 0);
    expect(await canvas(page, mode).screenshot()).not.toEqual(first);
    if (mode === 'pixel') {
      // The fixture brings its own palette.
      expect(await swatches.count()).not.toBe(paletteFirst);
    }

    await tab(page, DEFAULT_NAME[mode]).click();
    await expect(footer).toContainText(size);
    if (mode === 'pixel') expect(await swatches.count()).toBe(paletteFirst);
    await page.mouse.move(0, 0);
    await expect.poll(async () => (await canvas(page, mode).screenshot()).equals(first)).toBe(true);
    await expectActive(page, DEFAULT_NAME[mode]);
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('a')]);
  });

  test(`${mode}: each tab keeps its scroll position; tool options and colours are shared (AC-05)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 64, height: 64 });
    await openTabs(page, mode, ['a']);
    await tab(page, DEFAULT_NAME[mode]).click();
    // Scroll the first tab with Space + drag.
    await page.keyboard.down('Space');
    await drag(page, canvas(page, mode), [0.5, 0.5], [0.8, 0.7]);
    await page.keyboard.up('Space');
    const scrolled = await panTransform(page, mode);
    // A brush option and the colours.
    await page.keyboard.press('b');
    await page.keyboard.press(']');
    await page.keyboard.press(']');
    const sizeValue = await brushSize(page, mode).inputValue();
    await page.keyboard.press('x');
    const hex = page.getByLabel('Foreground color hex value');
    const fg = await hex.inputValue();

    await tab(page, name('a')).click();
    await expectActive(page, name('a'));
    expect(await panTransform(page, mode)).not.toBe(scrolled);
    await expect(brushSize(page, mode)).toHaveValue(sizeValue);
    await expect(hex).toHaveValue(fg);

    await tab(page, DEFAULT_NAME[mode]).click();
    expect(await panTransform(page, mode)).toBe(scrolled);
    await expect(brushSize(page, mode)).toHaveValue(sizeValue);
    await expect(hex).toHaveValue(fg);
  });
}

test.describe('clipboard', () => {
  test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

  test('image: Paste as New Image opens a tab after the unsaved one, without a prompt (AC-02)', async ({
    page,
  }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await paint(page, 'image');
    await putClipboardImage(page, 24, 12, '#00ff00');
    await page.keyboard.press('ControlOrMeta+Alt+v');
    await expectActive(page, 'Pasted image.png');
    expect(await tabNames(page)).toEqual(['image.ora', 'Pasted image.png']);
    await expect(page.locator('footer')).toContainText('24 × 12 · sRGB');
    await expectDirty(page, 'image.ora', true);
    await expect(prompt(page)).toHaveCount(0);
    await tab(page, 'image.ora').click();
    await expect(page.locator('footer')).toContainText('16 × 16 · sRGB');
  });
});
