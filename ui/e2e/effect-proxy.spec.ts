import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, setColor } from './helpers';
import { PROXY_EFFECTS, proxyParams } from '../src/lib/effects/proxy';
import { EFFECTS } from '../src/lib/effects/catalog';

// The instant reduced-size effect preview
// (docs/specs/performance-background-jobs.md AC-05): computed on the main
// thread at the canvas's on-screen size, shown until the job worker's
// full-resolution preview replaces it. The canvas publishes which one it
// shows as `data-preview` (`proxy` / `true`).

/** A large painted Image document, fitted well below 50 % zoom. */
async function largeDocument(page: Page, size = { width: 2400, height: 1600 }): Promise<void> {
  await createDocument(page, 'image', size);
  await setColor(page, 'fg', '#000000');
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.1, 0.2], [0.9, 0.7]);
}

/** A job worker that never answers, so no full-resolution preview comes. */
async function silentWorker(page: Page): Promise<void> {
  await page.route(/\/assets\/worker-[^/]*\.js$/, (route) =>
    route.fulfill({ contentType: 'text/javascript', body: 'self.onmessage = () => {};' }),
  );
}

/** Records every value the canvas's `data-preview` takes from now on. */
async function watchPreview(page: Page): Promise<() => Promise<string[]>> {
  await imageCanvas(page).evaluate((el) => {
    const seen: string[] = [];
    (window as { __previewSeen?: string[] }).__previewSeen = seen;
    new MutationObserver(() => seen.push(el.getAttribute('data-preview') ?? '')).observe(el, {
      attributes: true,
      attributeFilter: ['data-preview'],
    });
  });
  return () => page.evaluate(() => (window as { __previewSeen?: string[] }).__previewSeen ?? []);
}

async function gaussian(page: Page, radius: string) {
  await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
  const panel = page.getByRole('dialog', { name: 'Gaussian Blur' });
  await panel.getByLabel('Radius value', { exact: true }).fill(radius);
  return panel;
}

test('a large document shows the reduced-size preview before the worker answers (AC-05)', async ({
  page,
}) => {
  await silentWorker(page);
  await largeDocument(page);
  const canvas = imageCanvas(page);
  const before = await canvas.screenshot();
  const panel = await gaussian(page, '10');
  await expect(canvas).toHaveAttribute('data-preview', 'proxy');
  expect((await canvas.screenshot({ mask: [panel] })).equals(before)).toBe(false);
  await panel.getByRole('button', { name: 'Cancel' }).click();
  await expect(canvas).toHaveAttribute('data-preview', 'false');
});

test('the full-resolution preview replaces the reduced-size one (AC-05)', async ({ page }) => {
  await largeDocument(page);
  const seen = await watchPreview(page);
  await gaussian(page, '10');
  await expect(imageCanvas(page)).toHaveAttribute('data-preview', 'true', { timeout: 30_000 });
  const values = await seen();
  expect(values).toContain('proxy');
  expect(values.indexOf('proxy')).toBeLessThan(values.lastIndexOf('true'));
});

test('a small document shows no reduced-size preview (AC-05)', async ({ page }) => {
  await createDocument(page, 'image', { width: 160, height: 90 });
  const seen = await watchPreview(page);
  await gaussian(page, '4');
  await expect(imageCanvas(page)).toHaveAttribute('data-preview', 'true');
  expect(await seen()).not.toContain('proxy');
});

test('proxy parameters scale lengths and keep everything else (AC-05)', () => {
  expect(proxyParams('gaussian_blur', [10], 0.25)).toEqual([2.5]);
  expect(proxyParams('gaussian_blur', [0.2], 0.25)).toEqual([0.1]);
  expect(proxyParams('unsharp_mask', [50, 4, 7], 0.5)).toEqual([50, 2, 7]);
  expect(proxyParams('morphology', [1, 0, 9], 0.25)).toEqual([1, 0, 2]);
  expect(proxyParams('drop_shadow', [-8, 6, 4, 0xff, 50], 0.25)).toEqual([-2, 2, 1, 0xff, 50]);
  expect(proxyParams('curves', [0, 0, 0, 255, 255], 0.25)).toEqual([0, 0, 0, 255, 255]);
  // Distortions and renders have no proxy.
  expect(proxyParams('twist', [10, 100, 0.5, 0.5, 0], 0.25)).toBeNull();
  // Every effect with a proxy is in the catalog.
  const ids = new Set(EFFECTS.map((e) => e.id));
  for (const id of PROXY_EFFECTS) expect(ids.has(id), id).toBe(true);
});
