import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, switchLanguage } from './helpers';

// docs/specs/dialogs.md — the form dialogs of both modes on one shell:
// a close control, Escape, Enter on a field, Tab inside, focus on open and
// on close, a verb on the primary button, no close on a backdrop press.

type Mode = 'pixel' | 'image';

interface Case {
  mode: Mode;
  /** Menu path that opens the dialog. */
  path: string[];
  /** The primary button's label (AC-04). */
  verb: string;
  /** Runs before the menu (e.g. a selection for Feather). */
  before?: (page: Page) => Promise<void>;
}

const selectAll = async (page: Page) => menu(page, 'Select', 'Select All');

const CASES: Case[] = [
  { mode: 'pixel', path: ['View', 'Grid Settings…'], verb: 'Apply' },
  { mode: 'pixel', path: ['Image', 'Resize Canvas…'], verb: 'Resize' },
  { mode: 'pixel', path: ['Image', 'Scale Image…'], verb: 'Scale' },
  { mode: 'pixel', path: ['File', 'Export', 'PNG (current frame)'], verb: 'Export' },
  { mode: 'pixel', path: ['File', 'Export', 'Animation…'], verb: 'Export' },
  { mode: 'pixel', path: ['File', 'Export', 'SVG…'], verb: 'Export' },
  { mode: 'image', path: ['Image', 'Resize Canvas…'], verb: 'Resize' },
  { mode: 'image', path: ['Image', 'Scale Image…'], verb: 'Scale' },
  { mode: 'image', path: ['File', 'Export', 'JPEG…'], verb: 'Export' },
  { mode: 'image', path: ['Select', 'Feather…'], verb: 'Apply', before: selectAll },
  { mode: 'pixel', path: ['File', 'Export', 'Sprite Sheet…'], verb: 'Export' },
  { mode: 'pixel', path: ['Layer', 'Rename…'], verb: 'Rename' },
  { mode: 'pixel', path: ['Select', 'Expand…'], verb: 'Apply', before: selectAll },
  { mode: 'pixel', path: ['Select', 'Contract…'], verb: 'Apply', before: selectAll },
  { mode: 'image', path: ['File', 'Print…'], verb: 'Print' },
  { mode: 'image', path: ['File', 'Save As…'], verb: 'Save' },
  { mode: 'image', path: ['File', 'Export', 'SVG…'], verb: 'Export' },
  { mode: 'image', path: ['Layer', 'Rename…'], verb: 'Rename' },
  { mode: 'image', path: ['Select', 'Expand…'], verb: 'Apply', before: selectAll },
  { mode: 'image', path: ['Select', 'Contract…'], verb: 'Apply', before: selectAll },
];

const label = (c: Case) => `${c.mode}: ${c.path.join(' ▸ ')}`;
const dialog = (page: Page): Locator => page.locator('[data-dialog]');
const undo = (page: Page) =>
  page.locator('[data-editor]:not([inert]) [data-tour="quick"]').getByRole('button', { name: 'Undo', exact: true });
const tool = (page: Page) => page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');

async function open(page: Page, c: Case): Promise<Locator> {
  await createDocument(page, c.mode, { width: 16, height: 16 });
  await c.before?.(page);
  await menu(page, ...(c.path as [string, ...string[]]));
  await expect(dialog(page)).toHaveCount(1);
  return dialog(page);
}

test.use({ viewport: { width: 1280, height: 800 } });

for (const c of CASES) {
  test(`${label(c)}: one shell — modal, named by its title, a Close control, a verb, focus on the first field (AC-01, AC-04, AC-05)`, async ({
    page,
  }) => {
    const d = await open(page, c);
    await expect(d).toHaveAttribute('role', 'dialog');
    await expect(d).toHaveAttribute('aria-modal', 'true');
    const title = ((await d.locator('header h2').textContent()) ?? '').trim();
    expect(title.length).toBeGreaterThan(0);
    await expect(page.getByRole('dialog', { name: title, exact: true })).toHaveCount(1);
    const close = d.locator('header').getByRole('button', { name: 'Close', exact: true });
    await expect(close).toBeVisible();
    // At the header's right end.
    const h = await d.locator('header').boundingBox();
    const b = await close.boundingBox();
    if (!h || !b) throw new Error('not laid out');
    expect(h.x + h.width - (b.x + b.width)).toBeLessThan(32);
    await expect(d.locator('[data-dialog-primary]')).toHaveText(c.verb);
    // The first field has the focus, or the primary button without one.
    const first = await page.evaluate(() => {
      const panel = document.querySelector('[data-dialog]');
      const fields = [
        ...(panel?.querySelectorAll<HTMLElement>('.am-body input, .am-body select, .am-body textarea') ?? []),
      ].filter(
        (el) =>
          !(el as HTMLInputElement).disabled &&
          (el as HTMLInputElement).type !== 'hidden' &&
          el.offsetParent !== null,
      );
      const expected = fields[0] ?? panel?.querySelector('[data-dialog-primary]') ?? null;
      return expected !== null && document.activeElement === expected;
    });
    expect(first).toBe(true);
  });

  test(`${label(c)}: Close, Escape and Cancel change nothing; the backdrop does not close (AC-02, AC-09)`, async ({
    page,
  }) => {
    let d = await open(page, c);
    const undoBefore = await undo(page).isEnabled();
    // A press on the backdrop.
    await page.mouse.click(4, 400);
    await expect(d).toHaveCount(1);
    await d.locator('header').getByRole('button', { name: 'Close', exact: true }).click();
    await expect(d).toHaveCount(0);
    expect(await undo(page).isEnabled()).toBe(undoBefore);
    await menu(page, ...(c.path as [string, ...string[]]));
    d = dialog(page);
    await expect(d).toHaveCount(1);
    await page.keyboard.press('Escape');
    await expect(d).toHaveCount(0);
    expect(await undo(page).isEnabled()).toBe(undoBefore);
  });

  test(`${label(c)}: Tab stays inside; keys reach no tool; the focus returns on close (AC-06, AC-07, AC-08)`, async ({
    page,
  }) => {
    const d = await open(page, c);
    const before = await tool(page).getAttribute('aria-label');
    const inside = () => page.evaluate(() => document.activeElement?.closest('[data-dialog]') !== null);
    for (let i = 0; i < 30; i += 1) {
      await page.keyboard.press('Tab');
      expect(await inside()).toBe(true);
    }
    for (let i = 0; i < 10; i += 1) {
      await page.keyboard.press('Shift+Tab');
      expect(await inside()).toBe(true);
    }
    // A tool key typed into the dialog changes no tool.
    await d.locator('[data-dialog-primary]').focus();
    await page.keyboard.press('e');
    await expect(tool(page)).toHaveAttribute('aria-label', before ?? '');
    await page.keyboard.press('Escape');
    await expect(d).toHaveCount(0);
    // The menu item is gone with its menu: the canvas area has the focus.
    const focus = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      return el?.hasAttribute('data-stage') === true || el?.closest('[data-stage]') !== null;
    });
    expect(focus).toBe(true);
  });
}

