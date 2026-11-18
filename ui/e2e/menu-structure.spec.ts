import { expect, test, type Locator, type Page } from '@playwright/test';
import { es } from '../src/lib/i18n/es';
import { fr } from '../src/lib/i18n/fr';
import { ja } from '../src/lib/i18n/ja';
import { ptBr } from '../src/lib/i18n/pt-br';
import { createDocument, menu, menuHint, newDocument, switchLanguage } from './helpers';
import { installTauriMock, launchDesktop } from './tauri-mock';

// The menu bar of both editors (docs/specs/menu-structure.md): Tools for
// the desktop app's AI setup, Recognize Text… in Image, the probe in Help,
// Layer ▸ Transform, and one order for the entries both modes share.

const LINUX_UA =
  'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';
const MAC_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

type Mode = 'pixel' | 'image';
const MODES: readonly Mode[] = ['pixel', 'image'];
const SEP = '—';

const TRANSFORM = (mode: Mode): string[] => [
  'Flip Horizontal',
  'Flip Vertical',
  SEP,
  'Rotate 90° CW',
  'Rotate 90° CCW',
  'Rotate 180°',
  SEP,
  mode === 'pixel' ? 'Rotate…' : 'Rotate / Zoom…',
];

/** Titles of the menu bar, left to right. */
async function tops(page: Page): Promise<string[]> {
  const all = await page.getByRole('menubar').getByRole('menuitem').allTextContents();
  return all.map((s) => s.trim());
}

const ROWS =
  ':scope > [role="menuitem"], :scope > [role="menuitemcheckbox"], :scope > hr';

/** Labels of an open menu's rows, separators as `—`. */
async function rowLabels(menuEl: Locator): Promise<string[]> {
  return menuEl.locator(ROWS).evaluateAll((els) =>
    els.map((el) =>
      el.tagName === 'HR'
        ? '—'
        : (el.querySelector('.am-menu-label')?.textContent ?? '').trim(),
    ),
  );
}

async function closeMenus(page: Page): Promise<void> {
  while ((await page.getByRole('menu').count()) > 0) await page.keyboard.press('Escape');
}

/** Opens `top` (and the submenus of `path`) and returns the last menu's rows. */
async function rowsOf(page: Page, top: string, ...path: string[]): Promise<string[]> {
  await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
  let scope = top;
  for (const label of path) {
    await page
      .getByRole('menu', { name: scope, exact: true })
      .getByRole('menuitem', { name: label, exact: true })
      .click();
    scope = label;
  }
  const menuEl = page.getByRole('menu', { name: scope, exact: true });
  await expect(menuEl).toBeVisible();
  const out = await rowLabels(menuEl);
  await closeMenus(page);
  return out;
}

/** Every entry of the menu bar per top-level menu, as "Sub ▸ Item" paths in menu order. */
async function entriesByMenu(page: Page): Promise<Map<string, string[]>> {
  const out = new Map<string, string[]>();
  const walk = async (scope: string, path: string[], into: string[]): Promise<void> => {
    const menuEl = page.getByRole('menu', { name: scope, exact: true });
    await expect(menuEl).toBeVisible();
    const rows = menuEl.locator(':scope > [role="menuitem"], :scope > [role="menuitemcheckbox"]');
    const count = await rows.count();
    for (let i = 0; i < count; i += 1) {
      const row = rows.nth(i);
      const label = ((await row.locator('.am-menu-label').textContent()) ?? '').trim();
      into.push([...path, label].join(' ▸ '));
      if ((await row.getAttribute('aria-haspopup')) === 'menu' && !(await row.isDisabled())) {
        await row.click();
        await walk(label, [...path, label], into);
      }
    }
  };
  for (const top of await tops(page)) {
    await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
    const into: string[] = [];
    await walk(top, [], into);
    out.set(top, into);
    await closeMenus(page);
  }
  return out;
}

