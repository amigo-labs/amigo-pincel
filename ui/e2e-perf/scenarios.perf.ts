// The budget scenarios of docs/specs/pincel.md §4.5, measured in the built
// app (docs/specs/performance-benchmarks.md AC-03). Every scenario reports
// p50 / p95 frame time; painting scenarios also p50 / p95 dab latency
// (pointer event → pixels presented). Results: e2e-perf/results/browser.json.

import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  createDocument,
  gotoStart,
  imageCanvas,
  menu,
  pixelCanvas,
  waitForEditor,
} from '../e2e/helpers';
import {
  durationMetrics,
  frameMetrics,
  gradientPng,
  latencyMetrics,
  record,
  startRecording,
  stopRecording,
  wasmMemoryMb,
  writeResults,
} from './measure';
import { installProbe } from './probe';

const UHD = { width: 3840, height: 2160 };
const RESULTS = fileURLToPath(new URL('./results/browser.json', import.meta.url));

let browserVersion = 'unknown';

test.beforeEach(async ({ page, browser }) => {
  browserVersion = browser.version();
  await page.addInitScript(installProbe);
  // Save / Export take the download fallback (no native picker in headless).
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

test.afterAll(() => {
  writeResults(RESULTS, browserVersion);
});

/** Counts the draws on `surface`'s canvas only (not thumbnails, not off-screen buffers). */
async function target(surface: Locator): Promise<void> {
  await surface.evaluate((el) => {
    window.__perf.target =
      el instanceof HTMLCanvasElement
        ? el
        : (el.querySelector('canvas') ?? el.closest('main')?.querySelector('canvas') ?? null);
  });
}

/** Center and size of the on-screen part of `surface`, in page coordinates. */
async function box(surface: Locator): Promise<{ x: number; y: number; w: number; h: number }> {
  const b = await surface.boundingBox();
  const vp = surface.page().viewportSize();
  if (!b || !vp) throw new Error('surface not visible');
  const left = Math.max(0, b.x);
  const top = Math.max(0, b.y);
  const w = Math.min(vp.width, b.x + b.width) - left;
  const h = Math.min(vp.height, b.y + b.height) - top;
  return { x: left + w / 2, y: top + h / 2, w, h };
}

/** Moves the pointer `steps` times along a line, one event per frame (~16 ms). */
async function sweep(
  page: Page,
  from: [number, number],
  to: [number, number],
  steps: number,
): Promise<void> {
  for (let i = 1; i <= steps; i++) {
    const f = i / steps;
    await page.mouse.move(from[0] + (to[0] - from[0]) * f, from[1] + (to[1] - from[1]) * f);
    await page.waitForTimeout(16);
  }
}

/** A drag with the primary button, `steps` pointer events long. */
async function stroke(
  page: Page,
  from: [number, number],
  to: [number, number],
  steps: number,
): Promise<void> {
  await page.mouse.move(...from);
  await page.mouse.down();
  await sweep(page, from, to, steps);
  await page.mouse.up();
}

/** A 4K Image document with 10 layers, the probe aimed at its canvas. */
async function imageDoc4k(page: Page): Promise<Locator> {
  await createDocument(page, 'image', UHD);
  for (let i = 0; i < 9; i++) await menu(page, 'Layer', 'New Layer');
  await expect(page.getByRole('button', { name: /^Hide / })).toHaveCount(10);
  const surface = imageCanvas(page);
  await target(surface);
  return surface;
}

test('image 4K, 10 layers: brush, pan / zoom, recompose, effect, save and export', async ({
  page,
}) => {
  const surface = await imageDoc4k(page);
  await page.keyboard.press('b');
  const c = await box(surface);
  const span = Math.min(c.w, c.h) * 0.3;

  await test.step('brush dab → pixels on screen, at fit zoom and at 100 %', async () => {
    await startRecording(page);
    await stroke(page, [c.x - span, c.y - span / 2], [c.x + span, c.y + span / 2], 60);
    await page.getByRole('button', { name: 'Actual size' }).click();
    await stroke(page, [c.x - span, c.y + span / 2], [c.x + span, c.y - span / 2], 60);
    const rec = await stopRecording(page);
    expect(rec.latencies.length).toBeGreaterThan(0);
    record('image-brush-4k', 'Brush dab, Image 3840x2160, 10 layers', {
      ...frameMetrics(rec),
      ...latencyMetrics(rec),
    });
    await page.getByRole('button', { name: /Fit to window/ }).click();
  });

  await test.step('pan (middle-button drag) and zoom (wheel)', async () => {
    await startRecording(page);
    await page.mouse.move(c.x, c.y);
    await page.mouse.down({ button: 'middle' });
    await sweep(page, [c.x, c.y], [c.x + span, c.y + span / 2], 60);
    await page.mouse.up({ button: 'middle' });
    for (let i = 0; i < 20; i++) {
      await page.mouse.wheel(0, i < 10 ? -120 : 120);
      await page.waitForTimeout(16);
    }
    const rec = await stopRecording(page);
    record('image-pan-zoom-4k', 'Pan / zoom, Image 4K 10 layers', frameMetrics(rec));
    await page.getByRole('button', { name: /Fit to window/ }).click();
  });

  await test.step('full recompose (toggle a layer’s visibility)', async () => {
    await startRecording(page);
    for (let i = 0; i < 6; i++) {
      await page.getByRole('button', { name: /^(Hide|Show) Layer 2$/ }).click();
      await page.waitForTimeout(300);
    }
    const rec = await stopRecording(page);
    expect(rec.clicks.length).toBeGreaterThan(0);
    record('image-recompose-4k', 'Full recompose, Image 4K 10 layers', {
      ...frameMetrics(rec),
      ...durationMetrics('recompose', rec.clicks),
    });
  });

  await test.step('effect preview and result (Gaussian r = 10)', async () => {
    await menu(page, 'Effects', 'Blur', 'Gaussian Blur…');
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('heading', { name: 'Gaussian Blur' })).toBeVisible();
    await startRecording(page);
    await dialog.getByLabel('Radius value').fill('10');
    await page.waitForTimeout(3_000);
    const preview = await stopRecording(page, 0);
    const proxyDraw = preview.draws.find((d) => d > (preview.marks.input ?? Infinity));
    expect(proxyDraw, 'the preview reached the canvas').toBeDefined();
    await startRecording(page);
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toBeHidden({ timeout: 60_000 });
    const full = await stopRecording(page, 3_000);
    expect(full.clicks.length).toBeGreaterThan(0);
    record('image-gaussian-4k', 'Effect preview (Gaussian r = 10) on 4K', {
      ...frameMetrics({ ...full, frames: [...preview.frames, ...full.frames] }),
      proxy: Math.round(((proxyDraw ?? 0) - (preview.marks.input ?? 0)) * 100) / 100,
      full: Math.round(Math.max(...full.clicks) * 100) / 100,
    });
  });

  await test.step('save (ORA, the autosave encoder) and export PNG', async () => {
    await startRecording(page);
    const saved = page.waitForEvent('download', { timeout: 120_000 });
    await page.keyboard.press('Control+s');
    await saved;
    const exported = page.waitForEvent('download', { timeout: 120_000 });
    await menu(page, 'File', 'Export', 'PNG');
    const dialog = page.getByRole('dialog');
    if (await dialog.isVisible()) {
      await dialog.getByRole('button', { name: /^(Export|OK|Save)$/ }).click();
    }
    await exported;
    const rec = await stopRecording(page);
    record('image-autosave-export-4k', 'Save and export of a 4K 10-layer doc', frameMetrics(rec));
  });
});

test('image 4K, 10 layers: memory after 50 undo steps', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const surface = await imageDoc4k(page);
  await page.keyboard.press('b');
  const c = await box(surface);
  await startRecording(page);
  // 40 strokes of at most 512 canvas px (the view is at fit zoom, < 1:1).
  for (let i = 0; i < 40; i++) {
    const x = c.x - c.w * 0.4 + (i % 8) * c.w * 0.1;
    const y = c.y - c.h * 0.3 + Math.floor(i / 8) * c.h * 0.12;
    await stroke(page, [x, y], [x + 60, y + 30], 4);
  }
  // 5 whole-layer adjustments.
  for (let i = 0; i < 5; i++) {
    await menu(page, 'Adjustments', 'Invert Colors');
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: 'OK' }).click();
    await expect(dialog).toBeHidden({ timeout: 60_000 });
  }
  // 3 merge-downs.
  for (let i = 0; i < 3; i++) await menu(page, 'Layer', 'Merge Down');
  // 2 pastes of 1024².
  for (let i = 0; i < 2; i++) {
    await page.evaluate(async () => {
      const canvas = new OffscreenCanvas(1024, 1024);
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('no 2d context');
      ctx.fillStyle = '#29adff';
      ctx.fillRect(0, 0, 1024, 1024);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
    });
    await menu(page, 'Edit', 'Paste');
  }
  await expect(page.getByRole('button', { name: /^Hide / })).toHaveCount(9, { timeout: 60_000 });
  const rec = await stopRecording(page);
  record('image-memory-4k', 'Memory, 4K x 10 layers with 50 undo steps', {
    ...frameMetrics(rec),
    'wasm-memory-mb': await wasmMemoryMb(page),
  });
});

