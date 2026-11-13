import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  waitForEditor,
} from './helpers';

// UI language (spec §9.5): browser language by default, switchable on the
// start screen and in Edit ▸ Settings… (docs/specs/settings-dialog.md),
// remembered across reloads.

test.describe('German browser', () => {
  test.use({ locale: 'de-DE' });

  test('starts in German', async ({ page }) => {
    await page.goto('./');
    await expect(page.getByRole('button', { name: 'Neues Pixel-Sprite' })).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  });
});

test('start-screen switch persists across reloads', async ({ page }) => {
  await gotoStart(page);
  await page.getByLabel('Language').selectOption('de');
  await expect(page.getByRole('button', { name: 'Neues Bild' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Neues Bild' })).toBeVisible();
  await page.getByLabel('Sprache').selectOption('en');
  await expect(page.getByRole('button', { name: 'New image' })).toBeVisible();
});

test('Settings ▸ Language switches the editor', async ({ page }) => {
  await createDocument(page, 'pixel');
  await switchLanguage(page, 'en', 'Deutsch');
  await expect(page.getByRole('button', { name: 'Vergrößern' })).toBeVisible();
  const bar = page.getByRole('menubar');
  const menus = [
    'Datei',
    'Bearbeiten',
    'Auswahl',
    'Bild',
    'Ebene',
    'Frame',
    'Effekte',
    'Ansicht',
    'Hilfe',
  ];
  for (const name of menus) {
    await expect(bar.getByRole('menuitem', { name, exact: true })).toBeVisible();
  }
  await expect(page.getByRole('region', { name: 'Ebenen' })).toBeVisible();
  await bar.getByRole('menuitem', { name: 'Ebene', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: 'Umbenennen…' })).toBeVisible();
});

const MODES = ['pixel', 'image'] as const;

/** Settings dialog, Language field and Close titles per UI language. */
const UI = {
  en: { dialog: 'Settings', language: 'Language', close: 'Close' },
  de: { dialog: 'Einstellungen', language: 'Sprache', close: 'Schließen' },
} as const;

/** Opens Edit ▸ Settings… (Ctrl+,) with the titles of UI language `lang`;
 *  returns its Language field. */
async function languageField(page: Page, lang: keyof typeof UI): Promise<Locator> {
  await page.keyboard.press('Control+,');
  return page
    .getByRole('dialog', { name: UI[lang].dialog, exact: true })
    .getByRole('combobox', { name: UI[lang].language, exact: true });
}

/** Closes the Settings dialog, titled in UI language `lang`. */
async function closeSettings(page: Page, lang: keyof typeof UI): Promise<void> {
  const dialog = page.getByRole('dialog', { name: UI[lang].dialog, exact: true });
  await dialog.getByRole('button', { name: UI[lang].close, exact: true }).click();
  await expect(dialog).toBeHidden();
}

/** Settings ▸ Language ▸ `pick`, opened with the titles of UI language `lang`. */
async function switchLanguage(
  page: Page,
  lang: keyof typeof UI,
  pick: 'English' | 'Deutsch',
): Promise<void> {
  await (await languageField(page, lang)).selectOption({ label: pick });
  await closeSettings(page, pick === 'Deutsch' ? 'de' : 'en');
}

/** Marks the page so a later `expectNoReload` can tell it was not reloaded. */
async function markPage(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as { i18nMark?: boolean }).i18nMark = true;
  });
}

async function expectNoReload(page: Page): Promise<void> {
  expect(await page.evaluate(() => (window as { i18nMark?: boolean }).i18nMark)).toBe(true);
}

test.describe('Italian browser', () => {
  test.use({ locale: 'it-IT' });

  test('starts in English when the browser asks for no supported language (AC-02)', async ({
    page,
  }) => {
    await page.goto('./');
    const langs = await page.evaluate(() => [...navigator.languages]);
    expect(langs.some((l) => /^(de|en|es|ja|fr|pt)\b/i.test(l))).toBe(false);
    await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  });
});

/** Per mode: a tool of the rail in English and German. */
const TOOL = { pixel: ['Pencil', 'Stift'], image: ['Brush', 'Pinsel'] } as const;

