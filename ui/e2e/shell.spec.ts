import { expect, test, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  storedPref,
  waitForEditor,
} from './helpers';
import {
  dropFiles,
  editorOf,
  expectActive,
  fileOf,
  shownEditor,
  tab,
  tabs,
} from './tab-helpers';

// The app shell (docs/specs/app-shell.md): start screen, New cards and the
// New dialog, at most one editor per mode behind the tab bar, and the
// shared editor frame. The unsaved-changes prompt has its own spec
// (unsaved-prompt.spec.ts).

type Mode = 'pixel' | 'image';

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** The document size as the mode's status bar shows it. */
const sizeText = (mode: Mode, w: number, h: number) =>
  mode === 'pixel' ? `${w}×${h}` : `${w} × ${h} · sRGB`;

/** The editor top bar (mode badge, menus, document name). */
const topBar = (page: Page) => page.getByRole('banner');

/** One edit that makes the document dirty. */
async function edit(page: Page, mode: Mode): Promise<void> {
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(page, canvasOf(page, mode), [0.2, 0.5], [0.8, 0.5]);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
}

/** Submits a start-screen New card with the typed size (strings, so a field can be cleared). */
async function submitCard(page: Page, mode: Mode, width: string, height: string): Promise<void> {
  await gotoStart(page);
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const [label, value] of [
    ['Width', width],
    ['Height', height],
  ] as const) {
    const field = card.getByLabel(label);
    await expect(async () => {
      await field.fill(value);
      await expect(field).toHaveValue(value, { timeout: 500 });
    }).toPass();
  }
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

test('AC-01: the start screen offers New, Open… and About, with no editor mounted', async ({
  page,
}) => {
  await gotoStart(page);
  await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'New image' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open…' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'About Pincel' })).toBeVisible();
  // No editor: neither canvas, no menu bar, no editor top bar.
  await expect(pixelCanvas(page)).toHaveCount(0);
  await expect(imageCanvas(page)).toHaveCount(0);
  await expect(page.getByRole('menubar')).toHaveCount(0);
  await expect(topBar(page)).toHaveCount(0);
});

test('AC-05: the New pixel sprite card opens a sprite of exactly the typed size', async ({
  page,
}) => {
  await submitCard(page, 'pixel', '40', '24');
  await expect(topBar(page).getByText('Pixel', { exact: true })).toBeVisible();
  await expect(page.getByText(sizeText('pixel', 40, 24), { exact: true })).toBeVisible();
});

test('AC-06: an empty size falls back to the default, an out-of-range one is clamped', async ({
  page,
}) => {
  // Empty → the mode's default; above the edge limit → the limit; 0 → 1.
  await submitCard(page, 'pixel', '', '5000');
  await expect(page.getByText(sizeText('pixel', 64, 4096), { exact: true })).toBeVisible();
  await submitCard(page, 'pixel', '0', '');
  await expect(page.getByText(sizeText('pixel', 1, 64), { exact: true })).toBeVisible();
  await submitCard(page, 'image', '', '');
  await expect(page.getByText(sizeText('image', 800, 600))).toBeVisible();
  await submitCard(page, 'image', '20000', '0');
  await expect(page.getByText(sizeText('image', 16384, 1))).toBeVisible();

  // The New dialog clamps the same way.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Pixel sprite/ }).click();
  await dialog.getByLabel('Width').fill('9999');
  await dialog.getByLabel('Height').fill('');
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'pixel');
  await expect(page.getByText(sizeText('pixel', 4096, 64), { exact: true })).toBeVisible();
  // The area limit (16384 × 16384 → 8192 × 8192) is pinned in
  // large-image.spec.ts › the New dialog clamps an oversized canvas.
});

