import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, pixelCanvas, switchLanguage } from './helpers';

// docs/specs/notifications.md — an editor's notice shows as a toast over the
// canvas area, on every window width; the status bar keeps the text.

type Mode = 'pixel' | 'image';
const MODES: Mode[] = ['pixel', 'image'];

const editor = (page: Page) => page.locator('[data-editor]:not([inert])');
const toast = (page: Page) => editor(page).locator('[data-toast]');
const stage = (page: Page) => editor(page).locator('[data-stage]');
const statusText = (page: Page) => editor(page).locator('[data-frame="status"] [data-status-text]');
const canvasOf = (page: Page, mode: Mode) => (mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page));
const locked = (n: number) => `Layer “Layer ${n}” is locked`;

async function box(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

/** Locks layer `n` and presses on the canvas: the editor reports that the layer is locked. */
async function refuse(page: Page, mode: Mode, n = 1): Promise<void> {
  await page.getByRole('button', { name: `Lock Layer ${n}`, exact: true }).click();
  await drag(page, canvasOf(page, mode), [0.3, 0.3], [0.6, 0.6]);
}

test.describe('wide', () => {
  test.use({ viewport: { width: 1280, height: 720 } });

  for (const mode of MODES) {
    test(`${mode}: a notice shows as a toast on the status bar; idle texts and tool names do not (AC-01, AC-02, AC-06, AC-07)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      // A fresh document and a tool change report nothing.
      await stage(page).click({ position: { x: 5, y: 5 } });
      await page.keyboard.press('e');
      // Pixel mode's idle text stays; Image mode shows the tool's name.
      await expect(statusText(page)).toHaveText(mode === 'pixel' ? 'ready' : 'Eraser');
      await page.keyboard.press('b');
      await page.waitForTimeout(400);
      await expect(toast(page)).toHaveCount(0);

      await refuse(page, mode);
      await expect(toast(page)).toBeVisible();
      await expect(toast(page)).toHaveCount(1);
      await expect(toast(page)).toContainText(locked(1));
      await expect(toast(page)).toHaveAttribute('role', 'status');
      await expect(toast(page).getByRole('button', { name: 'Dismiss', exact: true })).toBeVisible();
      // Centred on the status bar, not higher than it: nothing of the canvas
      // area is covered.
      const s = await box(stage(page));
      const bar = await box(editor(page).locator('[data-frame="status"]'));
      const t = await box(toast(page));
      expect(Math.abs(t.x + t.width / 2 - (bar.x + bar.width / 2))).toBeLessThanOrEqual(1);
      expect(t.y).toBeGreaterThanOrEqual(bar.y - 0.5);
      expect(t.y + t.height).toBeLessThanOrEqual(bar.y + bar.height + 0.5);
      expect(t.y).toBeGreaterThanOrEqual(s.y + s.height);
      // Beside the toast, at its height, a press reaches what lies there.
      const beside = await page.evaluate(
        ([x, y]) => document.elementFromPoint(x, y)?.closest('[data-toast]') === null,
        [t.x - 20, t.y + t.height / 2] as const,
      );
      expect(beside).toBe(true);
      // The status bar shows the notice too.
      await expect(statusText(page)).toHaveText(locked(1));
    });

    test(`${mode}: the toast goes after 5 seconds, not while the pointer is over it; the status bar keeps the text (AC-03, AC-07)`, async ({
      page,
    }) => {
      test.setTimeout(60_000);
      await createDocument(page, mode, { width: 16, height: 16 });
      await refuse(page, mode);
      await expect(toast(page)).toBeVisible();
      const t = await box(toast(page));
      await page.mouse.move(t.x + 12, t.y + t.height / 2);
      await page.waitForTimeout(6000);
      await expect(toast(page)).toBeVisible();
      // Five seconds after the pointer has left.
      await page.mouse.move(t.x + 12, t.y - 200);
      await page.waitForTimeout(3500);
      await expect(toast(page)).toBeVisible();
      await expect(toast(page)).toHaveCount(0, { timeout: 3000 });
      await expect(statusText(page)).toHaveText(locked(1));
      // The same notice again changes nothing: no second toast.
      await drag(page, canvasOf(page, mode), [0.3, 0.3], [0.6, 0.6]);
      await page.waitForTimeout(400);
      await expect(toast(page)).toHaveCount(0);
    });

    test(`${mode}: without the pointer the toast is gone after 5 seconds; focus inside holds it (AC-03)`, async ({
      page,
    }) => {
      test.setTimeout(60_000);
      await createDocument(page, mode, { width: 16, height: 16 });
      await refuse(page, mode);
      await expect(toast(page)).toBeVisible();
      await page.mouse.move(2, 2);
      await page.waitForTimeout(3500);
      await expect(toast(page)).toBeVisible();
      await expect(toast(page)).toHaveCount(0, { timeout: 3000 });

      await menu(page, 'Layer', 'New Layer');
      await refuse(page, mode, 2);
      await expect(toast(page)).toContainText(locked(2));
      await page.mouse.move(2, 2);
      await toast(page).getByRole('button', { name: 'Dismiss', exact: true }).focus();
      await page.waitForTimeout(6000);
      await expect(toast(page)).toBeVisible();
    });

    test(`${mode}: Dismiss removes the toast at once; a newer notice replaces the shown one (AC-04, AC-05)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await refuse(page, mode);
      await expect(toast(page)).toContainText(locked(1));
      await menu(page, 'Layer', 'New Layer');
      await refuse(page, mode, 2);
      await expect(toast(page)).toHaveCount(1);
      await expect(toast(page)).toContainText(locked(2));
      await expect(toast(page)).not.toContainText(locked(1));
      await toast(page).getByRole('button', { name: 'Dismiss', exact: true }).click();
      await expect(toast(page)).toHaveCount(0, { timeout: 500 });
      await expect(statusText(page)).toHaveText(locked(2));
    });

    test(`${mode}: while an effect is open the toast shows and can be dismissed (AC-09)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await refuse(page, mode);
      await expect(toast(page)).toBeVisible();
      await menu(page, 'Adjustments', 'Invert Colors');
      await expect(page.getByRole('dialog', { name: 'Invert Colors' })).toBeVisible();
      await expect(toast(page)).toBeVisible();
      expect(await toast(page).evaluate((el) => el.closest('[inert]') === null)).toBe(true);
      await toast(page).getByRole('button', { name: 'Dismiss', exact: true }).click();
      await expect(toast(page)).toHaveCount(0);
    });
  }

  test('German: the toast shows the German notice and its control reads "Schließen" (AC-10)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    await switchLanguage(page, 'Deutsch');
    await page.getByRole('button', { name: /Ebene 1 sperren|Layer 1 sperren/ }).click();
    await drag(page, imageCanvas(page), [0.3, 0.3], [0.6, 0.6]);
    await expect(toast(page)).toBeVisible();
    await expect(toast(page)).toContainText('gesperrt');
    await expect(toast(page).getByRole('button', { name: 'Schließen', exact: true })).toBeVisible();
  });
});

test.describe('narrow', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  for (const mode of MODES) {
    test(`${mode}: on a phone the toast sits above the zoom control and the panels sheet, inside the canvas area (AC-01, AC-08)`, async ({
      page,
    }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await expect(page.locator('[data-frame="status"]')).toHaveCount(0);
      await editor(page).locator('[data-panels-toggle] button').click();
      const sheet = page.getByRole('dialog', { name: 'Panels' });
      await sheet.getByRole('tab', { name: 'Layers' }).click();
      await sheet.getByRole('button', { name: 'Lock Layer 1', exact: true }).click();
      const c = await box(canvasOf(page, mode));
      await page.mouse.move(c.x + 6, c.y + 6);
      await page.mouse.down();
      await page.mouse.move(c.x + 12, c.y + 12, { steps: 3 });
      await page.mouse.up();
      await expect(toast(page)).toBeVisible();
      await expect(toast(page)).toContainText(locked(1));
      const t = await box(toast(page));
      const s = await box(stage(page));
      expect(t.x).toBeGreaterThanOrEqual(s.x);
      expect(t.x + t.width).toBeLessThanOrEqual(s.x + s.width);
      expect(t.y).toBeGreaterThanOrEqual(s.y);
      expect(t.y + t.height).toBeLessThanOrEqual((await box(sheet)).y);
      const zoom = await box(editor(page).locator('[data-zoom-control]'));
      expect(t.y + t.height).toBeLessThanOrEqual(zoom.y);
      // With the sheet closed it is still above the zoom control.
      await sheet.getByRole('button', { name: 'Close panels' }).click();
      const t2 = await box(toast(page));
      expect(t2.y + t2.height).toBeLessThanOrEqual((await box(editor(page).locator('[data-zoom-control]'))).y);
      expect(t2.y + t2.height).toBeLessThanOrEqual(s.y + s.height);
    });
  }
});
