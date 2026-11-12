import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { NATIVE_JOB_MIN_PIXELS } from '../src/lib/jobs/native';
import { createDocument, drag, imageCanvas, menu, newDocument, setColor } from './helpers';
import { startNativeJobServer, type NativeJobServer } from './native-job-server';
import { diskFile, installTauriMock, launchDesktop, putFile, setMock } from './tauri-mock';

// Which backend runs an Image job (docs/specs/performance-desktop-jobs.md
// AC-03): measured on the reference machine, no document size of either
// job kind pays off natively against the SIMD worker (`NATIVE_JOB_MIN_PIXELS`,
// the spec's Decisions 2026-11-12), so the desktop app runs every job in the
// Web Worker like the PWA — even with the native endpoint reachable. The
// desktop app is a mocked Tauri bridge (`tauri-mock.ts`) whose
// `job_endpoint` names a stand-in endpoint (`native-job-server.ts`). The
// native-first runner and its fallback (AC-06) are tested in
// `native-jobs.spec.ts`.

/** A 4 Mpx document, past any crossover the measurement considered. */
const LARGE = { width: 2048, height: 2048 };
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** Console errors and the workers the page starts, from now on. */
function watch(page: Page): { errors: string[]; workers: string[] } {
  const seen = { errors: [] as string[], workers: [] as string[] };
  page.on('console', (m) => {
    if (m.type() === 'error') seen.errors.push(m.text());
  });
  page.on('worker', (w) => seen.workers.push(w.url()));
  return seen;
}

/** The Image job worker (not the native fetch worker) ran. */
const jobWorker = (urls: string[]): boolean => urls.some((u) => /\/assets\/worker-/.test(u));

/** One black stroke across the canvas. */
async function stroke(page: Page): Promise<void> {
  await setColor(page, 'fg', '#000000');
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.1, 0.2], [0.9, 0.7]);
}

/** A new desktop Image document of `size` with one black stroke. */
async function desktopDocument(page: Page, size: { width: number; height: number }) {
  await launchDesktop(page);
  await newDocument(page, 'image', size);
  await stroke(page);
}

/** Opens Gaussian Blur at radius 3 and applies it. */
async function blur(page: Page): Promise<void> {
  await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
  const panel = page.getByRole('dialog', { name: 'Gaussian Blur' });
  await panel.getByLabel('Radius value', { exact: true }).fill('3');
  await panel.getByRole('button', { name: 'OK' }).click();
  await expect(panel).toHaveCount(0);
}

/** Exports the document as PNG through the native save dialog to `path`
 *  and returns the written bytes. */
async function exportPng(page: Page, path: string): Promise<Buffer> {
  await putFile(page, path, null);
  await setMock(page, { savePick: path });
  await menu(page, 'File', 'Export', 'PNG');
  await expect.poll(async () => (await diskFile(page, path)) !== null).toBe(true);
  const bytes = await diskFile(page, path);
  if (!bytes) throw new Error(`${path} was not written`);
  return bytes;
}

test('no job kind has a crossover size (AC-03)', () => {
  expect(NATIVE_JOB_MIN_PIXELS).toEqual({ effect: null, encode: null });
});

test.describe('desktop app', () => {
  let server: NativeJobServer;

  test.beforeEach(async ({ page }) => {
    server = await startNativeJobServer();
    await installTauriMock(page);
  });

  test.afterEach(async () => {
    await server.close();
  });

  /** Lets `job_endpoint` name the stand-in endpoint. */
  const useEndpoint = (page: Page) =>
    setMock(page, { jobEndpoint: { url: server.url, token: server.token } });

  test('with the native endpoint reachable, an effect and an encode on a large document run in the worker (AC-03)', async ({
    page,
  }) => {
    const seen = watch(page);
    await desktopDocument(page, LARGE);
    await useEndpoint(page);
    const before = await exportPng(page, '/docs/before.png');
    expect(before.subarray(0, 8).equals(PNG_SIGNATURE)).toBe(true);
    await blur(page);
    const after = await exportPng(page, '/docs/after.png');
    expect(after.equals(before), 'the blur changed the layer').toBe(false);
    expect(server.jobs).toEqual([]);
    expect(jobWorker(seen.workers), 'a job worker ran').toBe(true);
    expect(
      seen.workers.some((u) => /native-worker/.test(u)),
      'no native fetch worker',
    ).toBe(false);
    expect(seen.errors).toEqual([]);
  });
});

test('every job in the PWA runs in the worker (AC-03)', async ({ page }) => {
  // Downloads instead of the File System Access save picker.
  await page.addInitScript(() => {
    delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
  });
  const seen = watch(page);
  await createDocument(page, 'image', LARGE);
  expect(await page.evaluate(() => '__TAURI_INTERNALS__' in window)).toBe(false);
  await stroke(page);
  await blur(page);
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    menu(page, 'File', 'Export', 'PNG'),
  ]);
  const png = await readFile(await download.path());
  expect(png.subarray(0, 8).equals(PNG_SIGNATURE)).toBe(true);
  expect(jobWorker(seen.workers), 'a job worker ran').toBe(true);
  expect(
    seen.workers.some((u) => /native-worker/.test(u)),
    'no native fetch worker',
  ).toBe(false);
  expect(seen.errors).toEqual([]);
});
