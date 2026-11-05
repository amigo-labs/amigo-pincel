import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  clearClipboard,
  createDocument,
  drag,
  imageCanvas,
  menu,
  pixelCanvas,
  waitForEditor,
} from './helpers';

// The unsaved-changes prompt (docs/specs/unsaved-changes-prompt.md): it
// asks Save / Don't Save / Cancel while a document has unsaved changes.
// Since document tabs of both modes (docs/specs/document-tabs.md) opening
// a document never replaces another — every open action adds a tab — so
// the action the prompt guards is closing: here closing the tab (File ▸
// Close), which shows the start screen once the only tab is gone. Desktop
// close and the OS hand-over are covered with a mocked Tauri bridge in
// `unsaved-close.spec.ts`; several unsaved tabs in
// `document-tabs-desktop.spec.ts`.
//
// The open picker is removed (Open… falls back to the `<input>` chooser
// Playwright can drive) and the save picker is a stub whose answer each
// test sets: write, cancel, fail, or hold until the test releases it.

type PickerMode = 'write' | 'cancel' | 'fail' | 'hold';

interface PickerStub {
  mode: PickerMode;
  calls: number;
  written: number;
  release: (() => void) | null;
}

type StubWindow = Window & { __picker: PickerStub };

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const stub: PickerStub = { mode: 'write', calls: 0, written: 0, release: null };
    const w = window as unknown as StubWindow & {
      showOpenFilePicker?: unknown;
      showSaveFilePicker?: unknown;
    };
    w.__picker = stub;
    delete w.showOpenFilePicker;
    w.showSaveFilePicker = async (opts: { suggestedName?: string }) => {
      stub.calls += 1;
      if (stub.mode === 'cancel') throw new DOMException('The user aborted a request.', 'AbortError');
      return {
        kind: 'file',
        name: opts.suggestedName ?? 'file',
        queryPermission: async () => 'granted',
        requestPermission: async () => 'granted',
        createWritable: async () => {
          if (stub.mode === 'fail') throw new Error('disk full');
          return {
            write: async () => {
              if (stub.mode === 'hold') await new Promise<void>((resolve) => (stub.release = resolve));
            },
            close: async () => {
              stub.written += 1;
            },
          };
        },
      };
    };
  });
});

/** A valid 1×1 PNG. */
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
  'base64',
);

/** A valid 1×1 GIF: an Image-mode format Pixel mode's Open hands over. */
const GIF_1X1 = Buffer.from('R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7', 'base64');

const NAME = { pixel: 'pincel.aseprite', image: 'image.ora' } as const;

const SPRITE = readFile(
  join(import.meta.dirname, '../../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite'),
);

/** The other mode's editor. */
function otherMode(mode: 'pixel' | 'image'): 'pixel' | 'image' {
  return mode === 'pixel' ? 'image' : 'pixel';
}

/** Drops a file of the other mode, named `stem` (`stem.png` from Pixel
 *  mode, `stem.aseprite` from Image mode). */
async function dropOther(page: Page, mode: 'pixel' | 'image', stem = 'other'): Promise<string> {
  if (mode === 'pixel') {
    await dropFile(page, `${stem}.png`, 'image/png', PNG_1X1);
    return `${stem}.png`;
  }
  await dropFile(page, `${stem}.aseprite`, 'application/octet-stream', await SPRITE);
  return `${stem}.aseprite`;
}

/** File ▸ Close on the only tab: the action the prompt guards. */
async function closeTab(page: Page): Promise<void> {
  await menu(page, 'File', 'Close');
}