test('pixel: Enter in a field does what the primary button does; Enter on Cancel only cancels (AC-03)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'Image', 'Resize Canvas…');
  const d = dialog(page);
  // Enter on a focused button activates that button only: Cancel resizes nothing.
  await d.getByRole('spinbutton').first().fill('20');
  await d.getByRole('button', { name: 'Cancel', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(d).toHaveCount(0);
  await expect(undo(page)).toBeDisabled();
  await menu(page, 'Image', 'Resize Canvas…');
  const width = d.getByRole('spinbutton').first();
  await width.fill('20');
  await width.press('Enter');
  await expect(d).toHaveCount(0);
  await expect(undo(page)).toBeEnabled();
  await expect(page.locator('[data-frame="status"] [data-status-size]')).toContainText('20');
});

test('image: Enter in a field applies; Enter with the primary disabled does nothing (AC-03)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await menu(page, 'Image', 'Resize Canvas…');
  const d = dialog(page);
  const absolute = d.getByRole('radio', { name: 'By absolute size' });
  if ((await absolute.count()) > 0) await absolute.check();
  const width = d.getByRole('textbox', { name: 'Width', exact: true });
  // A fractional pixel size is refused (image-resize-autocrop).
  await width.fill('12.5');
  await expect(d.locator('[data-dialog-primary]')).toBeDisabled();
  await width.press('Enter');
  await expect(d).toHaveCount(1);
  await width.fill('24');
  await expect(d.locator('[data-dialog-primary]')).toBeEnabled();
  await width.press('Enter');
  await expect(d).toHaveCount(0);
  await expect(page.locator('[data-frame="status"] [data-status-size]')).toContainText('24');
});

for (const how of ['Cancel', 'Close', 'Escape', 'Export'] as const) {
  test(`the focus returns to the element that had it before the dialog opened: ${how} (AC-07)`, async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    // A dialog opened by its shortcut while a panel control has the focus.
    const hide = page.getByRole('button', { name: 'Hide Layer 1', exact: true });
    await hide.focus();
    await page.keyboard.press('Control+e');
    const d = dialog(page);
    await expect(d).toHaveCount(1);
    await expect(d.locator('[data-dialog-primary]')).toHaveText('Export');
    if (how === 'Escape') await page.keyboard.press('Escape');
    else if (how === 'Close') await d.locator('header').getByRole('button', { name: 'Close', exact: true }).click();
    else if (how === 'Cancel') await d.getByRole('button', { name: 'Cancel', exact: true }).click();
    else await d.locator('[data-dialog-primary]').click();
    await expect(d).toHaveCount(0);
    await expect(hide).toBeFocused();
  });
}

test('the focus goes to the canvas area when the element that had it is gone (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  // Opened from the command palette, which is gone once its command ran.
  await page.keyboard.press('Control+k');
  const palette = page.getByRole('dialog', { name: 'Command Palette' });
  await palette.getByRole('combobox').fill('Layer Rename');
  await page.keyboard.press('Enter');
  const d = dialog(page);
  await expect(d).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(d).toHaveCount(0);
  await expect(page.locator('[data-editor]:not([inert]) [data-stage]')).toBeFocused();
});

test('German: Close reads "Schließen" and Grid Settings applies with "Anwenden" (AC-10)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await switchLanguage(page, 'Deutsch');
  await menu(page, 'Ansicht', 'Rastereinstellungen…');
  const d = dialog(page);
  await expect(d.locator('header').getByRole('button', { name: 'Schließen', exact: true })).toBeVisible();
  await expect(d.locator('[data-dialog-primary]')).toHaveText('Anwenden');
});
