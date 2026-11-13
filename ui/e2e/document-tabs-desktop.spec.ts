import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { drag, imageCanvas, menu, pixelCanvas, waitForEditor } from './helpers';
import {
  closeWindow,
  destroyed,
  diskPaths,
  emit,
  installTauriMock,
  putFile,
  releaseWrite,
  setMock,
} from './tauri-mock';

// Document tabs in the desktop app (docs/specs/document-tabs.md), against
// the mocked Tauri bridge of `tauri-mock.ts`: the same file by path
// (AC-03), closing the window with tabs of both modes (AC-10, AC-11,
// AC-13, AC-17, AC-18), the native menu (AC-21), OS hand-overs in order
// (AC-24), and the tab keys of the desktop app and the web build on each
// platform (AC-20). The platform is the user agent's.

const UA = {
  windows:
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
  linux:
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
} as const;

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

function prompt(page: Page): Locator {
  return page.getByRole('alertdialog', { name: 'Unsaved changes' });
}

function tabs(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
}

function tab(page: Page, name: string): Locator {
  return page
    .getByRole('tablist', { name: 'Open documents' })
    .getByRole('tab', { name: new RegExp(`^${name.replace('.', '\\.')}( |$)`) });
}

async function expectActive(page: Page, index: number): Promise<void> {
  await expect(tabs(page).nth(index)).toHaveAttribute('aria-selected', 'true');
}

/** Opens the app (past the first-launch note) with `mode`'s 16×16 document. */
async function start(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

async function paint(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  if (mode === 'pixel') {
    await page.locator('[data-editor]:not([inert])').getByTitle('Pencil (B)').click();
  }
  await drag(
    page,
    mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page),
    [0.3, 0.5],
    [0.7, 0.5],
  );
}

/** File ▸ New… in the current mode, or in `mode`: a new tab. */
async function newTab(page: Page, mode?: 'pixel' | 'image'): Promise<void> {
  const before = await tabs(page).count();
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  if (mode) {
    await dialog.getByRole('radio', { name: mode === 'image' ? /^Image/ : /^Pixel sprite/ }).click();
  }
  await dialog.getByRole('button', { name: 'Create' }).click();
  // The first tab of the other mode loads that editor (its module and
  // wasm) before the tab shows: wait on the editor's start-up budget first,
  // not on the tab count's short timeout.
  if (mode) await waitForEditor(page, mode);
  await expect(tabs(page)).toHaveCount(before + 1);
}

/** Hands `path` over as the OS does and waits until `name`'s tab is the
 *  active one. */
async function handOver(page: Page, path: string, name: string): Promise<void> {
  await emit(page, 'open-file', path);
  await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true');
}

/** The unsaved marker of the tab at `index`. */
function unsavedMark(page: Page, index: number): Locator {
  return tabs(page).nth(index).getByText('unsaved changes');
}

/** A keydown as macOS sends it: ⌥W types "∑", the physical key is W. */
async function pressOptionW(page: Page): Promise<void> {
  await page.evaluate(() => {
    window.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: '∑',
        code: 'KeyW',
        altKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
  });
}