for (const mode of MODES) {
  test(`${mode}: Settings ▸ Language switches menus, panels, tools and buttons at once and shows the active language (AC-04)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await markPage(page);
    const bar = page.getByRole('menubar');
    const tools = (name: string) =>
      page.getByRole('toolbar').getByRole('button', { name, exact: true });

    let field = await languageField(page, 'en');
    await expect(field).toHaveValue('en');
    await field.selectOption({ label: 'Deutsch' });
    await closeSettings(page, 'de');

    for (const name of ['Datei', 'Bearbeiten', 'Auswahl', 'Effekte', 'Korrekturen', 'Ansicht']) {
      await expect(bar.getByRole('menuitem', { name, exact: true })).toBeVisible();
    }
    await expect(page.getByRole('region', { name: 'Ebenen' })).toBeVisible();
    await expect(tools(TOOL[mode][1])).toBeVisible();
    await expect(page.getByRole('button', { name: 'Rückgängig', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Vergrößern', exact: true })).toBeVisible();
    await expect(bar.getByRole('menuitem', { name: 'File', exact: true })).toHaveCount(0);
    await expectNoReload(page);

    field = await languageField(page, 'de');
    await expect(field).toHaveValue('de');
    await field.selectOption({ label: 'English' });
    await closeSettings(page, 'en');

    for (const name of ['File', 'Edit', 'Select', 'Effects', 'Adjustments', 'View']) {
      await expect(bar.getByRole('menuitem', { name, exact: true })).toBeVisible();
    }
    await expect(page.getByRole('region', { name: 'Layers' })).toBeVisible();
    await expect(tools(TOOL[mode][0])).toBeVisible();
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Zoom in', exact: true })).toBeVisible();
    await expect(bar.getByRole('menuitem', { name: 'Datei', exact: true })).toHaveCount(0);
    await expectNoReload(page);
    field = await languageField(page, 'en');
    await expect(field).toHaveValue('en');
    await closeSettings(page, 'en');
  });

  test(`${mode}: switching the language keeps the document, its unsaved state and its history (AC-06)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const canvas = mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
    const shot = async () => {
      await page.mouse.move(1, 1);
      return canvas.screenshot();
    };
    const blank = await shot();
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(page, canvas, [0.2, 0.2], [0.8, 0.8]);
    await expect.poll(async () => (await shot()).equals(blank)).toBe(false);
    const painted = await shot();
    await expect(page.getByTitle(/ — unsaved changes$/)).toBeVisible();
    if (mode === 'pixel') await expect(page.getByText('undo 1 / redo 0')).toBeVisible();

    await switchLanguage(page, 'en', 'Deutsch');
    await expect(page.getByRole('menubar').getByRole('menuitem', { name: 'Datei' })).toBeVisible();
    expect((await shot()).equals(painted)).toBe(true);
    await expect(page.getByTitle(/ – ungespeicherte Änderungen$/)).toBeVisible();
    if (mode === 'pixel') await expect(page.getByText('rückg. 1 / wiederh. 0')).toBeVisible();

    // The history is the one from before the switch: one step, the stroke.
    const undo = page.getByRole('button', { name: 'Rückgängig', exact: true });
    await undo.click();
    await expect(undo).toBeDisabled();
    await expect.poll(async () => (await shot()).equals(blank)).toBe(true);
    await expect(page.getByTitle(/ – ungespeicherte Änderungen$/)).toHaveCount(0);
  });
}

/** Layer names in the layers panel, top-most first, read from the drag grips. */
async function layerNames(page: Page, lang: 'en' | 'de'): Promise<string[]> {
  const [list, grip] =
    lang === 'en'
      ? (['Layer list', /^Drag to reorder (.+)$/] as const)
      : (['Ebenenliste', /^(.+) zum Umsortieren ziehen$/] as const);
  const labels = await page
    .getByRole('list', { name: list, exact: true })
    .getByRole('button', { name: grip })
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
  return labels.map((label) => grip.exec(label)?.[1] ?? label);
}

