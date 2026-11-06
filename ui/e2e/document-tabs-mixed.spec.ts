import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, menu, putClipboardImage, waitForEditor } from './helpers';
import {
  canvas,
  dropFiles,
  editorOf,
  expectActive,
  expectDirty,
  fileOf,
  installPicker,
  openDocuments,
  paint,
  prompt,
  queuePicker,
  releasePicker,
  shownEditor,
  tab,
  tabBar,
  tabNames,
  tabs,
  waitForHeldWrite,
  zoomLabel,
} from './tab-helpers';

// Document tabs of both modes in one bar (docs/specs/document-tabs.md,
// revision "tabs mix both modes"): a mode's editor mounts with its first
// document and is released with its last; only the active tab's editor is
// shown and takes input; documents open in the order they arrive.

test.beforeEach(async ({ page }) => {
  await installPicker(page);
});

/** Counts the wasm modules the page fetches, per mode. */
function trackWasm(page: Page): { pixel: number; image: number } {
  const seen = { pixel: 0, image: 0 };
  page.on('request', (req) => {
    const url = req.url();
    if (!url.endsWith('.wasm')) return;
    if (url.includes('pincel_image_wasm')) seen.image += 1;
    else if (url.includes('pincel_wasm')) seen.pixel += 1;
  });
  return seen;
}

/** The active tool of the shown editor's rail. */
function railTool(page: Page, label: RegExp) {
  return shownEditor(page).getByRole('button', { name: label }).first();
}

/** An Image tab `image.ora` (unsaved) and a Pixel tab `a.aseprite`, the
 *  Pixel one active. */
async function imageThenPixel(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await paint(page, 'image');
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await waitForEditor(page, 'pixel');
  await expectActive(page, 'a.aseprite');
}

test('one tab bar holds both modes; each document opens after the last tab, with its mode marker (AC-01, AC-02, AC-22)', async ({
  page,
}) => {
  const wasm = trackWasm(page);
  await createDocument(page, 'image', { width: 16, height: 16 });
  // A session without a Pixel document never fetches the Pixel module.
  expect(wasm).toEqual({ pixel: 0, image: 1 });
  await expect(editorOf(page, 'pixel')).toHaveCount(0);
  await paint(page, 'image');

  // A drop opens the Pixel editor with its first document.
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await waitForEditor(page, 'pixel');
  await expectActive(page, 'a.aseprite');
  expect(wasm).toEqual({ pixel: 1, image: 1 });
  // File ▸ New… of the other mode (Image) from the Pixel tab.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expect(tabs(page)).toHaveCount(3);
  await expect(tabs(page).nth(2)).toHaveAttribute('aria-selected', 'true');
  expect(await tabNames(page)).toEqual(['image.ora', 'a.aseprite', 'image.ora']);
  await expect(prompt(page)).toHaveCount(0);
  // Each tab names its mode; the first keeps its unsaved changes.
  await expect(tabs(page).nth(0)).toHaveAccessibleName(/^image\.ora Image unsaved changes$/);
  await expect(tabs(page).nth(1)).toHaveAccessibleName(/^a\.aseprite Pixel$/);
  await expect(tabs(page).nth(2)).toHaveAccessibleName(/^image\.ora Image$/);
  for (const i of [0, 1, 2]) await expect(tabs(page).nth(i).locator('svg')).toHaveCount(1);
  await expect(tabBar(page).getByRole('tab', { selected: true })).toHaveCount(1);
  expect(wasm).toEqual({ pixel: 1, image: 1 });
});

