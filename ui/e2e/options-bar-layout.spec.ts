import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, switchLanguage } from './helpers';

// docs/specs/options-bar-layout.md — the options bar names the tool, never
// scrolls sideways, and its More control expands it to show every option.

type Mode = 'pixel' | 'image';
/** A window in which Image mode's Text options need more than one row. */
const NARROWER = { width: 1100, height: 720 };
const MODES: Mode[] = ['pixel', 'image'];

const editor = (page: Page) => page.locator('[data-editor]:not([inert])');
const bar = (page: Page) => editor(page).locator('[data-frame="options"]');
const name = (page: Page) => bar(page).locator('[data-tool-name]');
const more = (page: Page) => editor(page).locator('[data-more-options]');
const rail = (page: Page) => editor(page).locator('[data-tool-list]');
const stage = (page: Page) => editor(page).locator('[data-stage]');

interface Row {
  /** Options wholly inside the bar's box. */
  inside: number;
  /** Options that overlap the bar's box only in part. */
  cut: number;
  total: number;
  sideways: boolean;
  height: number;
}

/** Where the bar's direct children lie relative to its box. */
const row = (page: Page): Promise<Row> =>
  bar(page).evaluate((el) => {
    const b = el.getBoundingClientRect();
    let inside = 0;
    let cut = 0;
    for (const child of el.children) {
      const c = child.getBoundingClientRect();
      if (c.width === 0 && c.height === 0) continue;
      const within = c.left >= b.left - 0.5 && c.right <= b.right + 0.5 && c.top >= b.top - 0.5 && c.bottom <= b.bottom + 0.5;
      const apart = c.bottom <= b.top + 0.5 || c.top >= b.bottom - 0.5 || c.right <= b.left + 0.5 || c.left >= b.right - 0.5;
      if (within) inside += 1;
      else if (!apart) cut += 1;
    }
    return {
      inside,
      cut,
      total: el.children.length,
      sideways: el.scrollWidth > el.clientWidth,
      height: Math.round(b.height),
    };
  });

/** Selects Image mode's Text tool: the longest options bar. */
async function textTool(page: Page): Promise<void> {
  await stage(page).click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('t');
  await expect(name(page)).toHaveText('Text');
}

