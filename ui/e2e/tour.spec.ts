import { expect, test, type Page } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';
import { TOUR_STEPS } from '../src/lib/tour/steps';
import { createDocument, gotoStart, menu, storedPref, waitForEditor } from './helpers';

// The first-run tour (docs/specs/first-run-tutorial.md).

const banner = (page: Page) => page.getByRole('region', { name: 'Tour', exact: true });
const dialog = (page: Page) => page.getByRole('dialog', { name: 'Tour' });
const counter = (page: Page) => page.locator('[data-tour-counter]');

async function insideViewport(page: Page): Promise<boolean> {
  const box = await dialog(page).boundingBox();
  const vp = page.viewportSize();
  if (!box || !vp) return false;
  return box.x >= 0 && box.y >= 0 && box.x + box.width <= vp.width && box.y + box.height <= vp.height;
}

test('the start screen offers the tour once; No thanks is remembered (AC-01 – AC-03)', async ({ page }) => {
  await gotoStart(page);
  await expect(banner(page)).toBeVisible();
  await expect(banner(page).getByRole('button', { name: 'Tour', exact: true })).toBeVisible();
  await expect(dialog(page)).toHaveCount(0);
  await banner(page).getByRole('button', { name: 'No thanks' }).click();
  await expect(banner(page)).toHaveCount(0);
  await expect.poll(() => storedPref(page, 'tourDeclined')).toBe(true);
  await gotoStart(page);
  await expect(page.getByRole('button', { name: 'Open a sample' })).toBeVisible();
  await expect(banner(page)).toHaveCount(0);
  await expect(dialog(page)).toHaveCount(0);
});

test('the tour highlights each anchor with a callout, counter and Back / Next / Skip inside the viewport (AC-03, AC-04, AC-08)', async ({
  page,
}) => {
  await gotoStart(page);
  await banner(page).getByRole('button', { name: 'Tour', exact: true }).click();
  await expect(dialog(page)).toBeVisible();
  await expect(dialog(page)).toHaveAttribute('aria-modal', 'true');
  await expect(dialog(page).locator('[aria-live="polite"]')).toHaveText(en['tour.modes']);
  const steps = TOUR_STEPS.start.length;
  for (let i = 1; i <= steps; i += 1) {
    await expect(counter(page)).toHaveText(`${i} / ${steps}`);
    await expect(page.locator('[data-tour-highlight]')).toHaveCount(1);
    expect(await insideViewport(page)).toBe(true);
    for (const b of ['Skip', 'Back']) {
      await expect(dialog(page).getByRole('button', { name: b })).toBeVisible();
    }
    await dialog(page).getByRole('button', { name: i === steps ? 'Done' : 'Next' }).click();
  }
  await expect(dialog(page)).toHaveCount(0);
  await expect.poll(() => storedPref(page, 'tourSeen')).toBe(true);
  await gotoStart(page);
  await expect(page.getByRole('button', { name: 'Open a sample' })).toBeVisible();
  await expect(banner(page)).toHaveCount(0);
});

test('keyboard: arrows and Enter step, Esc skips, focus stays in the callout (AC-07)', async ({ page }) => {
  await gotoStart(page);
  await page.getByRole('button', { name: 'Tour', exact: true }).first().click();
  await expect(counter(page)).toHaveText('1 / 4');
  await page.keyboard.press('ArrowRight');
  await expect(counter(page)).toHaveText('2 / 4');
  await page.keyboard.press('ArrowLeft');
  await expect(counter(page)).toHaveText('1 / 4');
  // Enter on the focused Next button goes on too.
  await page.keyboard.press('Enter');
  await expect(counter(page)).toHaveText('2 / 4');
  for (let i = 0; i < 5; i += 1) {
    await page.keyboard.press('Tab');
    expect(await dialog(page).evaluate((el) => el.contains(document.activeElement))).toBe(true);
  }
  await page.keyboard.press('Escape');
  await expect(dialog(page)).toHaveCount(0);
});

