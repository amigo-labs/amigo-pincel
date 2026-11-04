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

// The unsaved-changes prompt (docs/specs/unsaved-changes-prompt.md): every
// action that would replace the open document asks Save / Don't Save /
// Cancel while the document has unsaved changes. Desktop close and the
// OS hand-over are covered with a mocked Tauri bridge in
// `unsaved-close.spec.ts`.
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

const NAME = { pixel: 'pincel.aseprite', image: 'image.ora' } as const;

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

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: the prompt names the document and offers Save, Don't Save, Cancel (AC-02)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await menu(page, 'File', 'New…');
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
    await expect(newDialog(page)).toHaveCount(0);

    // Escape is Cancel (AC-06): nothing ran, the document is untouched.
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(newDialog(page)).toHaveCount(0);
    await expect(marker(page, mode)).toBeVisible();
  });

  test(`${mode}: Cancel keeps the document, its history and its marker (AC-06)`, async ({ page }) => {
    await dirtyDocument(page, mode);
    await menu(page, 'File', 'New…');
    await prompt(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(prompt(page)).toHaveCount(0);
    await expect(newDialog(page)).toHaveCount(0);
    await expect(marker(page, mode)).toBeVisible();
    // One undo step back reaches the saved state: the history is intact.
    await page.keyboard.press('Control+z');
    await expect(page.getByTitle(NAME[mode], { exact: true })).toBeVisible();
  });

  test(`${mode}: Save saves like File ▸ Save, then runs the action (AC-03)`, async ({ page }) => {
    await dirtyDocument(page, mode);
    await menu(page, 'File', 'New…');
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    // A never-saved document asks where to save, as File ▸ Save does.
    await expect(newDialog(page)).toBeVisible();
    expect(await picker(page)).toEqual({ calls: 1, written: 1 });
    await newDialog(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByTitle(NAME[mode], { exact: true })).toBeVisible();
    await expect(marker(page, mode)).toHaveCount(0);
  });

  test(`${mode}: a cancelled save from the prompt keeps the document and skips the action (AC-04)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await setPicker(page, 'cancel');
    await menu(page, 'File', 'New…');
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    await expect.poll(async () => (await picker(page)).calls).toBe(1);
    await expect(prompt(page)).toHaveCount(0);
    await expect(newDialog(page)).toHaveCount(0);
    await expect(marker(page, mode)).toBeVisible();
    await expect(page.getByText(/failed/i)).toHaveCount(0);
  });

  test(`${mode}: a failed save from the prompt shows File ▸ Save's error and skips the action (AC-04)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await setPicker(page, 'fail');
    await menu(page, 'File', 'New…');
    await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
    await expect(page.getByText(/Save failed: .*disk full/)).toBeVisible();
    await expect(newDialog(page)).toHaveCount(0);
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

  test(`${mode}: Don't Save runs the action; cancelling the New dialog keeps the document (AC-05)`, async ({
    page,
  }) => {
    await dirtyDocument(page, mode);
    await menu(page, 'File', 'New…');
    await prompt(page).getByRole('button', { name: "Don't Save" }).click();
    await expect(newDialog(page)).toBeVisible();
    expect(await picker(page)).toEqual({ calls: 0, written: 0 });
    await newDialog(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(marker(page, mode)).toBeVisible();
  });
}

test('keys pressed while the prompt is open reach neither editor nor menus (AC-02)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  const before = await pixelCanvas(page).screenshot();
  const choosers: string[] = [];
  page.on('filechooser', () => choosers.push('open'));
  await menu(page, 'File', 'New…');
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
  await expect(newDialog(page)).toHaveCount(0);
  await expect(marker(page, 'pixel')).toBeVisible();
  expect(await pixelCanvas(page).screenshot()).toEqual(before);
});

test("Enter on Don't Save runs the pending action (AC-02, AC-05)", async ({ page }) => {
  await dirtyDocument(page, 'image');
  await menu(page, 'File', 'New…');
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expect(newDialog(page)).toBeVisible();
});

test('Save counts as done once the download is handed over, without file-system access (AC-03)', async ({
  page,
}) => {
  await dirtyDocument(page, 'pixel');
  await page.evaluate(() => {
    delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
  });
  await menu(page, 'File', 'New…');
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    prompt(page).getByRole('button', { name: 'Save', exact: true }).click(),
  ]);
  expect(download.suggestedFilename()).toBe('pincel.aseprite');
  await expect(newDialog(page)).toBeVisible();
});

