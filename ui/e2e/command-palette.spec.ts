import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menuHint, pixelCanvas, storedPref } from './helpers';

// The command palette, Ctrl+K (docs/specs/power-user-shortcuts-palette.md):
// a search over every menu command and tool of the current mode.

function palette(page: Page, name = 'Command Palette'): Locator {
  return page.getByRole('dialog', { name });
}

function search(page: Page): Locator {
  return palette(page).getByRole('combobox');
}

function options(page: Page): Locator {
  return palette(page).getByRole('listbox').getByRole('option');
}

/** The option whose label reads exactly `label` (its shortcut is a separate `<kbd>`). */
function option(page: Page, label: string): Locator {
  return options(page).filter({ has: page.getByText(label, { exact: true }) });
}

/** Labels of the listed options, in order. */
async function labels(page: Page): Promise<string[]> {
  return options(page).evaluateAll((els) =>
    els.map((el) => el.querySelector('span')?.textContent?.trim() ?? ''),
  );
}

async function openPalette(page: Page): Promise<void> {
  await page.keyboard.press('Control+k');
  await expect(palette(page)).toBeVisible();
}

/** Every command of the menu bar as "Menu ▸ Submenu ▸ Item", read by opening each menu. */
async function menuPaths(page: Page): Promise<string[]> {
  const out: string[] = [];
  const tops = await page.getByRole('menubar').getByRole('menuitem').allTextContents();
  const walk = async (scope: string, path: string[]): Promise<void> => {
    const menuEl = page.getByRole('menu', { name: scope, exact: true });
    await expect(menuEl).toBeVisible();
    const rows = menuEl.locator(':scope > [role="menuitem"], :scope > [role="menuitemcheckbox"]');
    const count = await rows.count();
    for (let i = 0; i < count; i += 1) {
      const row = rows.nth(i);
      const label = ((await row.locator('span.flex-1').textContent()) ?? '').trim();
      if ((await row.getAttribute('aria-haspopup')) === 'menu') {
        // A disabled submenu (an empty Open Recent) has nothing to list.
        if (await row.isDisabled()) continue;
        await row.click();
        await walk(label, [...path, label]);
      } else {
        out.push([...path, label].join(' ▸ '));
      }
    }
  };
  for (const raw of tops) {
    const top = raw.trim();
    await page.getByRole('menubar').getByRole('menuitem', { name: top, exact: true }).click();
    await walk(top, [top]);
    while ((await page.getByRole('menu').count()) > 0) await page.keyboard.press('Escape');
  }
  return out;
}