test.describe('desktop app', () => {
  test.beforeEach(async ({ page }) => {
    page.on('dialog', (d) => void d.accept());
    await installTauriMock(page);
  });

  test.describe('on Windows', () => {
    test.use({ userAgent: UA.windows });

    test('the same path opens no second tab, whatever its case (AC-03)', async ({ page }) => {
      await start(page, 'image');
      for (const path of ['C:/docs/Photo.png', 'c:/DOCS/photo.PNG'])
        await putFile(page, path, PNG_1X1);
      await handOver(page, 'C:/docs/Photo.png', 'Photo.png');
      await paint(page, 'image');
      await tabs(page).first().click();
      // Handed over again (other case), then picked in File ▸ Open….
      await handOver(page, 'c:/DOCS/photo.PNG', 'Photo.png');
      await tabs(page).first().click();
      await setMock(page, { openPick: 'C:/docs/Photo.png' });
      await menu(page, 'File', 'Open…');
      await expect(tab(page, 'Photo.png')).toHaveAttribute('aria-selected', 'true');
      await expect(tabs(page)).toHaveCount(2);
      // Its unsaved changes are kept.
      await expect(page.getByTitle('Photo.png — unsaved changes')).toBeVisible();
      await expect(prompt(page)).toHaveCount(0);
    });

    for (const mode of ['pixel', 'image'] as const) {
      test(`${mode}: Open Recent of a closed file of this mode opens it as a tab, without a prompt (AC-02)`, async ({
        page,
      }) => {
        const ext = mode === 'pixel' ? 'aseprite' : 'ora';
        await start(page, mode);
        await paint(page, mode);
        // A second tab saved to a path (a recent), then closed.
        await newTab(page);
        await paint(page, mode);
        await setMock(page, { savePick: `/docs/second.${ext}` });
        await page.keyboard.press('Control+s');
        if (mode === 'image') {
          await expect(page.getByTitle(`second.${ext}`, { exact: true })).toBeVisible();
        }
        await expect(tab(page, `second.${ext}`)).toBeVisible();
        // The save runs on until the recent is recorded, and a tab with a
        // running save does not close (AC-17): press again until it does.
        await expect(async () => {
          await page.keyboard.press('Control+w');
          await expect(tabs(page)).toHaveCount(1, { timeout: 1_000 });
        }).toPass();
        // Pixel mode reopens it itself, Image mode through the shell.
        await page
          .getByRole('menubar')
          .getByRole('menuitem', { name: 'File', exact: true })
          .click();
        await page
          .getByRole('menu', { name: 'File', exact: true })
          .getByRole('menuitem', { name: 'Open Recent' })
          .click();
        await page
          .getByRole('menu', { name: 'Open Recent', exact: true })
          .getByRole('menuitem', { name: `second.${ext}`, exact: true })
          .first()
          .click();
        await expect(tab(page, `second.${ext}`)).toHaveAttribute('aria-selected', 'true');
        await expect(tabs(page)).toHaveCount(2);
        await expect(prompt(page)).toHaveCount(0);
        // The first tab kept its unsaved changes.
        await expect(tabs(page).first().getByText('unsaved changes')).toHaveCount(1);
      });
    }

    test('Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+W switch and close tabs; File ▸ Close names Ctrl+W (AC-04, AC-07, AC-20)', async ({
      page,
    }) => {
      await start(page, 'pixel');
      await newTab(page);
      await newTab(page);
      await expectActive(page, 2);
      await page.keyboard.press('Control+Tab');
      await expectActive(page, 0);
      await page.keyboard.press('Control+Shift+Tab');
      await expectActive(page, 2);
      await page.keyboard.press('Alt+PageUp');
      await expectActive(page, 1);
      await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
      await expect(
        page.getByRole('menu', { name: 'File' }).getByRole('menuitem', { name: /^Close/ }),
      ).toContainText('Ctrl+W');
      await page.keyboard.press('Escape');
      await page.keyboard.press('Control+w');
      await expect(tabs(page)).toHaveCount(2);
      await expectActive(page, 1);
      await page.keyboard.press('Alt+w');
      await expect(tabs(page)).toHaveCount(1);
    });

    test('closing the window with unsaved tabs of both modes lists them; Save All saves them in tab order (AC-11, AC-13)', async ({
      page,
    }) => {
      await start(page, 'image');
      await paint(page, 'image');
      await newTab(page, 'pixel');
      await paint(page, 'pixel');
      // The active tab is clean; the close still asks (AC-13).
      await newTab(page, 'image');
      await closeWindow(page);
      await expect(prompt(page)).toContainText('Unsaved changes in 2 images');
      await expect(prompt(page).getByRole('listitem')).toHaveText(['image.ora', 'pincel.aseprite']);
      const saveAll = prompt(page).getByRole('button', { name: 'Save All' });
      await expect(saveAll).toBeFocused();
      // Escape cancels: nothing changes.
      await page.keyboard.press('Escape');
      await expect(prompt(page)).toHaveCount(0);
      expect(await destroyed(page)).toBe(0);
      await expect(unsavedMark(page, 0)).toHaveCount(1);
      await expect(unsavedMark(page, 1)).toHaveCount(1);

      // Save All with the second save cancelled: the first stays saved, the
      // second unsaved and active (its mode's editor shown), the app open.
      await setMock(page, { saveQueue: ['/docs/first.ora', null] });
      await closeWindow(page);
      await saveAll.click();
      await expect(prompt(page)).toHaveCount(0);
      await expect(unsavedMark(page, 0)).toHaveCount(0);
      await expect(unsavedMark(page, 1)).toHaveCount(1);
      await expectActive(page, 1);
      await expect(pixelCanvas(page)).toBeVisible();
      expect(await destroyed(page)).toBe(0);
      expect(await diskPaths(page)).toEqual(['/docs/first.ora']);

      // Save All once more: the rest saves, then the app closes.
      await paint(page, 'pixel');
      await tabs(page).first().click();
      await paint(page, 'image');
      await setMock(page, { saveQueue: ['/docs/second.aseprite'] });
      await closeWindow(page);
      await expect(prompt(page)).toContainText('Unsaved changes in 2 images');
      await saveAll.click();
      await expect.poll(() => destroyed(page)).toBe(1);
      expect((await diskPaths(page)).sort()).toEqual(['/docs/first.ora', '/docs/second.aseprite']);
    });

    test('a failed save in Save All keeps the app open and the rest unsaved (AC-11)', async ({
      page,
    }) => {
      await start(page, 'pixel');
      await paint(page, 'pixel');
      await newTab(page, 'image');
      await paint(page, 'image');
      await setMock(page, { writeQueue: ['fail'] });
      await closeWindow(page);
      await prompt(page).getByRole('button', { name: 'Save All' }).click();
      await expect(prompt(page)).toHaveCount(0);
      await expect(page.getByText(/disk full/).first()).toBeVisible();
      await expect(unsavedMark(page, 0)).toHaveCount(1);
      await expect(unsavedMark(page, 1)).toHaveCount(1);
      expect(await destroyed(page)).toBe(0);
    });

    test("Don't Save in the list prompt closes the app and discards every unsaved tab (AC-11)", async ({
      page,
    }) => {
      await start(page, 'pixel');
      await paint(page, 'pixel');
      await newTab(page, 'image');
      await paint(page, 'image');
      await closeWindow(page);
      await prompt(page).getByRole('button', { name: "Don't Save" }).click();
      await expect.poll(() => destroyed(page)).toBe(1);
      expect(await diskPaths(page)).toEqual([]);
    });

    test('one unsaved tab of the hidden mode is asked about and shown before closing (AC-10)', async ({
      page,
    }) => {
      await start(page, 'image');
      await paint(page, 'image');
      await newTab(page, 'pixel');
      await closeWindow(page);
      await expect(prompt(page)).toContainText('Save changes to "image.ora"?');
      await expectActive(page, 0);
      await expect(imageCanvas(page)).toBeVisible();
      await prompt(page).getByRole('button', { name: "Don't Save" }).click();
      await expect.poll(() => destroyed(page)).toBe(1);
    });

    test('the native menu reaches only the active tab’s editor (AC-21)', async ({ page }) => {
      await start(page, 'image');
      await paint(page, 'image');
      await newTab(page, 'pixel');
      await paint(page, 'pixel');
      await tabs(page).first().click();
      await expectActive(page, 0);
      await emit(page, 'menu', 'menu:undo');
      await expect(unsavedMark(page, 0)).toHaveCount(0);
      await expect(unsavedMark(page, 1)).toHaveCount(1);
      await emit(page, 'menu', 'menu:close');
      await expect(tabs(page)).toHaveCount(1);
      await expect(tab(page, 'pincel.aseprite')).toHaveAttribute('aria-selected', 'true');
      await expect(unsavedMark(page, 0)).toHaveCount(1);
    });

    test('files handed over by the OS open in the order they arrive, also while a mode loads (AC-24)', async ({
      page,
    }) => {
      await start(page, 'image');
      await putFile(page, '/docs/a.aseprite', await SPRITE);
      await putFile(page, '/docs/b.png', PNG_1X1);
      await emit(page, 'open-file', '/docs/a.aseprite');
      await emit(page, 'open-file', '/docs/b.png');
      await expect(tab(page, 'b.png')).toHaveAttribute('aria-selected', 'true');
      expect(
        await tabs(page).evaluateAll((els) => els.map((el) => el.querySelector('span')?.textContent)),
      ).toEqual(['image.ora', 'a.aseprite', 'b.png']);
    });

    test('a close during a running save waits for it, then asks about the rest (AC-17)', async ({
      page,
    }) => {
      await start(page, 'pixel');
      await paint(page, 'pixel');
      await newTab(page);
      await paint(page, 'pixel');
      await setMock(page, { writeMode: 'hold' });
      await page.keyboard.press('Control+s');
      await page.waitForFunction(
        () => (window as unknown as { __mock: { release: unknown } }).__mock.release !== null,
      );
      await closeWindow(page);
      await page.waitForTimeout(300);
      await expect(prompt(page)).toHaveCount(0);
      // Other tabs can be switched to meanwhile.
      await tabs(page).first().click();
      await expectActive(page, 0);
      await releaseWrite(page);
      // The saved tab is clean now: the prompt asks about the other one.
      await expect(prompt(page)).toContainText('Save changes to "pincel.aseprite"?');
      await expectActive(page, 0);
      await prompt(page).getByRole('button', { name: "Don't Save" }).click();
      await expect.poll(() => destroyed(page)).toBe(1);
    });
  });

  test.describe('German', () => {
    test.use({ locale: 'de-DE', userAgent: UA.windows });

    test('the tab bar, the list prompt and File ▸ Close speak German (AC-18)', async ({ page }) => {
      await page.goto('./');
      await page.getByRole('button', { name: 'Verstanden' }).click();
      await page.getByRole('button', { name: 'Neues Bild' }).click();
      await page
        .getByRole('form', { name: 'Neues Bild' })
        .getByRole('button', { name: 'Erstellen' })
        .click();
      await waitForEditor(page, 'image');
      await paint(page, 'image');
      await menu(page, 'Datei', 'Neu…');
      await page
        .getByRole('dialog', { name: 'Neues Dokument' })
        .getByRole('button', { name: 'Erstellen' })
        .click();
      await paint(page, 'image');
      const bar = page.getByRole('tablist', { name: 'Geöffnete Dokumente' });
      await expect(bar.getByRole('button', { name: '„image.ora“ schließen' }).first()).toHaveAttribute(
        'title',
        /^Schließen/,
      );
      await page.getByRole('menubar').getByRole('menuitem', { name: 'Datei', exact: true }).click();
      await expect(
        page.getByRole('menu', { name: 'Datei' }).getByRole('menuitem', { name: /^Schließen/ }),
      ).toBeVisible();
      await page.keyboard.press('Escape');
      await closeWindow(page);
      const dialog = page.getByRole('alertdialog', { name: 'Ungespeicherte Änderungen' });
      await expect(dialog).toContainText('Ungespeicherte Änderungen in 2 Bildern');
      await expect(dialog.getByRole('button', { name: 'Alle speichern' })).toBeVisible();
      await expect(dialog.getByRole('button', { name: 'Nicht speichern' })).toBeVisible();
      await expect(dialog.getByRole('button', { name: 'Abbrechen' })).toBeVisible();
    });
  });

  test.describe('on Linux', () => {
    test.use({ userAgent: UA.linux });

    test('paths differing in case are different files (AC-03)', async ({ page }) => {
      await start(page, 'image');
      for (const path of ['/docs/Photo.png', '/docs/photo.png']) await putFile(page, path, PNG_1X1);
      await handOver(page, '/docs/Photo.png', 'Photo.png');
      await handOver(page, '/docs/photo.png', 'photo.png');
      await handOver(page, '/docs/Photo.png', 'Photo.png');
      await expect(tabs(page)).toHaveCount(3);
    });
  });

  test.describe('on macOS', () => {
    test.use({ userAgent: UA.mac });

    test('⌃Tab, ⇧⌘[ / ], ⌥⇟ and ⌥W work by physical key; the menu owns ⌘W (AC-20)', async ({
      page,
    }) => {
      await start(page, 'image');
      await newTab(page);
      await newTab(page);
      await page.keyboard.press('Control+Tab');
      await expectActive(page, 0);
      await page.keyboard.press('Control+Shift+Tab');
      await expectActive(page, 2);
      await page.keyboard.press('Meta+Shift+BracketRight');
      await expectActive(page, 0);
      await page.keyboard.press('Meta+Shift+BracketLeft');
      await expectActive(page, 2);
      await page.keyboard.press('Alt+PageUp');
      await expectActive(page, 1);
      // Hints in the macOS form.
      await expect(page.getByRole('button', { name: 'Close image.ora' }).first()).toHaveAttribute(
        'title',
        'Close (⌘W)',
      );
      await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
      await expect(
        page.getByRole('menu', { name: 'File' }).getByRole('menuitem', { name: /^Close/ }),
      ).toContainText('⌘W');
      await page.keyboard.press('Escape');
      // ⌘W is the native menu's accelerator: the key itself closes nothing,
      // the menu item does.
      await page.keyboard.press('Meta+w');
      await page.waitForTimeout(200);
      await expect(tabs(page)).toHaveCount(3);
      await emit(page, 'menu', 'menu:close');
      await expect(tabs(page)).toHaveCount(2);
      await pressOptionW(page);
      await expect(tabs(page)).toHaveCount(1);
    });
  });
});