async function box(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

test.describe('wide', () => {
  test.use({ viewport: { width: 1280, height: 720 } });

  for (const mode of MODES) {
    test(`${mode}: the bar starts with the tool name, however the tool is chosen (AC-01)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      const brush = mode === 'pixel' ? 'Pencil' : 'Brush';
      await expect(name(page)).toHaveText(brush);
      // First in the bar, in the text colour, not the options' muted one.
      expect(await bar(page).evaluate((el) => el.firstElementChild?.hasAttribute('data-tool-name'))).toBe(true);
      const colours = await bar(page).evaluate((el) => {
        const first = el.firstElementChild as HTMLElement;
        return [getComputedStyle(first).color, getComputedStyle(el).color];
      });
      expect(colours[0]).not.toBe(colours[1]);
      // Rail, key, variant switcher, command palette.
      await rail(page).locator('button[data-group="eraser"]').click();
      await expect(name(page)).toHaveText('Eraser');
      await stage(page).click({ position: { x: 5, y: 5 } });
      await page.keyboard.press('u');
      await expect(name(page)).toHaveText('Rectangle');
      await bar(page).getByRole('radio', { name: 'Ellipse', exact: true }).click();
      await expect(name(page)).toHaveText('Ellipse');
      await page.keyboard.press('Control+k');
      const palette = page.getByRole('dialog', { name: 'Command Palette' });
      await palette.getByRole('combobox').fill('Tool: Eyedropper');
      await page.keyboard.press('Enter');
      await expect(name(page)).toHaveText('Eyedropper');
      // The rail's flyout.
      await rail(page).locator('button[data-group="shapes"]').click({ button: 'right' });
      await page.getByRole('menu', { name: 'Shapes' }).getByRole('menuitemradio', { name: 'Polygon', exact: true }).click();
      await expect(name(page)).toHaveText('Polygon');
    });

    test(`${mode}: options that fit show no More control and nothing is cut (AC-02, AC-03)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await rail(page).locator('button[data-group="eyedropper"]').click();
      await expect(name(page)).toHaveText('Eyedropper');
      await expect(more(page)).toHaveCount(0);
      const r = await row(page);
      expect(r.sideways).toBe(false);
      expect(r.cut).toBe(0);
      expect(r.inside).toBe(r.total);
      expect(r.height).toBe(40);
    });
  }

  test('image: a tool without options shows its name alone (AC-01, AC-03)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await rail(page).locator('button[data-group="zoom"]').click();
    await expect(name(page)).toHaveText('Zoom');
    await expect(more(page)).toHaveCount(0);
    expect((await row(page)).total).toBe(1);
  });

  test('image: Text has more options than fit; More expands the bar over the canvas and back (AC-02, AC-04, AC-05, AC-06)', async ({
    page,
  }) => {
    // The Text options fit one row at 1280 px (AC-13): a narrower window.
    await page.setViewportSize(NARROWER);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await textTool(page);
    await expect(more(page)).toBeVisible();
    await expect(more(page)).toHaveAttribute('aria-label', 'More options');
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
    const collapsed = await row(page);
    expect(collapsed.sideways).toBe(false);
    expect(collapsed.cut).toBe(0);
    expect(collapsed.inside).toBeGreaterThan(1);
    expect(collapsed.inside).toBeLessThan(collapsed.total);
    expect(collapsed.height).toBe(40);
    // The More control ends the row, beside the last option in it.
    const b = await box(bar(page));
    const m = await box(more(page));
    expect(m.x + m.width).toBeLessThanOrEqual(b.x + b.width);
    expect(m.x).toBeGreaterThan(b.x + b.width - 48);
    const canvasBefore = await box(stage(page));
    const order = await bar(page).evaluate((el) => [...el.children].map((c) => c.textContent ?? ''));

    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    const open = await row(page);
    expect(open.inside).toBe(open.total);
    expect(open.cut).toBe(0);
    expect(open.sideways).toBe(false);
    expect(open.height).toBeGreaterThan(60);
    // Same options in the same order, over the canvas, which does not move.
    expect(await bar(page).evaluate((el) => [...el.children].map((c) => c.textContent ?? ''))).toEqual(order);
    expect(await box(stage(page))).toEqual(canvasBefore);
    expect((await box(more(page))).y).toBe(m.y);

    // Again: one row.
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
    expect((await row(page)).height).toBe(40);
    // Escape with the focus in the bar.
    await more(page).click();
    await bar(page).getByRole('checkbox').first().focus();
    await page.keyboard.press('Escape');
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
    // Escape on the More control itself.
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    await page.keyboard.press('Escape');
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');

    // Nothing else closes it: a press on the canvas area, on the rail, …
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    const s = await box(stage(page));
    await page.mouse.click(s.x + s.width - 30, s.y + s.height - 30);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    await page.keyboard.press('Escape');
    await rail(page).locator('button[data-group="text"]').click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    // … or another tool. One whose options fit shows one row and no More control;
    // back on Text the bar is expanded as the user left it.
    await rail(page).locator('button[data-group="eyedropper"]').click();
    await expect(name(page)).toHaveText('Eyedropper');
    await expect(more(page)).toHaveCount(0);
    expect((await row(page)).height).toBe(40);
    await rail(page).locator('button[data-group="text"]').click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    expect((await row(page)).height).toBeGreaterThan(60);
    // A new page load starts with one row.
    await createDocument(page, 'image', { width: 16, height: 16 });
    await textTool(page);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
  });

  test('image: Tab onto an option outside the row expands the bar; a value changed there applies and the bar stays (AC-07, AC-08)', async ({
    page,
  }) => {
    // The Text options fit one row at 1280 px (AC-13): a narrower window.
    await page.setViewportSize(NARROWER);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await textTool(page);
    const collapsed = await row(page);
    // The last enabled control inside the row (Finish is disabled without a
    // text box), then Tab.
    const enabled = 'input:not([disabled]), select:not([disabled]), button:not([disabled])';
    const inRow = bar(page).locator(':scope > *');
    let control: Locator | null = null;
    for (let i = collapsed.inside - 1; i >= 0 && control === null; i -= 1) {
      const found = inRow.nth(i).locator(enabled);
      if ((await found.count()) > 0) control = found.last();
    }
    if (control === null) throw new Error('no enabled control in the row');
    await control.focus();
    await page.keyboard.press('Tab');
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    const focused = page.locator(':focus');
    await expect(focused).toBeInViewport({ ratio: 1 });
    expect(await focused.evaluate((el) => el.closest('[data-frame="options"]') !== null)).toBe(true);

    // A checkbox below the row: toggling it applies and keeps the bar open.
    const boxes = bar(page).getByRole('checkbox');
    const last = boxes.last();
    const before = await last.isChecked();
    await last.click();
    expect(await last.isChecked()).toBe(!before);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    // Back in one row, the value is kept.
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
    expect(await bar(page).getByRole('checkbox').last().isChecked()).toBe(!before);
  });

  test('image: while an effect is open the bar takes no input and is one row (AC-09)', async ({ page }) => {
    // The Text options fit one row at 1280 px (AC-13): a narrower window.
    await page.setViewportSize(NARROWER);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await textTool(page);
    await page.keyboard.press('Escape');
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    // The Adjustments menu stays usable while the bar is expanded.
    await menu(page, 'Adjustments', 'Invert Colors');
    await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
    expect(await more(page).evaluate((el) => el.closest('[inert]') !== null)).toBe(true);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
    expect((await row(page)).height).toBe(40);
    // Afterwards the bar is as the user left it.
    await page.getByRole('dialog', { name: 'Invert Colors' }).getByRole('button', { name: 'Cancel' }).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
  });

  test('image: the row follows the bar’s width (AC-11)', async ({ page }) => {
    await page.setViewportSize(NARROWER);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await textTool(page);
    const wide = (await row(page)).inside;
    await page.setViewportSize({ width: 1040, height: 720 });
    await expect.poll(async () => (await row(page)).inside).toBeLessThan(wide);
    const r = await row(page);
    expect(r.cut).toBe(0);
    expect(r.sideways).toBe(false);
    // Hiding the sidebar gives the bar its width.
    await page.setViewportSize(NARROWER);
    await expect.poll(async () => (await row(page)).inside).toBe(wide);
    await editor(page).locator('[data-panels-toggle] button').click();
    await expect.poll(async () => (await row(page)).inside).toBeGreaterThan(wide);
    // A tool whose options fit: the More control goes.
    await rail(page).locator('button[data-group="eyedropper"]').click();
    await expect(more(page)).toHaveCount(0);
  });

  test('German: the More control reads "Weitere Optionen" (AC-12)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await switchLanguage(page, 'Deutsch');
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('t');
    await expect(more(page)).toHaveAttribute('aria-label', 'Weitere Optionen');
    await expect(more(page)).toHaveAttribute('title', 'Weitere Optionen');
  });
});

