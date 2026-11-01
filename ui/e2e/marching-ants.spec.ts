import { expect, test, type Page } from '@playwright/test';
import { createDocument } from './helpers';

// Image mode's selection outline marches: dashes run along the outline, so
// every frame shows white and dark ants side by side. Drawn as separate
// one-pixel edges, the dash restarted on every edge and, zoomed out, the
// whole outline switched between white and dark at once.

/** White and dark outline pixels on the selection overlay right now. */
function antCounts(page: Page): Promise<{ white: number; dark: number }> {
  return page.getByTestId('selection-overlay').evaluate((el) => {
    const canvas = el as HTMLCanvasElement;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2d context');
    const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    let white = 0;
    let dark = 0;
    for (let i = 0; i < data.length; i += 4) {
      const r = data[i] ?? 0;
      const a = data[i + 3] ?? 0;
      if (a < 128) continue;
      if (r >= 160) white += 1;
      else if (r <= 80) dark += 1;
    }
    return { white, dark };
  });
}

test('image: zoomed out, the selection outline shows white and dark ants in every frame', async ({
  page,
}) => {
  // Larger than the viewport: the fitted zoom is below 1.
  await createDocument(page, 'image', { width: 2400, height: 1800 });
  await page.keyboard.press('ControlOrMeta+a');
  await expect
    .poll(async () => (await antCounts(page)).white + (await antCounts(page)).dark)
    .toBeGreaterThan(0);
  for (let frame = 0; frame < 6; frame++) {
    const { white, dark } = await antCounts(page);
    const total = white + dark;
    expect(white / total, `frame ${frame}: ${white} white, ${dark} dark`).toBeGreaterThan(0.1);
    expect(dark / total, `frame ${frame}: ${white} white, ${dark} dark`).toBeGreaterThan(0.1);
    await page.waitForTimeout(70);
  }
});