test('switching to a tab of the other mode shows that editor, its menus, panels and document (AC-04)', async ({
  page,
}) => {
  await imageThenPixel(page);
  // The top bar starts with the app name and the mode badge.
  const banner = page.getByRole('banner');
  const badge = (mode: string) => expect(banner).toHaveText(new RegExp(`^\\s*Pincel\\s*${mode}`));
  await badge('Pixel');
  await expect(page).toHaveTitle('a.aseprite – Pincel');
  await expect(page.getByRole('region', { name: 'Timeline' })).toBeVisible();
  await expect(shownEditor(page).locator('footer')).toContainText('×');

  // A click on the Image tab: Image editor, menus and title.
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await badge('Image');
  await expect(page).toHaveTitle('● image.ora – Pincel');
  await expect(page.getByRole('region', { name: 'Timeline' })).toHaveCount(0);
  await expect(shownEditor(page).locator('footer')).toContainText('16 × 16 px · sRGB');
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
  await expect(editorOf(page, 'pixel')).toHaveAttribute('inert', '');

  // Alt+Page Down / Up step across both modes and wrap around.
  await page.keyboard.press('Alt+PageDown');
  await expectActive(page, 'a.aseprite');
  await badge('Pixel');
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  await page.keyboard.press('Alt+PageDown');
  await expectActive(page, 'image.ora');
  await page.keyboard.press('Alt+PageUp');
  await expectActive(page, 'a.aseprite');
  await expect(page).toHaveTitle('a.aseprite – Pincel');
});

test('each tab keeps its view and selection across a mode switch; tools and colours stay per mode (AC-05)', async ({
  page,
}) => {
  await imageThenPixel(page);
  // Pixel tab: a zoom step, a selection, the Eraser, swapped colours.
  const fitted = await zoomLabel(page).textContent();
  await page.keyboard.press('-');
  await expect(zoomLabel(page)).not.toHaveText(fitted ?? '');
  const pixelZoom = await zoomLabel(page).textContent();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('e');
  await expect(railTool(page, /^Eraser/)).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('x');
  const hex = () => shownEditor(page).getByLabel('Foreground color hex value');
  const pixelFg = await hex().inputValue();

  // Image tab: its own tool and colours.
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await expect(railTool(page, /^Eraser/)).not.toHaveAttribute('aria-pressed', 'true');
  const imageFg = await hex().inputValue();
  expect(imageFg).not.toBe(pixelFg);

  // Back in the Pixel tab: view, selection, tool and colours as they were.
  await tab(page, 'a.aseprite').click();
  await expectActive(page, 'a.aseprite');
  await expect(zoomLabel(page)).toHaveText(pixelZoom ?? '');
  await expect(railTool(page, /^Eraser/)).toHaveAttribute('aria-pressed', 'true');
  await expect(hex()).toHaveValue(pixelFg);
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Select', exact: true }).click();
  await expect(
    page.getByRole('menu', { name: 'Select' }).getByRole('menuitem', { name: 'Deselect' }),
  ).toBeEnabled();
  await page.keyboard.press('Escape');
  // And the Image tab kept its own.
  await tab(page, 'image.ora').click();
  await expect(hex()).toHaveValue(imageFg);
});

test('File ▸ New… preselects the active tab’s mode and its size; Create opens a tab of the chosen mode, Cancel nothing (app-shell AC-08)', async ({
  page,
}) => {
  await imageThenPixel(page);
  const dialog = page.getByRole('dialog', { name: 'New document' });
  const radio = (name: RegExp) => dialog.getByRole('radio', { name });
  await menu(page, 'File', 'New…');
  await expect(radio(/^Pixel sprite/)).toHaveAttribute('aria-checked', 'true');
  await expect(dialog.getByLabel('Width')).toHaveValue('64');
  await expect(dialog.getByLabel('Height')).toHaveValue('64');
  await radio(/^Image/).click();
  await expect(dialog.getByLabel('Width')).toHaveValue('800');
  await expect(dialog.getByLabel('Height')).toHaveValue('600');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(await tabNames(page)).toEqual(['image.ora', 'a.aseprite']);

  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await menu(page, 'File', 'New…');
  await expect(radio(/^Image/)).toHaveAttribute('aria-checked', 'true');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toHaveCount(0);
  await menu(page, 'File', 'New…');
  await radio(/^Pixel sprite/).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await expectActive(page, 'pincel.aseprite');
  expect(await tabNames(page)).toEqual(['image.ora', 'a.aseprite', 'pincel.aseprite']);
  await expectDirty(page, 'image.ora', true);
});