function paletteOption(page: Page, label: string): Locator {
  return page
    .getByRole('dialog', { name: 'Command Palette' })
    .getByRole('listbox')
    .getByRole('option')
    .filter({ has: page.getByText(label, { exact: true }) });
}

async function openPalette(page: Page): Promise<void> {
  await page.keyboard.press('Control+k');
  await expect(page.getByRole('dialog', { name: 'Command Palette' })).toBeVisible();
}

test.describe('browser', () => {
  for (const mode of MODES) {
    test(`${mode}: no Tools menu, menus in order, a View of view entries only (AC-02, AC-03, AC-11)`, async ({
      page,
    }) => {
      await createDocument(page, mode);
      expect(await tops(page)).toEqual(
        mode === 'pixel'
          ? ['File', 'Edit', 'Select', 'Image', 'Layer', 'Frame', 'Effects', 'Adjustments', 'View', 'Help']
          : ['File', 'Edit', 'Select', 'Image', 'Layer', 'Effects', 'Adjustments', 'View', 'Help'],
      );
      const aids =
        mode === 'pixel'
          ? ['Show Pixel Grid', 'Show Grid', 'Grid Settings…', 'Tiled Mode']
          : ['Rulers', 'Pixel Grid', 'Units'];
      expect(await rowsOf(page, 'View')).toEqual([
        'Zoom In',
        'Zoom Out',
        'Fit to Window',
        'Actual Size',
        SEP,
        ...aids,
        SEP,
        'Toggle UI',
      ]);
    });

    test(`${mode}: Select holds All, Deselect, Invert, then the radius commands (AC-10)`, async ({
      page,
    }) => {
      await createDocument(page, mode);
      expect(await rowsOf(page, 'Select')).toEqual([
        'Select All',
        'Deselect',
        'Invert Selection',
        SEP,
        'Expand…',
        'Contract…',
        ...(mode === 'image' ? ['Feather…'] : []),
      ]);
    });

    test(`${mode}: Layer ends with the Transform submenu; its entries keep their hints (AC-07, AC-08)`, async ({
      page,
    }) => {
      await createDocument(page, mode);
      const layer = await rowsOf(page, 'Layer');
      expect(layer.slice(-2)).toEqual([SEP, 'Transform']);
      for (const flat of TRANSFORM(mode).filter((l) => l !== SEP)) expect(layer).not.toContain(flat);
      expect(await rowsOf(page, 'Layer', 'Transform')).toEqual(TRANSFORM(mode));
      expect(await menuHint(page, 'Layer', 'Transform', 'Flip Horizontal')).toBe('Shift+H');
      expect(await menuHint(page, 'Layer', 'Transform', 'Flip Vertical')).toBe('Shift+V');
      expect(await menuHint(page, 'Layer', 'Transform', 'Rotate 180°')).toBe('');
    });
  }

  test('image: Image ends with Recognize Text…, which opens the OCR dialog (AC-04)', async ({
    page,
  }) => {
    await createDocument(page, 'image');
    expect((await rowsOf(page, 'Image')).slice(-2)).toEqual([SEP, 'Recognize Text…']);
    await menu(page, 'Image', 'Recognize Text…');
    await expect(page.getByRole('dialog', { name: 'Recognize Text' })).toBeVisible();
  });

  test('pixel: Help lists the Frame-Time Probe before About and toggles it (AC-05)', async ({
    page,
  }) => {
    await createDocument(page, 'pixel');
    const help = await rowsOf(page, 'Help');
    const at = help.indexOf('Frame-Time Probe');
    expect(at).toBeGreaterThan(0);
    expect(help.slice(at + 1, at + 3)).toEqual([SEP, 'About Pincel']);
    expect(help.slice(0, at)).not.toContain(SEP);
    const openHelp = () =>
      page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }).click();
    const item = page
      .getByRole('menu', { name: 'Help', exact: true })
      .getByRole('menuitemcheckbox', { name: 'Frame-Time Probe' });
    await openHelp();
    await expect(item).toHaveAttribute('aria-checked', 'false');
    await expect(item.locator('kbd')).toHaveText('F2');
    await item.click();
    await expect(page.getByText(/fps · compose/)).toBeVisible();
    await openHelp();
    await expect(item).toHaveAttribute('aria-checked', 'true');
    await item.click();
    await expect(page.getByText(/fps · compose/)).toBeHidden();
    await page.keyboard.press('F2');
    await expect(page.getByText(/fps · compose/)).toBeVisible();
  });

  test('image: Help has no Frame-Time Probe (AC-05)', async ({ page }) => {
    await createDocument(page, 'image');
    expect(await rowsOf(page, 'Help')).not.toContain('Frame-Time Probe');
  });

  test('entries both modes share stand in the same order (AC-09)', async ({ page }) => {
    await createDocument(page, 'pixel');
    const pixel = await entriesByMenu(page);
    await createDocument(page, 'image');
    const image = await entriesByMenu(page);
    let compared = 0;
    for (const [top, a] of pixel) {
      const b = image.get(top);
      if (!b) continue;
      const shared = (xs: string[], other: string[]) => xs.filter((x) => other.includes(x));
      expect(shared(a, b), top).toEqual(shared(b, a));
      compared += 1;
    }
    // File, Edit, Select, Image, Layer, Effects, Adjustments, View, Help.
    expect(compared).toBe(9);
  });

  test('the palette lists the moved commands under their new paths only (AC-12)', async ({
    page,
  }) => {
    await createDocument(page, 'image');
    await openPalette(page);
    await expect(paletteOption(page, 'Image ▸ Recognize Text…')).toHaveCount(1);
    await expect(paletteOption(page, 'Tools ▸ Recognize Text…')).toHaveCount(0);
    await expect(paletteOption(page, 'Layer ▸ Transform ▸ Flip Horizontal')).toHaveCount(1);
    await expect(paletteOption(page, 'Layer ▸ Transform ▸ Rotate / Zoom…')).toHaveCount(1);
    await expect(paletteOption(page, 'Layer ▸ Flip Horizontal')).toHaveCount(0);
    await expect(paletteOption(page, 'Tools ▸ AI Agents…')).toHaveCount(0);
    await expect(paletteOption(page, 'View ▸ AI Agents…')).toHaveCount(0);
    await page.keyboard.press('Escape');

    await createDocument(page, 'pixel');
    await openPalette(page);
    await expect(paletteOption(page, 'Help ▸ Frame-Time Probe')).toHaveCount(1);
    await expect(paletteOption(page, 'View ▸ Frame-Time Probe')).toHaveCount(0);
    await expect(paletteOption(page, 'Layer ▸ Transform ▸ Rotate…')).toHaveCount(1);
    await expect(paletteOption(page, 'Tools ▸ Image Generation…')).toHaveCount(0);
  });

  test('German: Extras and Transformieren; the other languages have their own Transform (AC-14)', async ({
    page,
  }) => {
    await createDocument(page, 'image');
    await switchLanguage(page, 'Deutsch');
    expect((await rowsOf(page, 'Ebene')).at(-1)).toBe('Transformieren');
    expect((await rowsOf(page, 'Ebene', 'Transformieren'))[0]).toBe('Horizontal spiegeln');
    expect((await rowsOf(page, 'Bild')).at(-1)).toBe('Text erkennen…');
    for (const text of [es, fr, ja, ptBr].map((m) => m['menu.transform'])) {
      expect(text.length).toBeGreaterThan(0);
      expect(text).not.toBe('Transform');
    }
  });

  test.describe('phone', () => {
    test.use({ viewport: { width: 390, height: 780 } });

    for (const mode of MODES) {
      test(`${mode}: the ☰ sheet lists the menus and entries of the menu bar (AC-13)`, async ({
        page,
      }) => {
        await page.setViewportSize({ width: 1280, height: 800 });
        await createDocument(page, mode);
        const wide = new Map<string, string[]>();
        for (const top of await tops(page)) wide.set(top, await rowsOf(page, top));

        await page.setViewportSize({ width: 390, height: 780 });
        await page.getByRole('button', { name: 'Menu', exact: true }).click();
        const sheet = page.getByRole('menu', { name: 'Menu', exact: true });
        await expect(sheet).toBeVisible();
        const blocks = sheet.locator(':scope > div.relative');
        const narrow = new Map<string, string[]>();
        for (let i = 0; i < (await blocks.count()); i += 1) {
          const block = blocks.nth(i);
          const title = ((await block.locator(':scope > div').first().textContent()) ?? '').trim();
          narrow.set(title, await rowLabels(block));
        }
        expect([...narrow.keys()]).toEqual([...wide.keys()]);
        for (const [top, rows] of wide) expect(narrow.get(top), top).toEqual(rows);

        const transform = sheet.getByRole('menuitem', { name: 'Transform', exact: true });
        await transform.click();
        await expect(transform).toHaveAttribute('aria-expanded', 'true');
        await expect(sheet.getByRole('menuitem', { name: 'Flip Horizontal' }).last()).toBeVisible();
      });
    }
  });
});

