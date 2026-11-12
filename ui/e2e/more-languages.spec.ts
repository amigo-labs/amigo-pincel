import { expect, test, type Locator, type Page } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';
import { es } from '../src/lib/i18n/es';
import { fr } from '../src/lib/i18n/fr';
import { ja } from '../src/lib/i18n/ja';
import { LOCALES, pickLocale, type Locale } from '../src/lib/i18n/locales';
import { ptBr } from '../src/lib/i18n/pt-br';
import {
  clearClipboard,
  createDocument,
  gotoStart,
  imageCanvas,
  putClipboardImage,
} from './helpers';

// Six UI languages (docs/specs/more-languages.md). The dictionaries and the
// locale match are rune-free, so the tests read the expected strings from
// them.

const DICTS = { en, de, es, ja, fr, 'pt-BR': ptBr } as const;

test('the browser language picks a locale: full tag, base language, Portuguese as pt-BR (AC-03)', () => {
  expect(pickLocale(['es-MX'])).toBe('es');
  expect(pickLocale(['ja-JP'])).toBe('ja');
  expect(pickLocale(['fr-CA'])).toBe('fr');
  expect(pickLocale(['pt'])).toBe('pt-BR');
  expect(pickLocale(['pt-PT'])).toBe('pt-BR');
  expect(pickLocale(['pt-br'])).toBe('pt-BR');
  expect(pickLocale(['de-AT', 'en'])).toBe('de');
  // The first entry that matches wins; none → English.
  expect(pickLocale(['it-IT', 'fr', 'de'])).toBe('fr');
  expect(pickLocale(['it-IT', 'nl'])).toBe('en');
  expect(pickLocale([])).toBe('en');
});

for (const [browser, expected] of [
  ['es-MX', 'es'],
  ['ja-JP', 'ja'],
  ['fr-CA', 'fr'],
  ['pt-PT', 'pt-BR'],
] as const) {
  test.describe(`a ${browser} browser`, () => {
    test.use({ locale: browser });

    test(`starts in ${expected} with <html lang="${expected}"> (AC-03, AC-04)`, async ({
      page,
    }) => {
      await page.goto('./');
      await expect(
        page.getByRole('button', { name: DICTS[expected]['start.newPixel'] }),
      ).toBeVisible();
      await expect(page.locator('html')).toHaveAttribute('lang', expected);
    });
  });
}

/** The start screen's language picker, by its label in `loc`. */
const picker = (page: Page, loc: Locale) =>
  page.getByLabel(DICTS[loc]['start.language'], { exact: true });

test('the picker lists six languages in their own names; each switches at once and survives a reload (AC-01, AC-02, AC-04)', async ({
  page,
}) => {
  await gotoStart(page);
  await expect(picker(page, 'en').locator('option')).toHaveText(LOCALES.map((l) => l.label));
  let current: Locale = 'en';
  for (const { id } of LOCALES) {
    await picker(page, current).selectOption(id);
    current = id;
    await expect(page.getByRole('button', { name: DICTS[id]['start.newPixel'] })).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('lang', id);
  }
  // The choice is stored: a reload keeps the last one.
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          new Promise<unknown>((resolve) => {
            const req = indexedDB.open('pincel');
            req.onsuccess = () => {
              const tx = req.result.transaction('prefs', 'readonly');
              const get = tx.objectStore('prefs').get('language');
              get.onsuccess = () => resolve(get.result?.value ?? get.result);
              get.onerror = () => resolve(null);
            };
            req.onerror = () => resolve(null);
          }),
      ),
    )
    .toBe(current);
  await page.reload();
  await expect(page.getByRole('button', { name: DICTS[current]['start.newPixel'] })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', current);
});

test('French writes the brush opacity as a French percentage (AC-09)', async ({ page }) => {
  await gotoStart(page);
  await picker(page, 'en').selectOption('fr');
  await page.getByRole('button', { name: fr['start.newImage'] }).click();
  await page
    .getByRole('form', { name: fr['start.newImage'] })
    .getByRole('button', { name: fr['common.create'] })
    .click();
  await expect(imageCanvas(page)).toHaveAttribute('data-ready', 'true');
  await page.keyboard.press('b');
  await expect(page.getByText(/^100[\u00a0\u202f]%$/).first()).toBeVisible();
});

/** The layer names the Layers panel shows in `loc`, read off the grips. */
async function layerNames(page: Page, loc: Locale): Promise<string[]> {
  const dict = DICTS[loc];
  const [before, after] = dict['layers.move'].split('{name}');
  const labels = await page
    .getByRole('list', { name: dict['layers.list'], exact: true })
    .getByRole('button')
    .evaluateAll((els) => els.map((el) => el.getAttribute('aria-label') ?? ''));
  return labels
    .filter((l) => l.startsWith(before ?? '') && l.endsWith(after ?? ''))
    .map((l) => l.slice((before ?? '').length, l.length - (after ?? '').length));
}

