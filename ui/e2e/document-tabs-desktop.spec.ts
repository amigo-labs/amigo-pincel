import { expect, test, type Locator, type Page } from '@playwright/test';
import { drag, imageCanvas, menu, pixelCanvas, waitForEditor } from './helpers';
import {
  closeWindow,
  destroyed,
  emit,
  installTauriMock,
  putFile,
  releaseWrite,
  setMock,
} from './tauri-mock';

// Document tabs in the desktop app (docs/specs/document-tabs.md), against
// the mocked Tauri bridge of `tauri-mock.ts`: the same file by path
// (AC-03), closing the window with several tabs (AC-10, AC-11, AC-13,
// AC-17), and the tab keys of the desktop app and the web build on each
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
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(
    page,
    mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page),
    [0.3, 0.5],
    [0.7, 0.5],
  );
}

/** File ▸ New… in the current mode: a new tab. */
async function newTab(page: Page): Promise<void> {
  const before = await tabs(page).count();
  await menu(page, 'File', 'New…');
  await page
    .getByRole('dialog', { name: 'New document' })
    .getByRole('button', { name: 'Create' })
    .click();
  await expect(tabs(page)).toHaveCount(before + 1);
}

/** Hands `path` over as the OS does until `name`'s tab is the active one:
 *  a hand-over that arrives while the previous one is still being opened
 *  is dropped, so it is repeated. */
async function handOver(page: Page, path: string, name: string): Promise<void> {
  await expect(async () => {
    await emit(page, 'open-file', path);
    await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true', { timeout: 1_000 });
  }).toPass();
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
        await page.keyboard.press('Control+w');
        await expect(tabs(page)).toHaveCount(1);
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

    test('closing the window with several unsaved tabs lists them; Save All saves, then closes (AC-10, AC-11, AC-13)', async ({
      page,
    }) => {
      await start(page, 'pixel');
      await paint(page, 'pixel');
      await newTab(page);
      await paint(page, 'pixel');
      await newTab(page);
      // The active tab is clean; the close still asks (AC-13).
      await closeWindow(page);
      await expect(prompt(page)).toContainText('Unsaved changes in 2 images');
      await prompt(page).getByRole('button', { name: 'Cancel' }).click();
      expect(await destroyed(page)).toBe(0);
      await closeWindow(page);
      await prompt(page).getByRole('button', { name: 'Save All' }).click();
      await expect.poll(() => destroyed(page)).toBe(1);
    });

    test('one unsaved tab in the background is asked about and activated before closing (AC-10)', async ({
      page,
    }) => {
      await start(page, 'image');
      await paint(page, 'image');
      await newTab(page);
      await closeWindow(page);
      await expect(prompt(page)).toContainText('Save changes to "image.ora"?');
      await expectActive(page, 0);
      await prompt(page).getByRole('button', { name: "Don't Save" }).click();
      await expect.poll(() => destroyed(page)).toBe(1);
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