test.describe('desktop', () => {
  test.use({ userAgent: LINUX_UA });
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  for (const mode of MODES) {
    test(`${mode}: Tools follows View and holds the two AI entries (AC-01, AC-03, AC-12)`, async ({
      page,
    }) => {
      await launchDesktop(page);
      await newDocument(page, mode);
      const bar = await tops(page);
      expect(bar.slice(bar.indexOf('View'))).toEqual(['View', 'Tools', 'Help']);
      expect(await rowsOf(page, 'Tools')).toEqual(['AI Agents…', 'Image Generation…']);
      expect((await rowsOf(page, 'View')).at(-1)).toBe('Toggle UI');

      await menu(page, 'Tools', 'AI Agents…');
      const agents = page.getByRole('dialog', { name: 'AI agents' });
      await expect(agents).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(agents).toBeHidden();

      await openPalette(page);
      await expect(paletteOption(page, 'Tools ▸ AI Agents…')).toHaveCount(1);
      await expect(paletteOption(page, 'Tools ▸ Image Generation…')).toHaveCount(1);
      await expect(paletteOption(page, 'View ▸ AI Agents…')).toHaveCount(0);
      await expect(paletteOption(page, 'View ▸ Image Generation…')).toHaveCount(0);
    });

    test(`${mode}: Tools is inert while an effect is open (AC-15)`, async ({ page }) => {
      await launchDesktop(page);
      await newDocument(page, mode);
      const tools = page.locator('[data-menu-trigger="tools"]');
      await expect(tools).toBeVisible();
      expect(await tools.evaluate((el) => el.closest('[inert]') !== null)).toBe(false);
      await menu(page, 'Adjustments', 'Brightness / Contrast…');
      await expect(page.getByRole('menubar')).toBeVisible();
      expect(await tools.evaluate((el) => el.closest('[inert]') !== null)).toBe(true);
    });
  }
});

test.describe('desktop on macOS', () => {
  test.use({ userAgent: MAC_UA });
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  // The window draws no menus there; the native bar mirrors them, Help with
  // the probe included (docs/specs/macos-native-menu.md AC-01, AC-08).
  test('pixel: no in-window menus; F2 and the palette reach the probe under Help (AC-06)', async ({
    page,
  }) => {
    await launchDesktop(page);
    await newDocument(page, 'pixel');
    await expect(page.getByRole('menubar')).toHaveCount(0);

    await page.keyboard.press('F2');
    await expect(page.getByText(/fps · compose/)).toBeVisible();
    await page.keyboard.press('F2');
    await expect(page.getByText(/fps · compose/)).toBeHidden();

    await openPalette(page);
    const probe = paletteOption(page, 'Help ▸ Frame-Time Probe');
    await expect(probe).toHaveCount(1);
    await probe.click();
    await expect(page.getByText(/fps · compose/)).toBeVisible();
  });
});
