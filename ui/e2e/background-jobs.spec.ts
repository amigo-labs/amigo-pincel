import { readFile } from 'node:fs/promises';
import { expect, test, type ConsoleMessage, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, setColor } from './helpers';

// Heavy jobs off the main thread (docs/specs/performance-background-jobs.md):
// Image-mode exports and saves encode in the job worker.

// Downloads instead of the File System Access pickers (native dialogs).
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
});

/** The bytes of the download `action` starts. */
async function download(page: Page, action: () => Promise<void>): Promise<Buffer> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return readFile(await d.path());
}

/** A two-layer document with strokes on both layers. The colours are set
 *  explicitly: the last colours persist across reloads. */
async function paintedDocument(page: Page): Promise<void> {
  await createDocument(page, 'image', { width: 160, height: 90 });
  await setColor(page, 'fg', '#000000');
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.1, 0.2], [0.9, 0.7]);
  await menu(page, 'Layer', 'New Layer');
  await setColor(page, 'fg', '#33cc55');
  await drag(page, imageCanvas(page), [0.2, 0.8], [0.7, 0.1]);
}

/** Lets no job worker start: its script fails to load. */
async function breakWorker(page: Page): Promise<void> {
  await page.route(/\/assets\/worker-[^/]*\.js$/, (route) => route.abort());
}

const EXPORTS = [
  ['PNG', 'PNG'],
  ['WebP', 'WebP (lossless)'],
] as const;

for (const [label, item] of EXPORTS) {
  const exportIt = (page: Page) => () => menu(page, 'File', 'Export', item);
  test(`${label} export from the worker is byte-identical to the main-thread encoding (AC-01, AC-09)`, async ({
    page,
  }) => {
    const logged: string[] = [];
    page.on('console', (m) => {
      if (m.type() === 'error') logged.push(m.text());
    });
    const workers: string[] = [];
    page.on('worker', (w) => workers.push(w.url()));
    await paintedDocument(page);
    const fromWorker = await download(page, exportIt(page));
    expect(
      workers.some((u) => /\/assets\/worker-/.test(u)),
      'a job worker ran',
    ).toBe(true);
    expect(logged).toEqual([]);
    // With the worker unable to start, the job is retried once and then
    // encoded on the main thread, and the failure is logged.
    await breakWorker(page);
    await page.reload();
    await page.evaluate(() => undefined);
    await paintedDocument(page);
    const onMain = await download(page, exportIt(page));
    expect(onMain.equals(fromWorker), 'same bytes').toBe(true);
    expect(logged.some((l) => l.includes('worker job failed twice'))).toBe(true);
    await expect(page.getByText(/failed/i)).toHaveCount(0);
  });
}

test('a stroke made while an export encodes stays in the document and out of the file (AC-03)', async ({
  page,
}) => {
  await paintedDocument(page);
  const before = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  // Start the export and paint before it lands.
  const [d] = await Promise.all([
    page.waitForEvent('download'),
    (async () => {
      await menu(page, 'File', 'Export', 'PNG');
      await setColor(page, 'fg', '#ff00ff');
      await drag(page, imageCanvas(page), [0.05, 0.95], [0.95, 0.05]);
    })(),
  ]);
  const during = await readFile(await d.path());
  expect(during.equals(before), 'the file is the document at export start').toBe(true);
  const after = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  expect(after.equals(before), 'the stroke is in the document').toBe(false);
});

/** Counts snapshot writes into IndexedDB (`autosave_snapshots` puts). */
async function countSnapshotWrites(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as unknown as { snapshotWrites: number };
    w.snapshotWrites = 0;
    const put = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (
      this: IDBObjectStore,
      ...args: Parameters<typeof put>
    ) {
      if (this.name === 'autosave_snapshots') w.snapshotWrites += 1;
      return put.apply(this, args);
    };
  });
}

const snapshotWrites = (page: Page) =>
  page.evaluate(() => (window as unknown as { snapshotWrites: number }).snapshotWrites);