for (const mode of ['pixel', 'image'] as const) {
  const other: Mode = mode === 'pixel' ? 'image' : 'pixel';
  test(`AC-08: ${mode}: File ▸ New… preselects the mode, resets sizes and creates or cancels`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 30, height: 20 });
    // An edit, so "unchanged" below means the edit survived; New… opens its
    // dialog at once, an unsaved tab or not (document-tabs AC-02).
    await edit(page, mode);
    const dialog = page.getByRole('dialog', { name: 'New document' });
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const openNew = async () => {
      await menu(page, 'File', 'New…');
      await expect(dialog).toBeVisible();
    };
    const unchanged = async () => {
      await expect(dialog).toHaveCount(0);
      await expect(tabs(page)).toHaveCount(1);
      await expect(shownEditor(page)).toHaveAttribute('data-editor', mode);
      await expect(page.getByText(sizeText(mode, 30, 20), { exact: true })).toBeVisible();
      await expect(undo).toBeEnabled();
      await expect(page.getByTitle(/unsaved changes/)).toBeVisible();
    };
    const radio = (m: Mode) =>
      dialog.getByRole('radio', { name: m === 'pixel' ? /^Pixel sprite/ : /^Image/ });
    const defaults = { pixel: ['64', '64'], image: ['800', '600'] } as const;

    // Escape and Cancel close the dialog; the document stays as it was.
    await openNew();
    await page.keyboard.press('Escape');
    await unchanged();
    await openNew();
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await unchanged();

    // The current mode is preselected with its default size; switching
    // the mode resets the size to the other mode's default.
    await openNew();
    await expect(radio(mode)).toHaveAttribute('aria-checked', 'true');
    await expect(radio(other)).toHaveAttribute('aria-checked', 'false');
    await expect(dialog.getByLabel('Width')).toHaveValue(defaults[mode][0]);
    await expect(dialog.getByLabel('Height')).toHaveValue(defaults[mode][1]);
    await dialog.getByLabel('Width').fill('12');
    await radio(other).click();
    await expect(radio(other)).toHaveAttribute('aria-checked', 'true');
    await expect(dialog.getByLabel('Width')).toHaveValue(defaults[other][0]);
    await expect(dialog.getByLabel('Height')).toHaveValue(defaults[other][1]);

    // Create opens a new tab of the chosen mode, shown in that mode's
    // editor; the first tab keeps its document and its unsaved changes.
    await dialog.getByLabel('Width').fill('16');
    await dialog.getByLabel('Height').fill('8');
    await dialog.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, other);
    await expect(tabs(page)).toHaveCount(2);
    await expect(tabs(page).nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(shownEditor(page)).toHaveAttribute('data-editor', other);
    await expect(page.getByText(sizeText(other, 16, 8), { exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
    await expect(tabs(page).first().getByText('unsaved changes')).toHaveCount(1);
  });
}

test('AC-13: a Pixel session mounts one editor and fetches the image wasm only with an Image tab', async ({
  page,
}) => {
  const wasm: string[] = [];
  page.on('request', (req) => {
    if (req.url().endsWith('.wasm')) wasm.push(req.url());
  });
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await edit(page, 'pixel');
  await expect(pixelCanvas(page)).toHaveCount(1);
  await expect(imageCanvas(page)).toHaveCount(0);
  await expect(page.getByRole('menubar')).toHaveCount(1);
  expect(wasm.some((u) => u.includes('pincel_wasm_bg'))).toBe(true);
  expect(wasm.some((u) => u.includes('pincel_image_wasm'))).toBe(false);

  // An Image tab mounts the Image editor beside the Pixel one: one per
  // mode, only the active tab's visible, and only now the image wasm.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await expect(editorOf(page, 'image')).toHaveCount(1);
  await expect(editorOf(page, 'pixel')).toHaveCount(1);
  await expect(shownEditor(page)).toHaveCount(1);
  await expect(shownEditor(page)).toHaveAttribute('data-editor', 'image');
  await expect(editorOf(page, 'pixel')).toHaveAttribute('inert', '');
  await expect(imageCanvas(page)).toBeVisible();
  await expect(pixelCanvas(page)).toBeHidden();
  await expect(page.getByRole('menubar')).toHaveCount(1);
  expect(wasm.some((u) => u.includes('pincel_image_wasm'))).toBe(true);
});

// AC-14 (the Discard / Cancel guard before New…, Open… and Open Recent) is
// dropped: with tabs these open beside an unsaved document, proven in
// unsaved-prompt.spec.ts › "File ▸ New… of the other mode opens a tab
// without a prompt (AC-01)" and › "File ▸ Open… opens a tab of either mode
// without a prompt (AC-01)", and for Open Recent in shell-open.spec.ts.

for (const mode of ['pixel', 'image'] as const) {
  test(`AC-15: ${mode}: leaving the page asks only while there are unsaved changes`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 30, height: 20 });
    // A click gives the page the user activation Chromium wants before it
    // shows a leave-page prompt, so "no prompt" below means "not asked".
    await topBar(page).click();
    const asked: string[] = [];
    page.on('dialog', (d) => {
      asked.push(d.type());
      void d.dismiss();
    });
    const closed = new Promise<boolean>((resolve) => {
      page.once('close', () => resolve(true));
      setTimeout(() => resolve(false), 5_000);
    });
    await page.close({ runBeforeUnload: true });
    // Clean: the page closes and nothing asked.
    expect(await closed).toBe(true);
    expect(asked).toEqual([]);
  });

  test(`AC-15: ${mode}: unsaved changes trigger the leave-page confirmation`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 30, height: 20 });
    await edit(page, mode);
    const dialog = page.waitForEvent('dialog');
    await page.close({ runBeforeUnload: true });
    const d = await dialog;
    expect(d.type()).toBe('beforeunload');
    await d.dismiss();
    expect(page.isClosed()).toBe(false);
  });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`AC-16: ${mode}: the top bar shows mode, name and an unsaved marker until saved`, async ({
    page,
  }) => {
    // Save falls back to a download (no native save picker to drive).
    await page.addInitScript(() => {
      delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
    });
    await createDocument(page, mode, { width: 30, height: 20 });
    const name = mode === 'pixel' ? 'pincel.aseprite' : 'image.ora';
    const bar = topBar(page);
    const badge = mode === 'pixel' ? 'Pixel' : 'Image';
    await expect(bar.locator('span').filter({ hasText: new RegExp(`^${badge}$`) })).toBeVisible();
    await expect(bar.getByTitle(name, { exact: true })).toHaveText(name);
    await expect(bar.getByTitle(`${name} — unsaved changes`)).toHaveCount(0);

    await edit(page, mode);
    const marked = bar.getByTitle(`${name} — unsaved changes`, { exact: true });
    await expect(marked).toBeVisible();
    await expect(marked).toHaveText(new RegExp(`^${name.replace('.', '\\.')}\\s*●$`));

    const download = page.waitForEvent('download');
    await page.keyboard.press('ControlOrMeta+s');
    expect((await download).suggestedFilename()).toBe(name);
    await expect(marked).toHaveCount(0);
    await expect(bar.getByTitle(name, { exact: true })).toHaveText(name);
  });
}

