import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, newDocument, setColor } from './helpers';
import {
  diskFile,
  installTauriMock,
  launchDesktop,
  nativeJobs,
  putFile,
  setMock,
} from './tauri-mock';

// Which backend runs an Image job (docs/specs/performance-desktop-jobs.md
// AC-03): no document size pays off natively yet (`NATIVE_JOB_MIN_PIXELS`
// is null, see the spec's Decisions), so the desktop app runs every job in
// the Web Worker like the PWA. The desktop app is a mocked Tauri bridge
// (`tauri-mock.ts`) whose `image_job` command would answer if it were asked.
// The native-first runner itself is tested in `native-jobs.spec.ts`.

/** A document of a million pixels, a common crossover candidate. */
const LARGE = { width: 1000, height: 1000 };
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** Console errors and the job workers the page starts, from now on. */
function watch(page: Page): { errors: string[]; workers: string[] } {
  const seen = { errors: [] as string[], workers: [] as string[] };
  page.on('console', (m) => {
    if (m.type() === 'error') seen.errors.push(m.text());
  });
  page.on('worker', (w) => seen.workers.push(w.url()));
  return seen;
}

const jobWorker = (urls: string[]): boolean => urls.some((u) => /\/assets\/worker-/.test(u));

/** A new desktop Image document of `size` with one black stroke. */
async function desktopDocument(page: Page, size: { width: number; height: number }) {
  await launchDesktop(page);
  await newDocument(page, 'image', size);
  await setColor(page, 'fg', '#000000');
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.1, 0.2], [0.9, 0.7]);
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

test.describe('desktop app', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  test('without a crossover size a large document runs its jobs in the worker (AC-03)', async ({
    page,
  }) => {
    const seen = watch(page);
    await desktopDocument(page, LARGE);
    await setMock(page, { nativeJob: 'answer', nativeAnswer: [1, 2, 3, 4] });
    const png = await exportPng(page, '/docs/large.png');
    expect(png.subarray(0, 8).equals(PNG_SIGNATURE)).toBe(true);
    expect(await nativeJobs(page)).toEqual([]);
    expect(jobWorker(seen.workers), 'a job worker ran').toBe(true);
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
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    menu(page, 'File', 'Export', 'PNG'),
  ]);
  const png = await readFile(await download.path());
  expect(png.subarray(0, 8).equals(PNG_SIGNATURE)).toBe(true);
  expect(jobWorker(seen.workers), 'a job worker ran').toBe(true);
  expect(seen.errors).toEqual([]);
});