test('pixel: pencil drag on 256×256 at zoom 32', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 256, height: 256 });
  const surface = pixelCanvas(page);
  const zoom = page.getByRole('button', { name: /Fit to window/ });
  for (let i = 0; i < 8 && !(await zoom.textContent())?.includes('32'); i++) {
    await page.keyboard.press('+');
  }
  await expect(zoom).toContainText('32');
  await target(surface);
  await page.getByTitle('Pencil (B)').click();
  const c = await box(surface);
  // Every pointer event must reach a new sprite pixel (32 CSS px apart) or
  // it paints nothing and has no frame to wait for: a boustrophedon over
  // 6 rows of 20 pixels, 120 events.
  const step = 32;
  const x0 = c.x - 10 * step;
  const y0 = c.y - 3 * step;
  await startRecording(page);
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  for (let row = 0; row < 6; row++) {
    for (let i = 1; i <= 20; i++) {
      const x = row % 2 === 0 ? x0 + i * step : x0 + (20 - i) * step;
      await page.mouse.move(x, y0 + row * step + (i === 20 ? step : 0));
      await page.waitForTimeout(16);
    }
  }
  await page.mouse.up();
  const rec = await stopRecording(page);
  expect(rec.latencies.length).toBeGreaterThan(0);
  record('pixel-pencil-256-z32', 'Pixel: pencil drag on 256x256 at zoom 32', {
    ...frameMetrics(rec),
    ...latencyMetrics(rec),
  });
});