for (const language of ['English', 'Deutsch'] as const) {
  test.describe(`text options at 1280 × 720, ${language}`, () => {
    test.use({ viewport: { width: 1280, height: 720 } });

    test('every Text option is in the row; the styles and Align are icon toggles (AC-13)', async ({ page }) => {
      await createDocument(page, 'image', { width: 16, height: 16 });
      if (language === 'Deutsch') await switchLanguage(page, 'Deutsch');
      await expect(editor(page).getByRole('complementary')).toBeVisible();
      await stage(page).click({ position: { x: 5, y: 5 } });
      await page.keyboard.press('t');
      await expect(name(page)).toHaveText('Text');
      const r = await row(page);
      expect(r.cut).toBe(0);
      if (language === 'English') {
        // Everything fits, the shared options too.
        await expect(more(page)).toHaveCount(0);
        expect(r.inside).toBe(r.total);
      } else {
        // The Text tool's own options are in the row; at most the shared
        // Antialiasing and Blending options wait behind More.
        expect(r.inside).toBeGreaterThanOrEqual(r.total - 2);
      }
      const inRow = async (target: Locator): Promise<void> => {
        const b = await box(bar(page));
        const t = await box(target);
        expect(t.y + t.height).toBeLessThanOrEqual(b.y + b.height + 0.5);
        expect(t.x + t.width).toBeLessThanOrEqual(b.x + b.width + 0.5);
      };
      await inRow(bar(page).getByRole('button', { name: /^(Font|Schrift): / }));
      await inRow(bar(page).getByRole('slider', { name: language === 'English' ? 'Font size' : 'Schriftgröße' }));
      await inRow(bar(page).getByRole('button', { name: language === 'English' ? 'Finish' : 'Fertig', exact: true }));
      // Toggle buttons that stay checkboxes, and a radio group for Align.
      const styles =
        language === 'English'
          ? ['Bold', 'Italic', 'Underline', 'Strikethrough']
          : ['Fett', 'Kursiv', 'Unterstrichen', 'Durchgestrichen'];
      for (const style of styles) {
        const box = bar(page).getByRole('checkbox', { name: style, exact: true });
        await expect(box).toHaveCount(1);
        await expect(box.locator('svg')).toHaveCount(1);
      }
      const align = bar(page).getByRole('radiogroup', {
        name: language === 'English' ? 'Align' : 'Ausrichtung',
        exact: true,
      });
      await expect(align.getByRole('radio')).toHaveCount(3);
      const left = align.getByRole('radio', { name: language === 'English' ? 'Left' : 'Links', exact: true });
      await expect(left).toBeChecked();
      const centre = align.getByRole('radio', {
        name: language === 'English' ? 'Center' : 'Zentriert',
        exact: true,
      });
      await centre.click();
      await expect(centre).toBeChecked();
      await expect(left).not.toBeChecked();
      const bold = bar(page).getByRole('checkbox', { name: styles[0] ?? 'Bold', exact: true });
      await bold.check();
      await expect(bold).toBeChecked();
    });
  });
}