test('pixel: layer, tag and slice names stay untranslated in German (AC-08)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await switchLanguage(page, 'en', 'Deutsch');
  const layers = page.getByRole('list', { name: 'Ebenenliste', exact: true });
  await expect(layers.getByRole('button', { name: 'Layer 1', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Ebene hinzufügen', exact: true }).click();
  await expect(layers.getByRole('button', { name: 'Layer 2', exact: true })).toBeVisible();
  await expect(layers.getByRole('button', { name: 'Layer 1', exact: true })).toBeVisible();

  const timeline = page.getByRole('region', { name: 'Zeitleiste' });
  await timeline.getByRole('button', { name: 'Tag hinzufügen' }).click();
  await expect(timeline.getByRole('button', { name: 'Tag tag1', exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Slice', exact: true }).click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.6, 0.6]);
  const slices = page.getByRole('region', { name: 'Slices' });
  await slices.getByRole('button', { name: 'Slices', exact: true }).click();
  await expect(
    slices.getByRole('button', { name: 'Slice Slice 1 aktivieren', exact: true }),
  ).toBeVisible();
});

/** A `w` × `h` opaque PNG, encoded in the page (test side, not the app). */
async function pngFile(page: Page, w: number, h: number): Promise<Buffer> {
  const b64 = await page.evaluate(
    async ([width, height]) => {
      const canvas = new OffscreenCanvas(width, height);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = '#0088ff';
      ctx.fillRect(0, 0, width, height);
      const bytes = new Uint8Array(await (await canvas.convertToBlob()).arrayBuffer());
      let s = '';
      for (const b of bytes) s += String.fromCharCode(b);
      return btoa(s);
    },
    [w, h] as const,
  );
  return Buffer.from(b64, 'base64');
}

test('image: file, layer and format names stay untranslated in German (AC-08)', async ({
  page,
}) => {
  // The <input type="file"> fallback instead of the File System Access picker.
  await page.addInitScript(() => {
    delete (window as { showOpenFilePicker?: unknown }).showOpenFilePicker;
  });
  await gotoStart(page);
  const png = await pngFile(page, 8, 8);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({ name: 'Layer.png', mimeType: 'image/png', buffer: png });
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('Layer.png', { exact: true })).toBeVisible();
  const before = await layerNames(page, 'en');
  expect(before.length).toBeGreaterThan(0);

  await switchLanguage(page, 'en', 'Deutsch');
  await expect(page.getByTitle('Layer.png', { exact: true })).toBeVisible();
  expect(await layerNames(page, 'de')).toEqual(before);
  await page.getByRole('button', { name: 'Ebene hinzufügen', exact: true }).click();
  await expect
    .poll(() => layerNames(page, 'de'))
    .toEqual([`Layer ${before.length + 1}`, ...before]);

  await menu(page, 'Datei', 'Exportieren');
  const exportMenu = page.getByRole('menu', { name: 'Exportieren', exact: true });
  await expect(exportMenu.getByRole('menuitem', { name: 'PNG', exact: true })).toBeVisible();
  await expect(exportMenu.getByRole('menuitem', { name: 'JPEG…', exact: true })).toBeVisible();
});

/** The Settings dialog's languages, in order, by their labels. */
async function expectLanguageItems(field: Locator): Promise<void> {
  await expect(field.locator('option')).toHaveText([
    'English',
    'Deutsch',
    'Español',
    '日本語',
    'Français',
    'Português (Brasil)',
  ]);
}

test('both pickers label each language in its own language (AC-09)', async ({ page }) => {
  await gotoStart(page);
  await expect(page.getByLabel('Language').locator('option')).toHaveText([
    'English',
    'Deutsch',
    'Español',
    '日本語',
    'Français',
    'Português (Brasil)',
  ]);
  await page.getByLabel('Language').selectOption('de');
  await expect(page.getByLabel('Sprache').locator('option')).toHaveText([
    'English',
    'Deutsch',
    'Español',
    '日本語',
    'Français',
    'Português (Brasil)',
  ]);
  await page.getByLabel('Sprache').selectOption('en');

  await createDocument(page, 'pixel', { width: 16, height: 16 });
  let field = await languageField(page, 'en');
  await expectLanguageItems(field);
  await field.selectOption({ label: 'Deutsch' });
  await closeSettings(page, 'de');
  field = await languageField(page, 'de');
  await expectLanguageItems(field);
});

/** Init scripts that take IndexedDB away: failing on open, or missing. */
const BROKEN_IDB: [string, () => void][] = [
  [
    'failing',
    () => {
      IDBFactory.prototype.open = () => {
        throw new DOMException('IndexedDB is blocked', 'UnknownError');
      };
    },
  ],
  [
    'unavailable',
    () => {
      Object.defineProperty(window, 'indexedDB', { value: undefined, configurable: true });
    },
  ],
];

test.describe('German browser, broken IndexedDB', () => {
  test.use({ locale: 'de-DE' });

  for (const [kind, breakIdb] of BROKEN_IDB) {
    test(`IndexedDB ${kind}: the browser language wins and switching still works (AC-11)`, async ({
      page,
    }) => {
      await page.addInitScript(breakIdb);
      await page.goto('./');
      await expect(page.getByRole('button', { name: 'Neues Pixel-Sprite' })).toBeVisible();
      await expect(page.locator('html')).toHaveAttribute('lang', 'de');

      await page.getByLabel('Sprache').selectOption('en');
      await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeVisible();
      await expect(page.locator('html')).toHaveAttribute('lang', 'en');
      // Into an editor without a reload: the choice lives for this session.
      await page.getByRole('button', { name: 'New pixel sprite' }).click();
      const card = page.getByRole('form', { name: 'New pixel sprite' });
      await card.getByRole('button', { name: 'Create' }).click();
      await waitForEditor(page, 'pixel');
      await expect(page.getByRole('menubar').getByRole('menuitem', { name: 'File' })).toBeVisible();
      await switchLanguage(page, 'en', 'Deutsch');
      await expect(
        page.getByRole('menubar').getByRole('menuitem', { name: 'Datei' }),
      ).toBeVisible();
    });
  }
});