test('closing a tab activates its right-hand neighbour of either mode; the last tab of a mode releases its editor (AC-07, AC-09, AC-16, AC-22)', async ({
  page,
}) => {
  const wasm = trackWasm(page);
  await imageThenPixel(page);
  await dropFiles(page, [await fileOf('image', 'b')]);
  await expectActive(page, 'b.png');
  await tab(page, 'a.aseprite').click();
  await expectActive(page, 'a.aseprite');
  expect(await openDocuments(page, 'pixel')).toBe(1);

  // The Pixel tab closes: the Image tab to its right follows, and the
  // Pixel editor is released (its count still readable).
  await page.keyboard.press('Alt+w');
  await expectActive(page, 'b.png');
  expect(await tabNames(page)).toEqual(['image.ora', 'b.png']);
  await expect(editorOf(page, 'pixel')).toHaveCount(0);
  expect(await openDocuments(page, 'pixel')).toBe(0);

  // A Pixel document again: its editor mounts without a new download.
  await dropFiles(page, [await fileOf('pixel', 'c')]);
  await waitForEditor(page, 'pixel');
  await expectActive(page, 'c.aseprite');
  expect(wasm.pixel).toBe(1);
  expect(await openDocuments(page, 'pixel')).toBe(1);

  // The last tab closes: its left-hand neighbour (Image) follows.
  await tabBar(page).getByRole('button', { name: 'Close c.aseprite' }).click();
  await expectActive(page, 'b.png');
  await expect(editorOf(page, 'pixel')).toHaveCount(0);
  expect(await openDocuments(page, 'pixel')).toBe(0);
  expect(await openDocuments(page, 'image')).toBe(2);
});

test('closing an unsaved tab of the hidden mode shows it while the prompt asks (AC-08)', async ({
  page,
}) => {
  await imageThenPixel(page);
  await paint(page, 'pixel');
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  const close = tabBar(page).getByRole('button', { name: 'Close a.aseprite' });

  await close.click();
  await expect(prompt(page)).toContainText('Save changes to "a.aseprite"?');
  await expectActive(page, 'a.aseprite');
  await expect(canvas(page, 'pixel')).toBeVisible();
  await prompt(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(tabs(page)).toHaveCount(2);
  await expectDirty(page, 'a.aseprite', true);

  await tab(page, 'image.ora').click();
  await close.click();
  await expectActive(page, 'a.aseprite');
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect(tabs(page)).toHaveCount(1);
  expect(await tabNames(page)).toEqual(['image.ora']);
  await expectActive(page, 'image.ora');
  await expect(editorOf(page, 'pixel')).toHaveCount(0);
});

test('only the active tab’s editor takes keys, pointer input and paste; the hidden one keeps its tabs exactly (AC-21)', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await imageThenPixel(page);
  // Pixel tab state: an edit, a zoom step, a selection.
  await paint(page, 'pixel');
  await page.keyboard.press('-');
  await page.keyboard.press('Control+a');
  const pixelFooter = await shownEditor(page).locator('footer').innerText();
  const pixelZoom = await zoomLabel(page).textContent();
  const layerRows = () =>
    shownEditor(page).getByRole('list', { name: 'Layer list' }).getByRole('listitem');
  const pixelLayers = await layerRows().count();

  // In the Image tab: keys, a stroke, undo and a paste reach Image only.
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await page.keyboard.press('e');
  await drag(page, canvas(page, 'image'), [0.2, 0.2], [0.8, 0.8]);
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Control+z');
  await expectDirty(page, 'image.ora', false);
  await putClipboardImage(page, 4, 4, '#00ff00');
  await page.keyboard.press('ControlOrMeta+v');
  await expect(layerRows()).toHaveCount(2);
  await page.keyboard.press('Escape');
  await page.keyboard.press('Space');

  // The Pixel tab is exactly as it was.
  await tab(page, 'a.aseprite').click();
  await expectActive(page, 'a.aseprite');
  await expectDirty(page, 'a.aseprite', true);
  await expect(shownEditor(page).locator('footer')).toHaveText(pixelFooter);
  await expect(zoomLabel(page)).toHaveText(pixelZoom ?? '');
  await expect(layerRows()).toHaveCount(pixelLayers);
  await expect(railTool(page, /^Pencil/)).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Select', exact: true }).click();
  await expect(
    page.getByRole('menu', { name: 'Select' }).getByRole('menuitem', { name: 'Deselect' }),
  ).toBeEnabled();
});