test('open a 4K PNG: time to first paint', async ({ page }) => {
  await gotoStart(page);
  await page.evaluate(() => {
    new MutationObserver((records) => {
      for (const r of records) {
        if (r.target instanceof Element && r.target.getAttribute('data-ready') === 'true') {
          window.__perf.marks.ready ??= performance.now();
        }
      }
    }).observe(document, { subtree: true, attributes: true, attributeFilter: ['data-ready'] });
  });
  const png = gradientPng(UHD.width, UHD.height);
  await startRecording(page);
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('button', { name: 'Open…' }).click(),
  ]);
  await chooser.setFiles({ name: 'uhd.png', mimeType: 'image/png', buffer: png });
  await waitForEditor(page, 'image');
  const rec = await stopRecording(page);
  const { change, ready } = rec.marks;
  if (change === undefined || ready === undefined) throw new Error('no change / ready mark');
  // Frame timestamps are absolute; the first frame after `ready` shows it.
  const shown = await page.evaluate((r) => window.__perf.frames.find((f) => f > r) ?? r, ready);
  record('image-open-4k-png', 'Open a 4K PNG', {
    ...frameMetrics(rec),
    'first-paint': Math.round((shown - change) * 100) / 100,
  });
});

test('start screen: cold start on Slow 4G, then warm', async ({ page }) => {
  await page.addInitScript(() => {
    window.__perf.recording = true;
  });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Network.enable');
  // Lighthouse "Slow 4G": 150 ms RTT, 1.6 Mbps down, 750 kbps up; no CPU throttling.
  await cdp.send('Network.emulateNetworkConditions', {
    offline: false,
    latency: 150,
    downloadThroughput: (1.6 * 1024 * 1024) / 8,
    uploadThroughput: (750 * 1024) / 8,
  });
  const interactive = (): Promise<number> =>
    page
      .waitForFunction(
        () => {
          const heading = [...document.querySelectorAll('h1, h2')].some(
            (h) => h.textContent?.trim() === 'Pincel',
          );
          const button = document.querySelector('button:not([disabled])');
          return heading && button !== null ? performance.now() : false;
        },
        undefined,
        { polling: 'raf', timeout: 120_000 },
      )
      .then((h) => h.jsonValue() as Promise<number>);

  await cdp.send('Network.setCacheDisabled', { cacheDisabled: true });
  await page.goto('./');
  const cold = await interactive();
  const coldFrames = (await stopRecording(page, 0)).frames;

  await cdp.send('Network.setCacheDisabled', { cacheDisabled: false });
  await page.goto('./'); // fills the HTTP cache
  await interactive();
  await page.goto('./');
  const warm = await interactive();
  const warmFrames = (await stopRecording(page, 0)).frames;
  record('cold-start', 'Start screen interactive (Slow 4G)', {
    ...frameMetrics({
      frames: [...coldFrames, ...warmFrames],
      latencies: [],
      clicks: [],
      draws: [],
      marks: {},
    }),
    cold: Math.round(cold),
    warm: Math.round(warm),
  });
});