/** The pending action ran: the only tab closed, the start screen shows. */
async function expectClosed(page: Page): Promise<void> {
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

/** The open tabs, in bar order. */
function openTabs(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
}

function prompt(page: Page): Locator {
  return page.getByRole('alertdialog', { name: 'Unsaved changes' });
}

function newDialog(page: Page): Locator {
  return page.getByRole('dialog', { name: 'New document' });
}

function marker(page: Page, mode: 'pixel' | 'image'): Locator {
  return page.getByTitle(`${NAME[mode]} — unsaved changes`);
}

async function setPicker(page: Page, mode: PickerMode): Promise<void> {
  await page.evaluate((m) => {
    (window as unknown as StubWindow).__picker.mode = m;
  }, mode);
}

async function picker(page: Page): Promise<{ calls: number; written: number }> {
  return page.evaluate(() => {
    const { calls, written } = (window as unknown as StubWindow).__picker;
    return { calls, written };
  });
}

async function releasePicker(page: Page): Promise<void> {
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
  await page.evaluate(() => {
    const stub = (window as unknown as StubWindow).__picker;
    stub.release?.();
    stub.release = null;
  });
}

/** Creates a document of `mode` and paints one stroke into it. */
async function dirtyDocument(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await createDocument(page, mode, { width: 16, height: 16 });
  if (mode === 'pixel') {
    await page.getByTitle('Pencil (B)').click();
    await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  } else {
    await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  }
  await expect(marker(page, mode)).toBeVisible();
}

/** Drops a file onto the editor's canvas area. */
async function dropFile(page: Page, name: string, mime: string, bytes: Buffer): Promise<void> {
  await page.evaluate(
    ({ name, mime, data }) => {
      const dt = new DataTransfer();
      dt.items.add(new File([new Uint8Array(data)], name, { type: mime }));
      const target = document.querySelector('canvas') ?? document.body;
      for (const type of ['dragenter', 'dragover', 'drop']) {
        target.dispatchEvent(new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true }));
      }
    },
    { name, mime, data: Array.from(bytes) },
  );
}

/** Drops a raster image onto the open editor and answers its drop dialog
 *  (file-access AC-20) with "Open as document" (AC-23). */