function layerCount(page: Page): Promise<number> {
  return page
    .getByRole('list', { name: 'Layer list' })
    .getByRole('button', { name: /^Drag to reorder / })
    .count();
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: Ctrl+K opens the palette with every command and tool; Escape closes it (AC-01, AC-02, AC-03, AC-14)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await openPalette(page);
    await expect(search(page)).toBeFocused();
    await expect(option(page, 'Layer ▸ Merge Down')).toHaveCount(1);
    await expect(option(page, 'Image ▸ Flip Horizontal')).toHaveCount(1);
    await expect(option(page, 'Layer ▸ Flip Horizontal')).toHaveCount(1);
    await expect(option(page, 'Tool: Magic Wand')).toHaveCount(1);
    await expect(option(page, 'Go to frame…')).toHaveCount(mode === 'pixel' ? 1 : 0);

    const layers = await layerCount(page);
    await page.keyboard.press('Escape');
    await expect(palette(page)).toBeHidden();
    expect(await layerCount(page)).toBe(layers);
  });

  test(`${mode} mode: the palette lists exactly the menu bar's commands plus the tools (AC-02)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    const fromMenus = await menuPaths(page);
    expect(fromMenus.length).toBeGreaterThan(30);
    const tools = await page
      .getByRole('toolbar', { name: 'Tools' })
      .getByRole('button')
      .evaluateAll((els) =>
        // "Group: Tool" for a group of several tools, else the tool's name.
        els.map((el) => (el.getAttribute('aria-label') ?? '').split(': ').at(-1) ?? ''),
      );
    await openPalette(page);
    const listed = await labels(page);
    const commands = listed.filter((l) => !l.startsWith('Tool: ') && l !== 'Go to frame…');
    expect([...commands].sort()).toEqual([...fromMenus].sort());
    for (const tool of tools) expect(listed).toContain(`Tool: ${tool}`);
  });

  test(`${mode} mode: a text field keeps Ctrl+K`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.getByLabel('Foreground color hex value').focus();
    await page.keyboard.press('Control+k');
    await expect(palette(page)).toHaveCount(0);
  });
}

test('typing filters fuzzily, Enter runs the highlighted command, one Undo restores (AC-04, AC-06, AC-07, AC-15)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press('Shift+N');
  await expect.poll(() => layerCount(page)).toBe(2);
  const hint = await menuHint(page, 'Layer', 'Merge Down');

  await openPalette(page);
  await expect(palette(page)).toHaveAttribute('aria-modal', 'true');
  await search(page).fill('mrgdwn');
  expect((await labels(page))[0]).toBe('Layer ▸ Merge Down');
  await search(page).fill('merge');
  expect((await labels(page))[0]).toBe('Layer ▸ Merge Down');
  expect(await labels(page)).not.toContain('Layer ▸ New Layer');

  // The combobox announces the highlighted option, which carries the menu's hint.
  const first = options(page).first();
  await expect(first).toHaveAttribute('aria-selected', 'true');
  await expect(search(page)).toHaveAttribute(
    'aria-activedescendant',
    (await first.getAttribute('id')) ?? '',
  );
  await expect(search(page)).toHaveAttribute(
    'aria-controls',
    (await palette(page).getByRole('listbox').getAttribute('id')) ?? '',
  );
  await expect(first.locator('kbd')).toHaveText(hint);

  // ↓ / ↑ move the highlight and stop at the first entry.
  await page.keyboard.press('ArrowDown');
  await expect(options(page).nth(1)).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await expect(first).toHaveAttribute('aria-selected', 'true');

  await page.keyboard.press('Enter');
  await expect(palette(page)).toBeHidden();
  await expect.poll(() => layerCount(page)).toBe(1);
  await page.keyboard.press('Control+z');
  await expect.poll(() => layerCount(page)).toBe(2);
});

test('a click runs the command, a tool entry selects the tool (AC-08, AC-10)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await openPalette(page);
  await search(page).fill('new layer');
  await option(page, 'Layer ▸ New Layer').click();
  await expect(palette(page)).toBeHidden();
  await expect.poll(() => layerCount(page)).toBe(2);

  await openPalette(page);
  await search(page).fill('wand');
  await option(page, 'Tool: Magic Wand').click();
  await expect(palette(page)).toBeHidden();
  await expect(
    page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]'),
  ).toHaveAttribute('aria-label', /Magic Wand/);
});

test('a disabled command is dimmed and does nothing; no match runs nothing (AC-09, AC-16)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openPalette(page);
  await search(page).fill('merge down');
  const merge = option(page, 'Layer ▸ Merge Down');
  await expect(merge).toHaveAttribute('aria-disabled', 'true');
  await expect(options(page).first()).toHaveAttribute('aria-disabled', 'true');
  await page.keyboard.press('Enter');
  await expect(palette(page)).toBeVisible();
  // `force`: Playwright won't click an aria-disabled element by itself.
  await merge.click({ force: true });
  await expect(palette(page)).toBeVisible();
  expect(await layerCount(page)).toBe(1);

  await search(page).fill('zzqqxx');
  await expect(options(page)).toHaveCount(0);
  await page.keyboard.press('Enter');
  await expect(palette(page)).toBeVisible();
  expect(await layerCount(page)).toBe(1);
});

test('Go to frame… jumps to a frame in range and ignores anything else (AC-11, AC-12)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.keyboard.press('Alt+n');
  await page.keyboard.press('Alt+n');
  const current = page.locator('button[aria-current="true"][aria-label^="Frame "]');
  await expect(current).toHaveAttribute('aria-label', 'Frame 3');

  for (const bad of ['7', '0', 'abc', '1.5']) {
    await openPalette(page);
    await search(page).fill('go to frame');
    await page.keyboard.press('Enter');
    const field = palette(page).getByLabel('Frame number (1–3)');
    await expect(field).toBeFocused();
    await field.fill(bad);
    await page.keyboard.press('Enter');
    await expect(field).toHaveAttribute('aria-invalid', 'true');
    await page.keyboard.press('Escape');
    await expect(palette(page)).toBeHidden();
    await expect(current).toHaveAttribute('aria-label', 'Frame 3');
  }

  await openPalette(page);
  await search(page).fill('go to frame');
  await page.keyboard.press('Enter');
  await palette(page).getByLabel('Frame number (1–3)').fill('1');
  await page.keyboard.press('Enter');
  await expect(palette(page)).toBeHidden();
  await expect(current).toHaveAttribute('aria-label', 'Frame 1');
});

test('recently run commands come first, also after a reload (AC-13)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  for (const [query, label] of [
    ['new layer', 'Layer ▸ New Layer'],
    ['zoom in', 'View ▸ Zoom In'],
  ] as const) {
    await openPalette(page);
    await search(page).fill(query);
    await option(page, label).click();
    await expect(palette(page)).toBeHidden();
  }
  await openPalette(page);
  expect((await labels(page)).slice(0, 2)).toEqual(['View ▸ Zoom In', 'Layer ▸ New Layer']);
  await page.keyboard.press('Escape');

  await expect
    .poll(() => storedPref(page, 'commandRecents'))
    .toEqual(['View ▸ Zoom In', 'Layer ▸ New Layer']);
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openPalette(page);
  expect((await labels(page)).slice(0, 2)).toEqual(['View ▸ Zoom In', 'Layer ▸ New Layer']);
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('the palette speaks German and finds English queries (AC-05, AC-17)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Neues Pixel-Sprite' }).click();
    const card = page.getByRole('form', { name: 'Neues Pixel-Sprite' });
    await card.getByRole('button', { name: 'Erstellen' }).click();
    await expect(pixelCanvas(page)).toHaveAttribute('data-ready', 'true', { timeout: 60_000 });

    await page.keyboard.press('Control+k');
    const dialog = palette(page, 'Befehlspalette');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('combobox')).toHaveAttribute('placeholder', 'Befehl eingeben…');
    await expect(
      dialog.getByRole('option').filter({ has: page.getByText('Gehe zu Frame…', { exact: true }) }),
    ).toHaveCount(1);

    await dialog.getByRole('combobox').fill('merge');
    const first = dialog.getByRole('option').first().locator('span').first();
    await expect(first).toHaveText('Ebene ▸ Nach unten vereinen');
    await dialog.getByRole('combobox').fill('wand');
    await expect(first).toHaveText('Werkzeug: Zauberstab');
  });
});
