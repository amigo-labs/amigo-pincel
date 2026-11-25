import { expect, test, type Locator, type Page } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en, type MessageKey } from '../src/lib/i18n/en';
import { es } from '../src/lib/i18n/es';
import { fr } from '../src/lib/i18n/fr';
import { ja } from '../src/lib/i18n/ja';
import { ptBr } from '../src/lib/i18n/pt-br';
import { animatedGif, animatedWebp, apng, walkGif } from './animation-files';
import {
  createDocument,
  fakeFileHandles,
  fakeFs,
  gotoStart,
  menu,
  rasterFile,
  waitForEditor,
} from './helpers';
import {
  PNG_1X1,
  dispatchDrop,
  installPicker,
  picker,
  shownEditor,
  tab,
  tabNames,
  tabs,
} from './tab-helpers';
import { emit, installTauriMock, putFile } from './tauri-mock';

// Animated GIF, APNG and WebP on open (docs/specs/animated-image-import.md):
// every document-open path asks "animation or first frame?", the animation
// opens as an unsaved Pixel sprite with one frame per animation frame.

const TITLE = 'Animated image';
const AS_ANIMATION = 'Open as animation (Pixel mode)';
const FIRST_FRAME = 'Open first frame';

function dialog(page: Page): Locator {
  return page.getByRole('dialog', { name: TITLE });
}

/** Opening a file the header look finds animated loads the Pixel core
 *  for the probe first (dev wasm, slow under load): like editor start-up
 *  (`waitForEditor`) that wait gets its own budget. */
const START_BUDGET = 60_000;

async function withStartBudget(wait: () => Promise<void>): Promise<void> {
  const info = test.info();
  const base = info.timeout;
  info.setTimeout(base + START_BUDGET);
  const started = Date.now();
  await wait();
  info.setTimeout(base + (Date.now() - started));
}

/** Waits for the dialog to appear after an open. */
async function waitForDialog(page: Page): Promise<void> {
  await withStartBudget(() => expect(dialog(page)).toBeVisible({ timeout: START_BUDGET }));
}

function notice(page: Page): Locator {
  return page.getByRole('alert');
}

function timeline(page: Page): Locator {
  return shownEditor(page).getByRole('region', { name: 'Timeline' });
}

/** Runs `open` and answers the <input type=file> chooser it raises. */
async function choose(page: Page, open: () => Promise<void>, name: string, buffer: Buffer) {
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), open()]);
  await chooser.setFiles({ name, mimeType: '', buffer });
}

/** The start screen's Open… with `name` (input fallback). */
async function openFromStart(page: Page, name: string, buffer: Buffer): Promise<void> {
  await choose(page, () => page.getByRole('button', { name: 'Open…' }).click(), name, buffer);
}

/** The status bar's document size in `mode`'s editor. */
function sizeShown(page: Page, mode: 'pixel' | 'image', w: number, h: number): Locator {
  return shownEditor(page).getByText(mode === 'pixel' ? `${w} × ${h} px` : `${w} × ${h} px`, {
    exact: true,
  });
}

/** The number of frames the Pixel timeline shows. */
async function frameCount(page: Page): Promise<number> {
  return timeline(page)
    .getByRole('button', { name: /^Frame \d+$/ })
    .count();
}

