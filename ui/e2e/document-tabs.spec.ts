import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, gotoStart, menu, putClipboardImage, waitForEditor } from './helpers';
import {
  DEFAULT_NAME,
  EXT,
  canvas,
  dropFiles,
  editorOf,
  expectActive,
  expectDirty,
  fileOf,
  installPicker,
  openDocuments,
  openTabs,
  other,
  otherFile,
  paint,
  picker,
  prompt,
  queuePicker,
  releasePicker,
  tab,
  tabBar,
  tabNames,
  tabs,
  waitForHeldWrite,
  zoomLabel,
} from './tab-helpers';

// Document tabs (docs/specs/document-tabs.md), within one mode: several
// documents of a mode open side by side, each with its own state. Tabs of
// both modes mixed are in `document-tabs-mixed.spec.ts`, the desktop paths
// (file paths, closing the window, desktop keys) in
// `document-tabs-desktop.spec.ts`. Helpers and the save-picker stub are in
// `tab-helpers.ts`.

test.beforeEach(async ({ page }) => {
  await installPicker(page);
});

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
    // The active tab is marked visually as well: its background differs.
    const background = (t: Locator) =>
      t.evaluate((el) => getComputedStyle(el.parentElement ?? el).backgroundColor);
    expect(await background(tabs(page).nth(2))).not.toBe(await background(tabs(page).nth(0)));
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
    const redo = page.getByRole('button', { name: 'Redo', exact: true });
    await expect(redo).toBeEnabled();

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
    await expect(redo).toBeDisabled();
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

  test(`${mode}: another mode's document opens as a tab beside the unsaved ones, without a prompt (AC-02)`, async ({
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
    await expectActive(page, name('a'));

    const x = await otherFile(mode, 'other');
    await dropFiles(page, [x]);
    await waitForEditor(page, other(mode));
    await expectActive(page, x.name);
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('a'), x.name]);
    await expect(prompt(page)).toHaveCount(0);
    await expectDirty(page, DEFAULT_NAME[mode], true);
    // Back in the first tab: its document and history are still there.
    await tab(page, DEFAULT_NAME[mode]).click();
    await expectActive(page, DEFAULT_NAME[mode]);
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
  });

  test(`${mode}: closing tabs frees their documents; the last tab of a mode releases its editor (AC-16)`, async ({
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
    // A tab of the other mode, then every tab of this mode closed.
    const x = await otherFile(mode, 'other');
    await dropFiles(page, [x]);
    await waitForEditor(page, other(mode));
    expect(await openDocuments(page, other(mode))).toBe(1);
    for (const n of [DEFAULT_NAME[mode], name('a'), name('c')]) {
      await tabBar(page)
        .getByRole('button', { name: `Close ${n}` })
        .click();
    }
    expect(await tabNames(page)).toEqual([x.name]);
    await expect(editorOf(page, mode)).toHaveCount(0);
    // The count stays readable after the release: the module stays loaded.
    expect(await openDocuments(page, mode)).toBe(0);
    expect(await openDocuments(page, other(mode))).toBe(1);
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
    await waitForHeldWrite(page);
    // Closing the saving tab does nothing, by any means.
    await tabBar(page)
      .getByRole('button', { name: `Close ${DEFAULT_NAME[mode]}` })
      .click();
    await page.keyboard.press('Alt+w');
    await expect(tabs(page)).toHaveCount(2);
    await expect(prompt(page)).toHaveCount(0);
    // Another tab can be switched to, also one of the other mode.
    await tab(page, name('a')).click();
    await expectActive(page, name('a'));
    const x = await otherFile(mode, 'other');
    await dropFiles(page, [x]);
    await waitForEditor(page, other(mode));
    await expectActive(page, x.name);
    await expect(tabs(page)).toHaveCount(3);
    await releasePicker(page);
    await expectDirty(page, DEFAULT_NAME[mode], false);
    expect((await picker(page)).written).toEqual([DEFAULT_NAME[mode]]);
  });

  test(`${mode}: dropping several files opens each as a tab of its mode, in drop order (AC-19)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const x = await otherFile(mode, 'x');
    await dropFiles(page, [
      await fileOf(mode, 'a'),
      x,
      { name: 'notes.txt', bytes: Buffer.from('hello') },
      await fileOf(mode, 'b'),
    ]);
    await expectActive(page, name('b'));
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], name('a'), x.name, name('b')]);
    await expect(page.getByText('1 dropped file could not be opened')).toBeVisible();
    await expect(prompt(page)).toHaveCount(0);
    await tab(page, x.name).click();
    await expectActive(page, x.name);
    await expect(canvas(page, other(mode))).toHaveAttribute('data-ready', 'true');
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
      // The bar sits between the options bar and the canvas.
      const options = await page.getByRole('toolbar', { name: 'Tool options' }).boundingBox();
      const barBox = await bar.boundingBox();
      const stage = await canvas(page, mode).boundingBox();
      if (!options || !barBox || !stage) throw new Error('editor frame not laid out');
      expect(barBox.y).toBeGreaterThanOrEqual(options.y + options.height - 1);
      expect(stage.y).toBeGreaterThanOrEqual(barBox.y + barBox.height - 1);
      if (width < 1280) {
        expect(await bar.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
      }
      await page.keyboard.press('Alt+PageDown');
      await expect(tab(page, DEFAULT_NAME[mode])).toBeInViewport();
    }
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

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: dropping the file of an open tab activates that tab (AC-03, AC-19)`, async ({
    page,
  }) => {
    // File System Access handles for the picker and for dropped items
    // (`DataTransferItem.getAsFileSystemHandle`), equal by file name.
    const file = await fileOf(mode, 'dropped');
    await page.addInitScript(
      ({ name, data }) => {
        const handleFor = (n: string) => ({
          kind: 'file',
          name: n,
          getFile: async () => new File([new Uint8Array(data)], n),
          isSameEntry: async (other: { name?: string }) => other.name === n,
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
    await createDocument(page, mode, { width: 16, height: 16 });
    await menu(page, 'File', 'Open…');
    await expectActive(page, file.name);
    await paint(page, mode);
    await tab(page, DEFAULT_NAME[mode]).click();
    await dropFiles(page, [file]);
    await expectActive(page, file.name);
    await expect(tabs(page)).toHaveCount(2);
    await expectDirty(page, file.name, true);
    // Among several dropped files too, the open one is activated, not reopened.
    await dropFiles(page, [await fileOf(mode, 'new'), file]);
    await expectActive(page, file.name);
    expect(await tabNames(page)).toEqual([DEFAULT_NAME[mode], file.name, `new.${EXT[mode]}`]);
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
    const size = mode === 'pixel' ? '16×16' : '16 × 16 px · sRGB';
    await expect(footer).toContainText(size);
    const swatches = page.getByRole('group', { name: 'Palette colors' }).getByRole('button');
    const paletteFirst = mode === 'pixel' ? await swatches.count() : 0;
    // The Layers panel: one more layer in this tab.
    const layerRows = page.getByRole('list', { name: 'Layer list' }).getByRole('listitem');
    await page.getByRole('button', { name: 'Add layer', exact: true }).click();
    await expect(layerRows).toHaveCount(2);
    const layersFirst = await layerRows.allInnerTexts();
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
    expect(await layerRows.allInnerTexts()).not.toEqual(layersFirst);

    await tab(page, DEFAULT_NAME[mode]).click();
    await expect(footer).toContainText(size);
    if (mode === 'pixel') expect(await swatches.count()).toBe(paletteFirst);
    await expect.poll(() => layerRows.allInnerTexts()).toEqual(layersFirst);
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
    await expect(page.locator('footer')).toContainText('24 × 12 px · sRGB');
    await expectDirty(page, 'image.ora', true);
    await expect(prompt(page)).toHaveCount(0);
    await tab(page, 'image.ora').click();
    await expect(page.locator('footer')).toContainText('16 × 16 px · sRGB');
  });

  test('pixel: Paste as New Image opens a Pixel tab after the unsaved one, without a prompt (AC-02)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await paint(page, 'pixel');
    await putClipboardImage(page, 24, 12, '#00ff00');
    await page.keyboard.press('ControlOrMeta+Alt+v');
    // A tab of the active tab's mode (document-tabs Terms).
    await expectActive(page, 'Pasted image.aseprite');
    expect(await tabNames(page)).toEqual(['pincel.aseprite', 'Pasted image.aseprite']);
    await expect(page.locator('footer')).toContainText('24×12');
    await expect(editorOf(page, 'image')).toHaveCount(0);
    await expectDirty(page, 'pincel.aseprite', true);
    await expect(prompt(page)).toHaveCount(0);
  });
});