test('switching between tabs of two loaded modes remounts no editor and reloads no document (AC-22)', async ({
  page,
}) => {
  await imageThenPixel(page);
  await page.evaluate(() => {
    for (const el of document.querySelectorAll('canvas')) el.dataset.mark = 'kept';
  });
  const marked = await page.locator('canvas[data-mark="kept"]').count();
  const fetched: string[] = [];
  page.on('request', (req) => fetched.push(req.url()));
  for (let i = 0; i < 4; i += 1) {
    await page.keyboard.press('Alt+PageDown');
    await expect(tabBar(page).getByRole('tab', { selected: true })).toHaveCount(1);
  }
  await expectActive(page, 'a.aseprite');
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await expect(page.locator('canvas[data-mark="kept"]')).toHaveCount(marked);
  await expect(page.locator('canvas:not([data-mark])')).toHaveCount(0);
  expect(fetched.filter((u) => /\.(wasm|js)(\?|$)/.test(u))).toEqual([]);
  expect(await openDocuments(page, 'pixel')).toBe(1);
  expect(await openDocuments(page, 'image')).toBe(1);
  await expectDirty(page, 'image.ora', true);
});

test('a mode whose wasm fails to load names the document and opens no tab (AC-23)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await paint(page, 'image');
  await page.route(/pincel_wasm_bg[^/]*\.wasm/, (route) => route.abort());
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await expect(page.getByText(/^a\.aseprite could not be opened: /)).toBeVisible();
  expect(await tabNames(page)).toEqual(['image.ora']);
  await expectActive(page, 'image.ora');
  await expectDirty(page, 'image.ora', true);
  await expect(editorOf(page, 'pixel')).toHaveCount(0);

  // Back online: the next Pixel document loads the module.
  await page.unroute(/pincel_wasm_bg[^/]*\.wasm/);
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await waitForEditor(page, 'pixel');
  await expectActive(page, 'a.aseprite');
});

test('while a mode loads, tabs can be switched and later documents open after it, in order (AC-24)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await dropFiles(page, [await fileOf('image', 'x')]);
  await expectActive(page, 'x.png');
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route(/pincel_wasm_bg[^/]*\.wasm/, async (route) => {
    await held;
    await route.continue();
  });
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await dropFiles(page, [await fileOf('image', 'b')]);
  // Still loading: the open tabs stay usable.
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await page.keyboard.press('Alt+PageDown');
  await expectActive(page, 'x.png');
  expect(await tabNames(page)).toEqual(['image.ora', 'x.png']);

  release();
  await expectActive(page, 'b.png');
  expect(await tabNames(page)).toEqual(['image.ora', 'x.png', 'a.aseprite', 'b.png']);
});

test('a failed save of a tab in the background is named at once (AC-25)', async ({ page }) => {
  await imageThenPixel(page);
  await paint(page, 'pixel');
  await queuePicker(page, ['holdFail']);
  await page.keyboard.press('Control+s');
  await waitForHeldWrite(page);
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await releasePicker(page);
  await expect(page.getByText(/^Saving a\.aseprite failed: .*disk full/)).toBeVisible();
  await expectDirty(page, 'a.aseprite', true);
  await expectActive(page, 'image.ora');
});

test('opening the file of an open tab of the other mode activates that tab (AC-03)', async ({
  page,
}) => {
  // File System Access handles for the picker and for dropped items, equal
  // by file name.
  const file = await fileOf('image', 'same');
  await page.addInitScript(
    ({ name, data }) => {
      const handleFor = (n: string) => ({
        kind: 'file',
        name: n,
        getFile: async () => new File([new Uint8Array(data)], n),
        isSameEntry: async (o: { name?: string }) => o.name === n,
        queryPermission: async () => 'granted',
        requestPermission: async () => 'granted',
      });
      Object.assign(window, { showOpenFilePicker: async () => [handleFor(name)] });
      Object.defineProperty(DataTransferItem.prototype, 'getAsFileSystemHandle', {
        configurable: true,
        value(this: DataTransferItem) {
          const f = this.getAsFile();
          return Promise.resolve(f ? handleFor(f.name) : null);
        },
      });
    },
    { name: file.name, data: Array.from(file.bytes) },
  );
  await createDocument(page, 'image', { width: 16, height: 16 });
  await menu(page, 'File', 'Open…');
  await expectActive(page, file.name);
  await paint(page, 'image');
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await waitForEditor(page, 'pixel');

  // Pixel mode's own Open… (which opens a PNG as a sprite) finds it.
  await menu(page, 'File', 'Open…');
  await expectActive(page, file.name);
  // A drop from the Pixel tab too.
  await tab(page, 'a.aseprite').click();
  await dropFiles(page, [file]);
  await expectActive(page, file.name);
  expect(await tabNames(page)).toEqual(['image.ora', file.name, 'a.aseprite']);
  await expectDirty(page, file.name, true);
  await expect(prompt(page)).toHaveCount(0);
});