test('a recovered worker autosave is the document as the autosave started (AC-02)', async ({
  page,
}) => {
  await countSnapshotWrites(page);
  await page.clock.install();
  await paintedDocument(page);
  const atStart = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  await page.clock.fastForward(31_000);
  // Painted while the autosave encodes: not part of the snapshot.
  await setColor(page, 'fg', '#ff00ff');
  await drag(page, imageCanvas(page), [0.05, 0.95], [0.95, 0.05]);
  await expect.poll(() => snapshotWrites(page)).toBe(1);
  await page.waitForTimeout(300);

  await page.reload();
  await createDocument(page, 'image', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await dialog.getByRole('button', { name: 'Recover' }).first().click();
  await expect(dialog).toHaveCount(0);
  const recovered = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  expect(recovered.equals(atStart)).toBe(true);
});

test('an autosave tick due while one is still encoding is skipped (AC-10)', async ({ page }) => {
  test.setTimeout(120_000);
  await countSnapshotWrites(page);
  await page.clock.install();
  // Large enough that encoding outlasts the next tick.
  await createDocument(page, 'image', { width: 600, height: 400 });
  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.1, 0.2], [0.9, 0.7]);
  await page.clock.fastForward(31_000);
  await page.clock.fastForward(30_000);
  await expect.poll(() => snapshotWrites(page), { timeout: 45_000 }).toBe(1);
  await page.waitForTimeout(1000);
  expect(await snapshotWrites(page), 'the skipped tick wrote nothing').toBe(1);
  // The next tick after a change saves the latest state.
  await drag(page, imageCanvas(page), [0.2, 0.8], [0.7, 0.1]);
  await page.clock.fastForward(30_000);
  await expect.poll(() => snapshotWrites(page), { timeout: 45_000 }).toBe(2);
});

// Regression: a job worker that fetched and compiled the 12 MB core again
// took one to two and a half seconds for its first job, so an autosave
// landed in IndexedDB long after its tick (recovery.spec.ts reloads 500 ms
// later). The worker instantiates the module the page has compiled.
test('the job worker reuses the page’s compiled core instead of fetching it again', async ({
  page,
}) => {
  const wasmFetches: string[] = [];
  page.context().on('request', (r) => {
    if (/pincel_image_wasm_bg[^/]*\.wasm$/.test(r.url())) wasmFetches.push(r.url());
  });
  const workers: string[] = [];
  page.on('worker', (w) => workers.push(w.url()));
  await paintedDocument(page);
  await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  expect(
    workers.some((u) => /\/assets\/worker-/.test(u)),
    'a job worker ran',
  ).toBe(true);
  expect(wasmFetches, 'the Image core is fetched once').toHaveLength(1);
});

/** Opens the Gaussian blur panel with `radius`. */
async function gaussianPanel(page: Page, radius: string) {
  await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
  const panel = page.getByRole('dialog', { name: 'Gaussian Blur' });
  await panel.getByLabel('Radius value', { exact: true }).fill(radius);
  return panel;
}

/** A painted document blurred through the panel and exported as PNG, and
 *  the console errors logged up to the export. */
async function blurredExport(page: Page): Promise<{ png: Buffer; logged: string[] }> {
  const logged: string[] = [];
  const onConsole = (m: ConsoleMessage) => {
    if (m.type() === 'error') logged.push(m.text());
  };
  page.on('console', onConsole);
  await paintedDocument(page);
  const panel = await gaussianPanel(page, '3');
  await panel.getByRole('button', { name: 'OK' }).click();
  await expect(panel).toHaveCount(0);
  await expect(page.getByText(/failed/i)).toHaveCount(0);
  page.off('console', onConsole);
  return { png: await download(page, () => menu(page, 'File', 'Export', 'PNG')), logged };
}

test('an effect applied in the worker is byte-identical to the main-thread apply (AC-07, AC-09)', async ({
  page,
}) => {
  const workers: string[] = [];
  page.on('worker', (w) => workers.push(w.url()));
  const fromWorker = await blurredExport(page);
  expect(
    workers.some((u) => /\/assets\/worker-/.test(u)),
    'a job worker ran',
  ).toBe(true);
  expect(fromWorker.logged).toEqual([]);

  // Without a worker the effect is retried, then applied on the main
  // thread with a console log and no message to the user.
  const other = await page.context().newPage();
  await other.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
  await breakWorker(other);
  const fromMain = await blurredExport(other);
  expect(
    fromMain.logged.some((m) => m.includes('worker job failed twice')),
    'the effect fallback is logged',
  ).toBe(true);
  expect(fromWorker.png.equals(fromMain.png)).toBe(true);
});

test('cancelling the panel while a preview computes leaves the document unchanged (AC-08)', async ({
  page,
}) => {
  await paintedDocument(page);
  const before = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  const panel = await gaussianPanel(page, '40');
  await panel.getByLabel('Radius value', { exact: true }).fill('60');
  await panel.getByRole('button', { name: 'Cancel' }).click();
  await expect(panel).toHaveCount(0);
  // A preview that finishes after the cancel is discarded.
  await page.waitForTimeout(500);
  const after = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  expect(after.equals(before)).toBe(true);
});