test('AC-16: with several tabs the top bar follows the active tab: mode, name and its marker', async ({
  page,
}) => {
  // A dirty Image tab, then a clean Pixel tab opened beside it.
  await createDocument(page, 'image', { width: 16, height: 16 });
  await edit(page, 'image');
  await dropFiles(page, [await fileOf('pixel', 'a')]);
  await waitForEditor(page, 'pixel');
  await expectActive(page, 'a.aseprite');

  const bar = topBar(page);
  const badge = (text: string) =>
    bar.locator('span').filter({ hasText: new RegExp(`^${text}$`) });
  // The clean Pixel tab: its badge and name, no marker of the other tab.
  await expect(badge('Pixel')).toBeVisible();
  await expect(bar.getByTitle('a.aseprite', { exact: true })).toHaveText('a.aseprite');
  await expect(bar.getByTitle(/ — unsaved changes$/)).toHaveCount(0);
  await expect(bar.getByTitle('image.ora', { exact: true })).toHaveCount(0);

  // The dirty Image tab: its badge, name and marker.
  await tab(page, 'image.ora').click();
  await expectActive(page, 'image.ora');
  await expect(badge('Image')).toBeVisible();
  await expect(bar.getByTitle('image.ora — unsaved changes', { exact: true })).toBeVisible();
  await expect(bar.getByTitle('a.aseprite', { exact: true })).toHaveCount(0);
});