test('Help ▸ Tour starts the editor tour at step 1; a missing anchor centres the callout (AC-06, AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'Help', 'Tour');
  await expect(counter(page)).toHaveText(`1 / ${TOUR_STEPS.pixel.length}`);
  await page.keyboard.press('Escape');

  // Phone layout: no sidebar, so the Panels step centres its callout.
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: 'Menu', exact: true }).click();
  const help = page.getByRole('menuitem', { name: 'Help', exact: true });
  if (await help.count()) await help.click();
  await page.getByRole('menuitem', { name: 'Tour', exact: true }).click();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await expect(counter(page)).toHaveText(`3 / ${TOUR_STEPS.pixel.length}`);
  await expect(page.locator('[data-tour-highlight]')).toHaveCount(0);
  expect(await insideViewport(page)).toBe(true);
  const box = await dialog(page).boundingBox();
  if (!box) throw new Error('no callout');
  expect(Math.abs(box.x + box.width / 2 - 195)).toBeLessThanOrEqual(2);
  await page.keyboard.press('ArrowRight');
  await expect(counter(page)).toHaveText(`4 / ${TOUR_STEPS.pixel.length}`);
});

test('with reduced motion the highlight and callout do not animate (AC-09)', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await gotoStart(page);
  await page.getByRole('button', { name: 'Tour', exact: true }).first().click();
  const durations = await page.evaluate(() =>
    ['[data-tour-callout]', '[data-tour-highlight]'].map((s) => {
      const el = document.querySelector(s);
      return el ? getComputedStyle(el).transitionDuration : 'missing';
    }),
  );
  // The app's reduced-motion rule leaves 0.01 ms: no visible animation.
  for (const d of durations) expect(parseFloat(d)).toBeLessThan(0.001);
});

test('each editor offers its part once (AC-10)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const hint = page.getByRole('status').filter({ hasText: 'First time in Pixel mode?' });
  await expect(hint).toBeVisible();
  await expect.poll(() => storedPref(page, 'tourHintPixel')).toBe(true);
  await hint.getByRole('button', { name: 'Take the tour' }).click();
  await expect(counter(page)).toHaveText(`1 / ${TOUR_STEPS.pixel.length}`);
  await page.keyboard.press('Escape');
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await waitForEditor(page, 'pixel');
  await expect(page.getByText('First time in Pixel mode?')).toHaveCount(0);

  // The other editor still offers its own part.
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect(page.getByText('First time in Image mode?')).toBeVisible();
});

test('after No thanks no editor hint shows (AC-10)', async ({ page }) => {
  await gotoStart(page);
  await banner(page).getByRole('button', { name: 'No thanks' }).click();
  await expect.poll(() => storedPref(page, 'tourDeclined')).toBe(true);
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect(page.getByText('First time in Image mode?')).toHaveCount(0);
});

test('the start screen has a Tour button beside About (AC-18)', async ({ page }) => {
  await gotoStart(page);
  await banner(page).getByRole('button', { name: 'No thanks' }).click();
  const tour = page.getByRole('button', { name: 'Tour', exact: true });
  await expect(tour).toHaveCount(1);
  await tour.click();
  await expect(counter(page)).toHaveText(`1 / ${TOUR_STEPS.start.length}`);
});

test('step lists stay short and every string exists in both languages (AC-05, AC-17)', () => {
  const topics = new Set<string>();
  for (const [ctx, steps] of Object.entries(TOUR_STEPS)) {
    expect(steps.length, ctx).toBeLessThanOrEqual(7);
    for (const s of steps) {
      expect(en[s.text].length, s.text).toBeLessThanOrEqual(140);
      expect(de[s.text], s.text).toBeTruthy();
      topics.add(s.text);
    }
  }
  // Modes, create / open, tools, options, panels, undo + shortcuts, help.
  for (const k of ['tour.modes', 'tour.create', 'tour.tools', 'tour.options', 'tour.panels', 'tour.undo', 'tour.help']) {
    expect(topics.has(k), k).toBe(true);
  }
  for (const k of Object.keys(en).filter((k) => k.startsWith('tour.') || k.startsWith('start.sample'))) {
    expect((de as Record<string, string>)[k], k).toBeTruthy();
  }
});