test.describe('pixel, a bar that needs More', () => {
  test.use({ viewport: { width: 760, height: 720 } });

  test('pixel: a press on the canvas or the rail and a tool change keep the bar expanded (AC-06)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await expect(name(page)).toHaveText('Pencil');
    await expect(more(page)).toBeVisible();
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    // The canvas area.
    const s = await box(stage(page));
    await page.mouse.click(s.x + s.width / 2, s.y + s.height - 40);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    // The rail, choosing another tool with more options than fit.
    await rail(page).locator('button[data-group="eraser"]').click();
    await expect(name(page)).toHaveText('Eraser');
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    // A tool change by key (the expanded bar lies over the canvas area's top).
    await page.mouse.click(s.x + 8, s.y + s.height - 8);
    await page.keyboard.press('b');
    await expect(name(page)).toHaveText('Pencil');
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    expect((await row(page)).inside).toBe((await row(page)).total);
  });
});

test.describe('narrow', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  for (const mode of MODES) {
    test(`${mode}: on a phone the bar names the tool, does not scroll sideways and expands (AC-10)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      const brush = mode === 'pixel' ? 'Pencil' : 'Brush';
      await expect(name(page)).toHaveText(brush);
      // The name is wholly in view.
      const n = await box(name(page));
      const b = await box(bar(page));
      expect(n.x).toBeGreaterThanOrEqual(b.x);
      expect(n.x + n.width).toBeLessThanOrEqual(b.x + b.width);
      expect(await name(page).evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
      const collapsed = await row(page);
      expect(collapsed.sideways).toBe(false);
      expect(collapsed.cut).toBe(0);
      expect(collapsed.height).toBe(44);
      await expect(more(page)).toBeVisible();
      await more(page).click();
      await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
      const open = await row(page);
      expect(open.inside).toBe(open.total);
      expect(open.sideways).toBe(false);
      // A press on the canvas area leaves it open; the control closes it.
      const s = await box(stage(page));
      await page.mouse.click(s.x + s.width / 2, s.y + s.height - 80);
      await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
      await more(page).click();
      await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
      expect((await row(page)).height).toBe(44);
    });
  }
});