test.describe('input fallback', () => {
  test.beforeEach(async ({ page }) => {
    await installPicker(page);
  });

  test('AC-01, AC-03, AC-10: Open… asks first, then opens an unsaved Pixel sprite with every frame', async ({
    page,
  }) => {
    await gotoStart(page);
    await openFromStart(page, 'walk.gif', walkGif(3, 6, 4));
    await waitForDialog(page);
    await expect(dialog(page)).toBeVisible();
    await expect(dialog(page)).toContainText('walk.gif has 3 frames (6 × 4 px)');
    // No document opens before the choice.
    await expect(tabs(page)).toHaveCount(0);
    await expect(dialog(page).getByRole('button', { name: AS_ANIMATION })).toBeFocused();

    await dialog(page).getByRole('button', { name: AS_ANIMATION }).click();
    await waitForEditor(page, 'pixel');
    await expect(dialog(page)).toBeHidden();
    await expect(sizeShown(page, 'pixel', 6, 4)).toBeVisible();
    expect(await frameCount(page)).toBe(3);
    for (const n of [1, 2, 3]) {
      await expect(timeline(page).getByLabel(`Frame ${n} duration (ms)`)).toHaveValue('70');
    }
    await expect(shownEditor(page).getByText('Layer 1', { exact: true }).first()).toBeVisible();
    expect(await tabNames(page)).toEqual(['walk.aseprite']);
    // Unsaved, named after the file; the first Save asks for a location.
    await expect(tab(page, 'walk.aseprite')).toHaveAttribute(
      'title',
      'walk.aseprite — unsaved changes',
    );
    await page.keyboard.press('Control+s');
    await expect.poll(async () => (await picker(page)).written).toEqual(['walk.aseprite']);
    expect((await picker(page)).calls).toBe(1);
  });

  test('AC-01, AC-03: an animated APNG and an animated WebP ask and open with all frames', async ({
    page,
  }) => {
    await gotoStart(page);
    const webp = await animatedWebp(page, 4);
    await openFromStart(
      page,
      'blink.png',
      apng(
        4,
        2,
        [
          [255, 0, 0, 255],
          [0, 255, 0, 255],
        ],
        50,
      ),
    );
    await waitForDialog(page);
    await expect(dialog(page)).toContainText('blink.png has 2 frames (4 × 2 px)');
    await dialog(page).getByRole('button', { name: AS_ANIMATION }).click();
    await waitForEditor(page, 'pixel');
    expect(await frameCount(page)).toBe(2);
    await expect(timeline(page).getByLabel('Frame 2 duration (ms)')).toHaveValue('50');

    await choose(page, () => menu(page, 'File', 'Open…'), 'spin.webp', webp);
    await waitForDialog(page);
    await expect(dialog(page)).toContainText('spin.webp has 4 frames (8 × 6 px)');
    await dialog(page).getByRole('button', { name: AS_ANIMATION }).click();
    await expect(tab(page, 'spin.aseprite')).toHaveAttribute('aria-selected', 'true');
    await expect(sizeShown(page, 'pixel', 8, 6)).toBeVisible();
    expect(await frameCount(page)).toBe(4);
    await expect(timeline(page).getByLabel('Frame 4 duration (ms)')).toHaveValue('80');
  });

  test('AC-02: a static GIF, a one-frame APNG and a still WebP open without the dialog', async ({
    page,
  }) => {
    await gotoStart(page);
    const still = await rasterFile(page, 'webp');
    await openFromStart(page, 'dot.gif', await rasterFile(page, 'gif'));
    await waitForEditor(page, 'image');
    await expect(sizeShown(page, 'image', 1, 1)).toBeVisible();
    const files: [string, Buffer][] = [
      ['one.png', apng(3, 3, [[0, 0, 255, 255]])],
      ['still.webp', still],
    ];
    for (const [name, bytes] of files) {
      await choose(page, () => menu(page, 'File', 'Open…'), name, bytes);
      await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true');
    }
    await expect(dialog(page)).toHaveCount(0);
    expect(await tabNames(page)).toEqual(['dot.gif', 'one.png', 'still.webp']);
  });

  test('AC-06: "Open first frame" opens the file in Image mode as before', async ({ page }) => {
    await gotoStart(page);
    await openFromStart(page, 'walk.gif', walkGif(3, 6, 4));
    await waitForDialog(page);
    await dialog(page).getByRole('button', { name: FIRST_FRAME }).click();
    await waitForEditor(page, 'image');
    await expect(sizeShown(page, 'image', 6, 4)).toBeVisible();
    expect(await tabNames(page)).toEqual(['walk.gif']);
  });

  test('AC-06: an APNG through Pixel File ▸ Open… opens its first frame as a one-frame sprite', async ({
    page,
  }) => {
    await createDocument(page, 'pixel');
    const bytes = apng(5, 3, [
      [255, 0, 0, 255],
      [0, 255, 0, 255],
      [0, 0, 255, 255],
    ]);
    await choose(page, () => menu(page, 'File', 'Open…'), 'blink.png', bytes);
    await waitForDialog(page);
    await dialog(page).getByRole('button', { name: FIRST_FRAME }).click();
    await expect(tab(page, 'blink.aseprite')).toHaveAttribute('aria-selected', 'true');
    await expect(sizeShown(page, 'pixel', 5, 3)).toBeVisible();
    expect(await frameCount(page)).toBe(1);
  });

  test('AC-07: Cancel opens nothing and leaves the open documents as they were', async ({
    page,
  }) => {
    await createDocument(page, 'image');
    const before = await tabNames(page);
    await choose(page, () => menu(page, 'File', 'Open…'), 'walk.gif', walkGif());
    await waitForDialog(page);
    await expect(dialog(page)).toBeVisible();
    await dialog(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog(page)).toBeHidden();
    expect(await tabNames(page)).toEqual(before);
    await expect(notice(page)).toHaveCount(0);
    // Escape cancels too.
    await choose(page, () => menu(page, 'File', 'Open…'), 'walk.gif', walkGif());
    await waitForDialog(page);
    await expect(dialog(page)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(dialog(page)).toBeHidden();
    expect(await tabNames(page)).toEqual(before);
  });

  test('AC-08: a canvas wider than 4096 px disables "Open as animation" and names the limit', async ({
    page,
  }) => {
    await gotoStart(page);
    const bytes = animatedGif(5120, 300, [
      { width: 1, height: 1, fill: 0 },
      { width: 1, height: 1, fill: 1 },
    ]);
    await openFromStart(page, 'wide.gif', bytes);
    await waitForDialog(page);
    await expect(dialog(page).getByRole('button', { name: AS_ANIMATION })).toBeDisabled();
    await expect(dialog(page).getByTestId('animation-limit')).toHaveText(
      'Too large to open as an animation: 5120 × 300 px, the limit is 4096 px per side',
    );
    await expect(dialog(page).getByRole('button', { name: FIRST_FRAME })).toBeFocused();
    await dialog(page).getByRole('button', { name: FIRST_FRAME }).click();
    await waitForEditor(page, 'image');
    await expect(sizeShown(page, 'image', 5120, 300)).toBeVisible();
  });

  test('AC-08: frames × width × height above the pixel budget disables "Open as animation"', async ({
    page,
  }) => {
    await gotoStart(page);
    const frames = Array.from({ length: 17 }, (_, i) => ({ width: 1, height: 1, fill: i % 3 }));
    await openFromStart(page, 'long.gif', animatedGif(2048, 2048, frames));
    await waitForDialog(page);
    await expect(dialog(page).getByRole('button', { name: AS_ANIMATION })).toBeDisabled();
    await expect(dialog(page).getByTestId('animation-limit')).toHaveText(
      'Too large to open as an animation: 17 frames × 2048 × 2048 px = 71303168 pixels, the limit is 67108864 pixels',
    );
    await expect(dialog(page).getByRole('button', { name: FIRST_FRAME })).toBeEnabled();
  });

  test('AC-09: a broken later frame opens no animation, says so, and the first frame still opens', async ({
    page,
  }) => {
    await gotoStart(page);
    const bytes = animatedGif(4, 4, [
      { width: 4, height: 4, fill: 0 },
      { width: 4, height: 4, fill: 1, broken: true },
    ]);
    await openFromStart(page, 'broken.gif', bytes);
    await waitForDialog(page);
    await dialog(page).getByRole('button', { name: AS_ANIMATION }).click();
    await expect(notice(page)).toContainText(
      'broken.gif: the animation cannot be read completely.',
    );
    await expect(tabs(page)).toHaveCount(0);
    await page.getByRole('button', { name: 'Dismiss' }).click();

    await openFromStart(page, 'broken.gif', bytes);
    await waitForDialog(page);
    await dialog(page).getByRole('button', { name: FIRST_FRAME }).click();
    await waitForEditor(page, 'image');
    await expect(sizeShown(page, 'image', 4, 4)).toBeVisible();
  });

  test('AC-13: an undecodable first frame shows no dialog and fails like an unreadable image', async ({
    page,
  }) => {
    await gotoStart(page);
    const bytes = animatedGif(4, 4, [
      { width: 4, height: 4, fill: 0, broken: true },
      { width: 4, height: 4, fill: 1 },
    ]);
    await openFromStart(page, 'bad.gif', bytes);
    // The probe loads the Pixel core, then the Image editor fails to open it.
    await withStartBudget(() =>
      expect(notice(page)).toContainText('bad.gif could not be opened', { timeout: START_BUDGET }),
    );
    await expect(dialog(page)).toHaveCount(0);
    await expect(tabs(page)).toHaveCount(0);
  });

  test('AC-01: File ▸ Open… in both modes and drops onto the start screen or an editor ask', async ({
    page,
  }) => {
    const gif = walkGif();
    // A file dropped onto the start screen.
    await gotoStart(page);
    await dispatchDrop(page, [{ name: 'walk.gif', bytes: gif }]);
    await waitForDialog(page);
    await expect(dialog(page)).toBeVisible();
    await expect(tabs(page)).toHaveCount(0);
    await dialog(page).getByRole('button', { name: 'Cancel' }).click();

    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode);
      // Pixel / Image File ▸ Open….
      await choose(page, () => menu(page, 'File', 'Open…'), 'walk.gif', gif);
      await waitForDialog(page);
      await expect(dialog(page)).toBeVisible();
      await expect(tabs(page)).toHaveCount(1);
      await dialog(page).getByRole('button', { name: 'Cancel' }).click();
      // Dropped "as document" onto the editor.
      await dispatchDrop(page, [{ name: 'walk.gif', bytes: gif }]);
      await page
        .getByRole('dialog', { name: 'Dropped images' })
        .getByRole('button', { name: 'Open as document' })
        .click();
      await waitForDialog(page);
      await expect(dialog(page)).toBeVisible();
      await expect(tabs(page)).toHaveCount(1);
      await dialog(page).getByRole('button', { name: 'Cancel' }).click();
      await expect(tabs(page)).toHaveCount(1);
    }
  });

  test('AC-15: the dialog is modal: drops and the editor menus are blocked', async ({ page }) => {
    await createDocument(page, 'pixel');
    await choose(page, () => menu(page, 'File', 'Open…'), 'walk.gif', walkGif());
    await waitForDialog(page);
    await expect(dialog(page)).toBeVisible();
    // The menu bar sits under the dialog's backdrop.
    const fileMenu = page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true });
    const box = await fileMenu.boundingBox();
    expect(box).not.toBeNull();
    const hit = await page.evaluate(
      ([x, y]) => document.elementFromPoint(x, y)?.closest('[role="dialog"]')?.ariaLabel ?? null,
      [(box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2] as const,
    );
    expect(hit).toBe(TITLE);
    // A drop is not taken.
    await dispatchDrop(page, [{ name: 'dot.png', bytes: PNG_1X1 }]);
    await expect(page.getByRole('dialog', { name: 'Dropped images' })).toHaveCount(0);
    await dialog(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog(page)).toBeHidden();
    expect(await tabNames(page)).toHaveLength(1);
  });
});

