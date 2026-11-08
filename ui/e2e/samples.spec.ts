import { statSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import { gotoStart, waitForEditor } from './helpers';

// "Open a sample" on the start screen (docs/specs/first-run-tutorial.md
// AC-13 – AC-16). The samples' structure is checked by the cores'
// `tests/samples.rs`; here they open through the app, also offline.

const layerRows = (page: Page) => page.getByRole('button', { name: /^Hide / });

async function openSample(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: 'Open a sample' }).click();
  await page.getByRole('group', { name: 'Open a sample' }).getByRole('button', { name }).click();
}

test('the sprite sample opens in Pixel mode with its two layers and four frames (AC-13)', async ({ page }) => {
  await gotoStart(page);
  await openSample(page, 'Bouncing slime (Pixel)');
  await waitForEditor(page, 'pixel');
  await expect(page.getByTitle('slime.aseprite', { exact: true })).toHaveCount(1);
  await expect(layerRows(page)).toHaveCount(2);
  // The tag spans all four frames.
  await expect(page.getByRole('button', { name: 'Tag bounce' })).toHaveAttribute(
    'title',
    /frames 1–4/,
  );
});

test('the image sample opens in Image mode with three layers (AC-14)', async ({ page }) => {
  await gotoStart(page);
  await openSample(page, 'Postcard (Image)');
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('postcard.ora', { exact: true })).toHaveCount(1);
  await expect(layerRows(page)).toHaveCount(3);
});

test('each sample file is at most 100 KB and comes with its CC0 note (AC-16)', () => {
  const dir = join(import.meta.dirname, '../public/samples');
  for (const f of ['slime.aseprite', 'postcard.ora']) {
    expect(statSync(join(dir, f)).size).toBeLessThanOrEqual(100 * 1024);
  }
  expect(statSync(join(dir, 'LICENSE.md')).size).toBeGreaterThan(0);
});

test.describe('installed web app', () => {
  test.use({ serviceWorkers: 'allow' });

  test('both samples open offline once the app was loaded (AC-15)', async ({ page, context }) => {
    await gotoStart(page);
    await page.evaluate(async () => {
      await navigator.serviceWorker.ready;
    });
    await expect
      .poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null), {
        timeout: 30_000,
      })
      .toBe(true);
    await context.setOffline(true);
    await page.reload();
    await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
    await openSample(page, 'Bouncing slime (Pixel)');
    await waitForEditor(page, 'pixel');
    await gotoStart(page);
    await openSample(page, 'Postcard (Image)');
    await waitForEditor(page, 'image');
  });
});