async function dropToOpen(page: Page, name: string, mime: string, bytes: Buffer): Promise<void> {
  await dropFile(page, name, mime, bytes);
  const dialog = page.getByRole('dialog', { name: 'Dropped images' });
  await dialog.getByRole('button', { name: 'Open as document' }).click();
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the prompt names the document and offers Save, Don't Save, Cancel (AC-02)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await closeTab(page);
    const dialog = prompt(page);
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(`Save changes to "${NAME[mode]}"?`);
    const save = dialog.getByRole('button', { name: 'Save', exact: true });
    const dontSave = dialog.getByRole('button', { name: "Don't Save" });
    const cancel = dialog.getByRole('button', { name: 'Cancel' });
    await expect(save).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(dontSave).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(cancel).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(save).toBeFocused();
    await page.keyboard.press('Shift+Tab');
    await expect(cancel).toBeFocused();

    // Escape is Cancel (AC-06): nothing ran, the document is untouched.
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await page.waitForTimeout(200);
    await waitForEditor(page, mode);
    await expect(marker(page, mode)).toBeVisible();
  });

  test(`${mode}: Cancel keeps the document, its history and its marker (AC-06)`, async ({ page }) => {
    await dirtyDocument(page, mode);
    await closeTab(page);
    await prompt(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(prompt(page)).toHaveCount(0);
    await waitForEditor(page, mode);
    await expect(marker(page, mode)).toBeVisible();
    // One undo step back reaches the saved state: the history is intact.
    await page.keyboard.press('Control+z');
    await expect(page.getByTitle(NAME[mode], { exact: true })).toBeVisible();
  });

  test(`${mode}: Save saves like File ▸ Save, then runs the action (AC-03)`, async ({ page }) => {
    await dirtyDocument(page, mode);
    await closeTab(page);
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    // A never-saved document asks where to save, as File ▸ Save does.
    await expectClosed(page);
    expect(await picker(page)).toEqual({ calls: 1, written: 1 });
  });

  test(`${mode}: a cancelled save from the prompt keeps the document and skips the action (AC-04)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await setPicker(page, 'cancel');
    await closeTab(page);
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    await expect.poll(async () => (await picker(page)).calls).toBe(1);
    await expect(prompt(page)).toHaveCount(0);
    await page.waitForTimeout(200);
    await waitForEditor(page, mode);
    await expect(marker(page, mode)).toBeVisible();
    await expect(page.getByText(/failed/i)).toHaveCount(0);
  });

  test(`${mode}: a failed save from the prompt shows File ▸ Save's error and skips the action (AC-04)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await setPicker(page, 'fail');
    await closeTab(page);
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    await expect(page.getByText(/Save failed: .*disk full/)).toBeVisible();
    await waitForEditor(page, mode);
    await expect(marker(page, mode)).toBeVisible();
  });

  test(`${mode}: dismissing the Save As picker reports no failure (AC-12)`, async ({ page }) => {
    await dirtyDocument(page, mode);
    await setPicker(page, 'cancel');
    for (const [i, keys] of ['Control+s', 'Control+Shift+s'].entries()) {
      await page.keyboard.press(keys);
      // Image mode's Save As asks for the format before the picker.
      if (mode === 'image' && i === 1) {
        await page.getByRole('dialog').getByRole('button', { name: 'Save', exact: true }).click();
      }
      await expect.poll(async () => (await picker(page)).calls).toBe(i + 1);
      await page.waitForTimeout(200);
      await expect(page.getByText(/failed|saved \d+ bytes/i)).toHaveCount(0);
      await expect(marker(page, mode)).toBeVisible();
    }
    expect((await picker(page)).calls).toBe(2);
  });

  test(`${mode}: Don't Save runs the action without saving (AC-05)`, async ({ page }) => {
    await dirtyDocument(page, mode);
    await closeTab(page);
    await prompt(page).getByRole('button', { name: "Don't Save" }).click();
    await expectClosed(page);
    expect(await picker(page)).toEqual({ calls: 0, written: 0 });
  });

  test(`${mode}: File ▸ New… of the other mode opens a tab without a prompt (AC-01)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await menu(page, 'File', 'New…');
    await expect(newDialog(page)).toBeVisible();
    await expect(prompt(page)).toHaveCount(0);
    await newDialog(page)
      .getByRole('radio', { name: mode === 'pixel' ? /^Image/ : /^Pixel sprite/ })
      .click();
    await newDialog(page).getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, otherMode(mode));
    await expect(openTabs(page)).toHaveCount(2);
    await expect(prompt(page)).toHaveCount(0);
    await expect(openTabs(page).first().getByText('unsaved changes')).toHaveCount(1);
    expect(await picker(page)).toEqual({ calls: 0, written: 0 });
  });
}

test('keys pressed while the prompt is open reach neither editor nor menus (AC-02)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  const before = await pixelCanvas(page).screenshot();
  const choosers: string[] = [];
  page.on('filechooser', () => choosers.push('open'));
  await closeTab(page);
  await expect(prompt(page)).toBeVisible();
  // Ctrl is ⌘ on macOS (shortcuts.ts `mod`): the Meta chords are blocked too.
  const keys = ['b', 'e', 'm', 'Control+z', 'Control+s', 'Control+o', 'Meta+z', 'Meta+s', 'Meta+o'];
  for (const key of [...keys, 'Delete', ' ']) {
    await page.keyboard.press(key);
  }
  await page.waitForTimeout(200);
  await expect(prompt(page)).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(choosers).toEqual([]);
  expect(await picker(page)).toEqual({ calls: 0, written: 0 });
  await expect(page.getByTitle('Pencil (B)')).toHaveAttribute('aria-pressed', 'true');

  // Enter activates the focused button: Cancel.
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Enter');
  await expect(prompt(page)).toHaveCount(0);
  await expect(marker(page, 'pixel')).toBeVisible();
  expect(await pixelCanvas(page).screenshot()).toEqual(before);
});

test("Enter on Don't Save runs the pending action (AC-02, AC-05)", async ({ page }) => {
  await dirtyDocument(page, 'image');
  await closeTab(page);
  await expect(prompt(page)).toBeVisible();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expectClosed(page);
});

test('Save counts as done once the download is handed over, without file-system access (AC-03)', async ({
  page,
}) => {
  await dirtyDocument(page, 'pixel');
  await page.evaluate(() => {
    delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
  });
  await closeTab(page);
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    prompt(page).getByRole('button', { name: 'Save', exact: true }).click(),
  ]);
  expect(download.suggestedFilename()).toBe('pincel.aseprite');
  await expectClosed(page);
});

test('File ▸ Open… opens a tab of either mode without a prompt (AC-01)', async ({ page }) => {
  for (const mode of ['image', 'pixel'] as const) {
    await dirtyDocument(page, mode);
    // Pixel mode's own Open opens a PNG as a sprite.
    const [chooser] = await Promise.all([page.waitForEvent('filechooser'), menu(page, 'File', 'Open…')]);
    await expect(prompt(page)).toHaveCount(0);
    await chooser.setFiles({ name: 'dot.png', mimeType: 'image/png', buffer: PNG_1X1 });
    await expect(page.getByTitle(mode === 'image' ? 'dot.png' : 'dot.aseprite', { exact: true })).toBeVisible();
    await expect(prompt(page)).toHaveCount(0);
    await expect(openTabs(page)).toHaveCount(2);
  }
  // From the Pixel tab, another mode's file: a tab of that mode, no prompt.
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), menu(page, 'File', 'Open…')]);
  await chooser.setFiles({ name: 'photo.gif', mimeType: 'image/gif', buffer: GIF_1X1 });
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('photo.gif', { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(openTabs(page)).toHaveCount(3);
  await expect(openTabs(page).first().getByText('unsaved changes')).toHaveCount(1);
});

test('dropping a file onto an open editor opens a tab without a prompt (AC-01)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  await dropToOpen(page, 'dot.png', 'image/png', PNG_1X1);
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(openTabs(page)).toHaveCount(2);
});

test('a dropped file that cannot be opened shows its error and no prompt (AC-11)', async ({ page }) => {
  await dirtyDocument(page, 'image');
  await dropFile(page, 'notes.txt', 'text/plain', Buffer.from('hello'));
  await expect(page.getByText('notes.txt: not a supported file')).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(marker(page, 'image')).toBeVisible();
});

test.describe('clipboard', () => {
  test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

  test('Paste as New Image with no image on the clipboard shows no prompt (AC-11)', async ({ page }) => {
    await dirtyDocument(page, 'pixel');
    await clearClipboard(page);
    await menu(page, 'Edit', 'Paste as New Image');
    await expect(page.getByText('The clipboard holds no image')).toBeVisible();
    await expect(prompt(page)).toHaveCount(0);
    await expect(marker(page, 'pixel')).toBeVisible();
  });
});

test('a picked file that cannot be opened shows its error and no prompt (AC-11)', async ({ page }) => {
  await dirtyDocument(page, 'image');
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), menu(page, 'File', 'Open…')]);
  await chooser.setFiles({ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') });
  await expect(page.getByText('notes.txt: not a supported file')).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(marker(page, 'image')).toBeVisible();
});

test('a clean document runs every action at once (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'File', 'New…');
  await expect(newDialog(page)).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await newDialog(page).getByRole('button', { name: 'Cancel' }).click();

  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), menu(page, 'File', 'Open…')]);
  await expect(prompt(page)).toHaveCount(0);
  await chooser.setFiles([]);

  await dropToOpen(page, 'dot.png', 'image/png', PNG_1X1);
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
});

test('a document dropped while the prompt is open is not opened (AC-08)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  await closeTab(page);
  await expect(prompt(page)).toBeVisible();
  await dropOther(page, 'pixel', 'second');
  await expect(
    page.getByText('second.png was not opened: an unsaved-changes prompt is open'),
  ).toBeVisible();
  await expect(prompt(page)).toHaveCount(1);
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  // The prompt answered the close, and the drop opened nothing.
  await expectClosed(page);
  await page.waitForTimeout(200);
  await expect(openTabs(page)).toHaveCount(0);
});

test("a document dropped while the prompt's save runs is not opened (AC-08)", async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  await setPicker(page, 'hold');
  await closeTab(page);
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
  await dropOther(page, 'pixel', 'second');
  await expect(
    page.getByText('second.png was not opened: an unsaved-changes prompt is open'),
  ).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await releasePicker(page);
  await expectClosed(page);
  await page.waitForTimeout(200);
  await expect(openTabs(page)).toHaveCount(0);
});

test('a document dropped while a File ▸ Save runs opens as a tab at once; the save finishes (AC-08, document-tabs AC-17)', async ({
  page,
}) => {
  await dirtyDocument(page, 'image');
  await setPicker(page, 'hold');
  await page.keyboard.press('Control+s');
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
  const other = await dropOther(page, 'image');
  await waitForEditor(page, 'pixel');
  await expect(page.getByTitle(other, { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await releasePicker(page);
  await expect(openTabs(page).first().getByText('unsaved changes')).toHaveCount(0);
  expect((await picker(page)).written).toBe(1);
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('the prompt speaks German (AC-13)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Neues Bild' }).click();
    const card = page.getByRole('form', { name: 'Neues Bild' });
    await card.getByRole('button', { name: 'Erstellen' }).click();
    await waitForEditor(page, 'image');
    await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
    await menu(page, 'Datei', 'Schließen');
    const dialog = page.getByRole('alertdialog', { name: 'Ungespeicherte Änderungen' });
    await expect(dialog).toContainText('Änderungen an „image.ora“ speichern?');
    await expect(dialog.getByRole('button', { name: 'Speichern', exact: true })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Nicht speichern' })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeVisible();
  });
});