test('AC-01: Open Recent asks as well', async ({ page }) => {
  const gif = walkGif();
  await fakeFileHandles(page, { 'walk.gif': gif });
  await gotoStart(page);
  await fakeFs(page, { pick: 'walk.gif' });
  await page.getByRole('button', { name: 'Open…' }).click();
  await waitForDialog(page);
  await dialog(page).getByRole('button', { name: FIRST_FRAME }).click();
  await waitForEditor(page, 'image');
  // Close the tab (it is unchanged), then reopen it from the recents.
  await page.getByRole('button', { name: 'Close walk.gif' }).click();
  await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
  await page
    .getByRole('button', { name: /walk\.gif/ })
    .first()
    .click();
  await waitForDialog(page);
  await expect(dialog(page)).toBeVisible();
  await expect(tabs(page)).toHaveCount(0);
  await dialog(page).getByRole('button', { name: AS_ANIMATION }).click();
  await waitForEditor(page, 'pixel');
  expect(await frameCount(page)).toBe(3);
});

test.describe('desktop', () => {
  test.use({
    userAgent:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
  });

  test('AC-01, AC-14, AC-15: OS hand-overs wait for the dialog and open in arrival order from the bytes read', async ({
    page,
  }) => {
    await installTauriMock(page);
    await page.goto('./');
    await page.getByRole('button', { name: 'Got it' }).click();
    await putFile(page, '/u/a.gif', walkGif(3, 6, 4));
    await putFile(page, '/u/b.gif', walkGif(2));
    await putFile(page, '/u/c.png', PNG_1X1);
    await emit(page, 'open-file', '/u/a.gif');
    await waitForDialog(page);
    await expect(dialog(page)).toContainText('a.gif has 3 frames');
    await emit(page, 'open-file', '/u/b.gif');
    await emit(page, 'open-file', '/u/c.png');
    await expect(tabs(page)).toHaveCount(0);
    // The file changes on disk while the dialog is open: the content read
    // when it appeared opens.
    await putFile(page, '/u/a.gif', walkGif(5, 2, 2));
    await dialog(page).getByRole('button', { name: AS_ANIMATION }).click();
    await expect(dialog(page)).toContainText('b.gif has 2 frames');
    await waitForEditor(page, 'pixel');
    await expect(sizeShown(page, 'pixel', 6, 4)).toBeVisible();
    expect(await frameCount(page)).toBe(3);
    expect(await tabNames(page)).toEqual(['a.aseprite']);
    // Cancel the second; the static third opens after it.
    await dialog(page).getByRole('button', { name: 'Cancel' }).click();
    // c.png is the first Image-mode document: its editor loads first.
    await waitForEditor(page, 'image');
    await expect(tab(page, 'c.png')).toHaveAttribute('aria-selected', 'true');
    expect(await tabNames(page)).toEqual(['a.aseprite', 'c.png']);
  });
});

test('AC-12: every string of the dialog has a translation in every UI language', () => {
  const keys = (Object.keys(en) as MessageKey[]).filter((k) => k.startsWith('animOpen.'));
  expect(keys.length).toBeGreaterThanOrEqual(8);
  const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
  for (const [name, table] of Object.entries({ de, es, fr, ja, ptBr })) {
    for (const key of keys) {
      const text = table[key];
      expect(text, `${name} ${key}`).toBeTruthy();
      expect(text, `${name} ${key} is translated`).not.toBe(en[key]);
      expect(placeholders(text), `${name} ${key}`).toEqual(placeholders(en[key]));
    }
  }
});