test('AC-18: a hidden sidebar and a collapsed panel come back on remount and reload', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  const sidebar = page.getByRole('complementary', { name: 'Panels' });
  const toggle = page.getByRole('button', { name: 'Toggle panels' });
  const layersHeader = sidebar
    .getByRole('region', { name: 'Layers' })
    .getByRole('button', { name: /^Layers/ });

  // Collapse Layers, then hide the docked sidebar.
  await layersHeader.click();
  await expect(layersHeader).toHaveAttribute('aria-expanded', 'false');
  await toggle.click();
  await expect(sidebar).toHaveCount(0);
  await expect
    .poll(() => storedPref(page, 'layout:image'))
    .toMatchObject({ sidebar: false, collapsed: { layers: true } });

  // The next Image editor of this session starts the same way.
  await menu(page, 'File', 'New…');
  await page.getByRole('dialog', { name: 'New document' }).getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await expect(sidebar).toHaveCount(0);
  await toggle.click();
  await expect(layersHeader).toHaveAttribute('aria-expanded', 'false');
  await toggle.click();
  await expect
    .poll(() => storedPref(page, 'layout:image'))
    .toMatchObject({ sidebar: false });

  // So does one after a reload, and Pixel mode keeps its own layout.
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect(sidebar).toHaveCount(0);
  await toggle.click();
  await expect(layersHeader).toHaveAttribute('aria-expanded', 'false');
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(sidebar).toBeVisible();
  await expect(
    sidebar.getByRole('region', { name: 'Layers' }).getByRole('button', { name: /^Layers/ }),
  ).toHaveAttribute('aria-expanded', 'true');
});

for (const mode of ['pixel', 'image'] as const) {
  test(`AC-20: ${mode}: a focused text field keeps tool keys, undo and paste`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await edit(page, mode);
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    const tools = page.getByRole('toolbar', { name: 'Tools' });
    const active = tools.locator('[aria-pressed="true"]');
    const before = await active.getAttribute('title');
    const layers = page.getByRole('button', { name: /^Hide / });

    const hex = page.getByLabel('Foreground color hex value');
    await hex.focus();
    await page.keyboard.press('e');
    await page.keyboard.press('ControlOrMeta+z');
    await hex.evaluate(async (el) => {
      const canvas = new OffscreenCanvas(8, 8);
      canvas.getContext('2d')?.fillRect(0, 0, 8, 8);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      const data = new DataTransfer();
      data.items.add(new File([blob], 'clip.png', { type: 'image/png' }));
      el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true }));
    });
    // Give a (wrongly) accepted key or paste time to land.
    await page.waitForTimeout(500);
    await expect(active).toHaveAttribute('title', before ?? '');
    await expect(undo).toBeEnabled();
    await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled();
    await expect(layers).toHaveCount(1);
  });

  test(`AC-20: ${mode}: the New document dialog keeps tool keys and undo`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await edit(page, mode);
    const active = page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
    const before = await active.getAttribute('title');
    await menu(page, 'File', 'New…');
    const dialog = page.getByRole('dialog', { name: 'New document' });
    // Focus on a dialog control that is not a text field.
    await dialog.getByRole('radio', { name: mode === 'pixel' ? /^Pixel sprite/ : /^Image/ }).focus();
    await page.keyboard.press('e');
    await page.keyboard.press('ControlOrMeta+z');
    await page.waitForTimeout(300);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(active).toHaveAttribute('title', before ?? '');
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
    await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled();
  });
}
