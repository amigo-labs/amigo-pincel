import { expect, test, type Locator, type Page } from '@playwright/test';
import { pixelCanvas, setColor, zoomReadout } from './helpers';
import { done, event, isInert, panel, sendMessage, sent, startDesktop } from './assistant-helpers';

// The UI refresh while an Assistant answer runs (mocked Tauri bridge):
// tool-rail-colors AC-06, AC-08 (desktop app), AC-12; options-bar-layout
// AC-09; notifications AC-09.

const editor = (page: Page) => page.locator('[data-editor]:not([inert])');
const rail = (page: Page) => editor(page).getByRole('toolbar', { name: 'Tools', exact: true });
const list = (page: Page) => rail(page).locator('[data-tool-list]');
const well = (page: Page) => rail(page).locator('[data-color-well]');
const stage = (page: Page) => editor(page).locator('[data-stage]');
const more = (page: Page) => editor(page).locator('[data-more-options]');
const bar = (page: Page) => editor(page).locator('[data-frame="options"]');
const toast = (page: Page) => editor(page).locator('[data-toast]');
const stop = (page: Page) => panel(page).getByRole('button', { name: 'Stop' });

/** Sends a message and leaves its answer running; returns the chat id. */
async function answering(page: Page): Promise<string> {
  await sendMessage(page, 'draw something');
  const chat = String((await sent(page)).chat);
  await event(page, chat, { kind: 'text', text: 'Working' });
  await expect(stop(page)).toBeVisible();
  return chat;
}

async function finish(page: Page, chat: string): Promise<void> {
  await done(page, chat);
  await expect(stop(page)).toHaveCount(0);
}

async function box(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const b = await locator.boundingBox();
  if (b === null) throw new Error('element has no box');
  return b;
}

test.describe('1280 × 720', () => {
  test.use({ viewport: { width: 1280, height: 720 } });

  for (const mode of ['pixel', 'image'] as const) {
    test(`${mode}: in the desktop app the rail shows every tool and the well without scrolling (tool-rail-colors AC-08)`, async ({
      page,
    }) => {
      await startDesktop(page, mode);
      await expect(well(page)).toBeVisible();
      expect(await list(page).evaluate((el) => el.scrollHeight <= el.clientHeight)).toBe(true);
      const r = await box(rail(page));
      const w = await box(well(page));
      expect(w.y + w.height).toBeLessThanOrEqual(r.y + r.height + 0.5);
    });

    test(`${mode}: while an answer runs the well takes no input; afterwards it does (tool-rail-colors AC-06)`, async ({
      page,
    }) => {
      await startDesktop(page, mode);
      const chat = await answering(page);
      expect(await isInert(well(page))).toBe(true);
      await finish(page, chat);
      expect(await isInert(well(page))).toBe(false);
      await well(page).getByRole('button', { name: 'Swap foreground and background colors' }).click();
      await expect
        .poll(() =>
          well(page)
            .getByRole('button', { name: 'Foreground color', exact: true })
            .evaluate((el) => getComputedStyle(el).getPropertyValue('--swatch').trim()),
        )
        .toBe('rgb(255 255 255 / 1)');
    });
  }

  test('pixel: while an answer runs Pan and Zoom still pan and zoom (tool-rail-colors AC-12)', async ({ page }) => {
    await startDesktop(page, 'pixel');
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('z');
    const zoom = async () => Number.parseInt((await zoomReadout(page).textContent()) ?? '', 10);
    const chat = await answering(page);
    const start = await zoom();
    const c = await box(pixelCanvas(page));
    await page.mouse.click(c.x + c.width / 2, c.y + c.height / 2);
    await expect.poll(zoom).toBe(start * 2);
    await finish(page, chat);
    // Pan: chosen before the answer, used during it.
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('h');
    const chat2 = await answering(page);
    const before = await box(pixelCanvas(page));
    // From the middle of the canvas area: the zoomed canvas can be larger
    // than the area, so its own corner may lie outside the window.
    const area = await box(stage(page));
    const [x, y] = [area.x + area.width / 2, area.y + area.height / 2];
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x + 40, y, { steps: 4 });
    await page.mouse.up();
    expect(Math.round((await box(pixelCanvas(page))).x - before.x)).toBe(40);
    await finish(page, chat2);
  });

  test('image: a toast that shows stays while an answer runs and its Dismiss works (notifications AC-09)', async ({
    page,
  }) => {
    await startDesktop(page, 'image');
    await page.getByRole('button', { name: 'Lock Layer 1', exact: true }).click();
    const c = await box(stage(page).locator('canvas').first());
    await page.mouse.move(c.x + c.width * 0.3, c.y + c.height * 0.3);
    await page.mouse.down();
    await page.mouse.move(c.x + c.width * 0.6, c.y + c.height * 0.6, { steps: 3 });
    await page.mouse.up();
    await expect(toast(page)).toBeVisible();
    const chat = await answering(page);
    await expect(toast(page)).toBeVisible();
    expect(await isInert(toast(page))).toBe(false);
    await toast(page).getByRole('button', { name: 'Dismiss', exact: true }).click();
    await expect(toast(page)).toHaveCount(0);
    await finish(page, chat);
  });
});

test.describe('narrower window', () => {
  test.use({ viewport: { width: 1100, height: 720 } });

  test('image: while an answer runs the options bar is one row and takes no input; afterwards it is as it was (options-bar-layout AC-09)', async ({
    page,
  }) => {
    await startDesktop(page, 'image');
    await setColor(page, 'fg', '#000000');
    await stage(page).click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('t');
    await expect(more(page)).toBeVisible();
    await more(page).click();
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    const chat = await answering(page);
    expect(await isInert(more(page))).toBe(true);
    expect(await isInert(bar(page))).toBe(true);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'false');
    expect(Math.round((await box(bar(page))).height)).toBe(40);
    await finish(page, chat);
    await expect(more(page)).toHaveAttribute('aria-expanded', 'true');
    expect(await isInert(more(page))).toBe(false);
  });
});