test('the recovery dialog lists both modes; Recover opens each as a tab of its mode (AC-15)', async ({
  page,
}) => {
  await page.clock.install();
  await imageThenPixel(page);
  await paint(page, 'pixel');
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  // The Pixel tab once more, so its snapshot is the newer one.
  await paint(page, 'pixel');
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  page.on('dialog', (d) => void d.accept());
  await page.reload();
  await createDocument(page, 'image', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await expect(dialog.getByRole('listitem')).toHaveCount(2);
  // Newest first, each with its name, mode, time and size
  // (autosave-recovery AC-04).
  await expect(dialog.getByRole('listitem').first()).toContainText('a.aseprite');
  for (const item of await dialog.getByRole('listitem').all()) {
    await expect(item).toContainText(/\d{1,2}:\d{2}.* · \d+(\.\d+)? (B|KB|MB)/);
  }
  await expect(dialog.getByRole('listitem').filter({ hasText: 'a.aseprite' })).toContainText(
    'Pixel',
  );
  await expect(dialog.getByRole('listitem').filter({ hasText: 'image.ora' })).toContainText(
    'Image',
  );
  // The Pixel snapshot: the Pixel editor loads for it, the dialog stays.
  await dialog
    .getByRole('listitem')
    .filter({ hasText: 'a.aseprite' })
    .getByRole('button', { name: 'Recover' })
    .click();
  await expect(dialog.getByRole('listitem')).toHaveCount(1);
  await expect(tabs(page)).toHaveCount(2);
  await dialog.getByRole('button', { name: 'Recover' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(tabs(page)).toHaveCount(3);
  expect(await tabNames(page)).toEqual(['image.ora', 'a.aseprite', 'image.ora']);
  await expect(tabs(page).nth(2)).toHaveAttribute('aria-selected', 'true');
  for (const i of [1, 2]) await expect(tabs(page).nth(i).getByText('unsaved changes')).toHaveCount(1);
  await tab(page, 'a.aseprite').click();
  await waitForEditor(page, 'pixel');
});

test('the tab bar keeps its scroll position across a mode switch (AC-01)', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await createDocument(page, 'image', { width: 16, height: 16 });
  for (const [mode, stem] of [
    ['pixel', 'alpha-tab'],
    ['image', 'bravo-tab'],
    ['pixel', 'charlie-tab'],
    ['image', 'delta-tab'],
  ] as const) {
    await dropFiles(page, [await fileOf(mode, stem)]);
    await expectActive(page, `${stem}.${mode === 'pixel' ? 'aseprite' : 'png'}`);
  }
  const bar = tabBar(page);
  expect(await bar.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
  // In the Pixel tab's bar, scroll so that it and the Image tab to its
  // right are both in view.
  await tab(page, 'charlie-tab.aseprite').click();
  await expectActive(page, 'charlie-tab.aseprite');
  const left = await bar.evaluate((el) => {
    const active = el.querySelector('[aria-selected="true"]')?.parentElement;
    if (active) {
      el.scrollLeft += active.getBoundingClientRect().left - el.getBoundingClientRect().left - 2;
    }
    return el.scrollLeft;
  });
  expect(left).toBeGreaterThan(0);
  await expect(tab(page, 'delta-tab.png')).toBeInViewport({ ratio: 1 });
  await page.waitForTimeout(100);
  // The Image editor's bar takes over at the same position.
  await page.keyboard.press('Alt+PageDown');
  await expectActive(page, 'delta-tab.png');
  await expect(editorOf(page, 'pixel')).toHaveAttribute('inert', '');
  expect(await bar.evaluate((el) => el.scrollLeft)).toBe(left);
});