test('layer names stay as stored in every language (AC-12)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  const stored = await layerNames(page, 'en');
  expect(stored.length).toBeGreaterThan(0);
  let current: Locale = 'en';
  for (const { id, label } of LOCALES.filter((l) => l.id !== 'en')) {
    await page.keyboard.press('Control+,');
    const dialog = page.getByRole('dialog', { name: DICTS[current]['settings.title'] });
    await dialog
      .getByRole('combobox', { name: DICTS[current]['settings.language'] })
      .selectOption({ label });
    await page
      .getByRole('dialog', { name: DICTS[id]['settings.title'] })
      .getByRole('button', { name: DICTS[id]['common.close'], exact: true })
      .click();
    current = id;
    await expect.poll(() => layerNames(page, id)).toEqual(stored);
  }
});

/**
 * Text elements under `root` whose text does not fit: content wider than
 * the box that either spills out visibly or is cut without its full text as
 * a `title` (on the element or an ancestor). Inline elements flow and are
 * skipped; so is anything hidden.
 */
async function unfit(root: Locator): Promise<string[]> {
  // A part the layout does not show at this width (the menu bar folds into
  // a menu button on a phone) has nothing to clip.
  if ((await root.count()) === 0) return [];
  return root.first().evaluate((rootEl) => {
    const out: string[] = [];
    const sel = 'button, label, span, a, p, h1, h2, h3, legend, [role="menuitem"], [role="tab"]';
    for (const el of rootEl.querySelectorAll<HTMLElement>(sel)) {
      const cs = getComputedStyle(el);
      if (cs.display === 'none' || cs.display === 'inline' || cs.visibility === 'hidden') continue;
      if (el.getClientRects().length === 0) continue;
      const text = (el.textContent ?? '').trim();
      if (text === '') continue;
      if (el.scrollWidth <= el.clientWidth + 1) continue;
      const cut = cs.overflowX !== 'visible' || cs.textOverflow === 'ellipsis';
      const titled = el.closest('[title]')?.getAttribute('title')?.includes(text) ?? false;
      if (!cut) out.push(`spills: ${text}`);
      else if (!titled) out.push(`cut without title: ${text}`);
    }
    return out;
  });
}

for (const [width, height] of [
  [1280, 800],
  [390, 844],
] as const) {
  for (const { id } of LOCALES) {
    test(`at ${width} px ${id} fits or shows its full text as a tooltip (AC-10)`, async ({
      page,
      context,
    }) => {
      await context.grantPermissions(['clipboard-read', 'clipboard-write']);
      await page.setViewportSize({ width, height });
      const d = DICTS[id];
      // The New dialog takes its size from a clipboard image, and the
      // clipboard outlives a test: an earlier test's 4000 × 20 image would
      // make the canvas as large as the paste.
      await gotoStart(page);
      await clearClipboard(page);
      const problems: string[] = [];
      const note = (where: string, found: string[]) => {
        for (const f of found) problems.push(`${where}: ${f}`);
      };
      await gotoStart(page);
      await page.getByRole('combobox').first().selectOption(id);
      await expect(page.locator('html')).toHaveAttribute('lang', id);
      note('start screen', await unfit(page.locator('body')));
      await page.getByRole('button', { name: d['start.newImage'] }).click();
      const form = page.getByRole('form', { name: d['start.newImage'] });
      note('New dialog', await unfit(form));
      await form.getByRole('button', { name: d['common.create'] }).click();
      await expect(imageCanvas(page)).toHaveAttribute('data-ready', 'true');
      note('menu bar', await unfit(page.getByRole('menubar')));
      for (const key of ['b', 'm', 'v', 'u', 't'] as const) {
        await page.keyboard.press(key);
        note(
          `options (${key})`,
          await unfit(page.getByRole('toolbar', { name: d['frame.toolOptions'] })),
        );
      }
      await putClipboardImage(page, 4000, 20, '#0000ff');
      // The selection tool: a click with Text would open a text box that
      // takes the paste.
      await page.keyboard.press('m');
      await imageCanvas(page).click();
      await page.keyboard.press('Control+v');
      const paste = page.getByRole('dialog', { name: d['paste.largerTitle'] });
      await expect(paste).toBeVisible();
      note('paste dialog', await unfit(paste));
      expect(problems).toEqual([]);
    });
  }
}