test('File ▸ Open… asks before the picker opens (AC-01)', async ({ page }) => {
  for (const mode of ['image', 'pixel'] as const) {
    await dirtyDocument(page, mode);
    const choosers: string[] = [];
    page.on('filechooser', () => choosers.push('open'));
    await menu(page, 'File', 'Open…');
    await expect(prompt(page)).toBeVisible();
    await page.waitForTimeout(200);
    expect(choosers).toEqual([]);
    const [chooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      prompt(page).getByRole('button', { name: "Don't Save" }).click(),
    ]);
    await chooser.setFiles({ name: 'dot.png', mimeType: 'image/png', buffer: PNG_1X1 });
    if (mode === 'image') {
      await waitForEditor(page, 'image');
      await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
    } else {
      // Pixel mode opens a PNG as a sprite in place.
      await expect(page.getByTitle('dot.aseprite', { exact: true })).toBeVisible();
    }
  }
});

test('dropping a file onto an open editor asks first (AC-01)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  await dropFile(page, 'dot.png', 'image/png', PNG_1X1);
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
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

test('a file picked after Don\'t Save that cannot be opened leaves the document as it was (AC-11)', async ({
  page,
}) => {
  await dirtyDocument(page, 'image');
  await menu(page, 'File', 'Open…');
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    prompt(page).getByRole('button', { name: "Don't Save" }).click(),
  ]);
  await chooser.setFiles({ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') });
  await expect(page.getByText('notes.txt: not a supported file')).toBeVisible();
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

  await dropFile(page, 'dot.png', 'image/png', PNG_1X1);
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
});

test('a second action while the prompt is open is ignored (AC-08)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  await menu(page, 'File', 'New…');
  await expect(prompt(page)).toBeVisible();
  await dropFile(page, 'dot.png', 'image/png', PNG_1X1);
  await expect(prompt(page)).toHaveCount(1);
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  // The prompt answered the first action, not the drop.
  await expect(newDialog(page)).toBeVisible();
  await expect(page.getByTitle('dot.png', { exact: true })).toHaveCount(0);
});

test('a second action while the prompt\'s save runs is ignored (AC-08)', async ({ page }) => {
  await dirtyDocument(page, 'pixel');
  await setPicker(page, 'hold');
  await menu(page, 'File', 'New…');
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
  await dropFile(page, 'dot.png', 'image/png', PNG_1X1);
  await page.waitForTimeout(200);
  await expect(prompt(page)).toHaveCount(0);
  await releasePicker(page);
  await expect(newDialog(page)).toBeVisible();
  await expect(page.getByTitle('dot.png', { exact: true })).toHaveCount(0);
});

test('an action while a File ▸ Save runs is ignored (AC-08)', async ({ page }) => {
  await dirtyDocument(page, 'image');
  await setPicker(page, 'hold');
  await page.keyboard.press('Control+s');
  await page.waitForFunction(() => (window as unknown as StubWindow).__picker.release !== null);
  await dropFile(page, 'dot.png', 'image/png', PNG_1X1);
  await page.waitForTimeout(200);
  await expect(prompt(page)).toHaveCount(0);
  await releasePicker(page);
  await expect(page.getByTitle('image.ora', { exact: true })).toBeVisible();
  await expect(page.getByTitle('dot.png', { exact: true })).toHaveCount(0);
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
    await page.keyboard.press('Control+o');
    const dialog = page.getByRole('alertdialog', { name: 'Ungespeicherte Änderungen' });
    await expect(dialog).toContainText('Änderungen an „image.ora“ speichern?');
    await expect(dialog.getByRole('button', { name: 'Speichern', exact: true })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Nicht speichern' })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeVisible();
  });
});