test.describe('web build', () => {
  test.describe('on macOS', () => {
    test.use({ userAgent: UA.mac });

    test('⌥⇟ / ⌥⇞ / ⌥W, named with macOS symbols; no Control or ⌘ chords (AC-20)', async ({
      page,
    }) => {
      await page.goto('./');
      await page.getByRole('button', { name: 'New image' }).click();
      await page
        .getByRole('form', { name: 'New image' })
        .getByRole('button', { name: 'Create' })
        .click();
      await waitForEditor(page, 'image');
      await newTab(page);
      await page.keyboard.press('Control+Tab');
      await page.keyboard.press('Meta+Shift+BracketRight');
      await page.waitForTimeout(200);
      await expectActive(page, 1);
      await page.keyboard.press('Alt+PageDown');
      await expectActive(page, 0);
      await expect(page.getByRole('button', { name: 'Close image.ora' }).first()).toHaveAttribute(
        'title',
        'Close (⌥W)',
      );
      await page.keyboard.press('Meta+w');
      await page.waitForTimeout(200);
      await expect(tabs(page)).toHaveCount(2);
      await pressOptionW(page);
      await expect(tabs(page)).toHaveCount(1);
    });
  });

  test.describe('on Windows', () => {
    test.use({ userAgent: UA.windows });

    test('Alt+Page Down / Up / Alt+W; Ctrl+Tab and Ctrl+W stay the browser’s (AC-20)', async ({
      page,
    }) => {
      await page.goto('./');
      await page.getByRole('button', { name: 'New pixel sprite' }).click();
      await page
        .getByRole('form', { name: 'New pixel sprite' })
        .getByRole('button', { name: 'Create' })
        .click();
      await waitForEditor(page, 'pixel');
      await newTab(page);
      await page.keyboard.press('Control+Tab');
      await page.keyboard.press('Control+w');
      await page.waitForTimeout(200);
      await expect(tabs(page)).toHaveCount(2);
      await expectActive(page, 1);
      await page.keyboard.press('Alt+PageDown');
      await expectActive(page, 0);
      await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
      await expect(
        page.getByRole('menu', { name: 'File' }).getByRole('menuitem', { name: /^Close/ }),
      ).toContainText('Alt+W');
      await page.keyboard.press('Escape');
      await page.keyboard.press('Alt+w');
      await expect(tabs(page)).toHaveCount(1);
    });
  });
});
