import { expect, test, type Page } from '@playwright/test';
import {
  colorOf,
  createDocument,
  drag,
  gotoStart,
  imageCanvas,
  menu,
  newDocument,
  pixelCanvas,
  setColor,
  storedPref,
  waitForEditor,
} from './helpers';
import { tabs } from './tab-helpers';

// Autosave and recovery (spec §10, M10.3; docs/specs/autosave-recovery.md),
// the same in both modes: an edit is snapshotted after 30 s, a reload
// offers it in the Recovery dialog, and the recovered document reads as
// unsaved. Snapshots are read (and, for listing and error cases, seeded)
// straight in IndexedDB, the way the app stores them.

type Mode = 'pixel' | 'image';

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** One `autosave_meta` row, as the app writes it. */
interface Meta {
  docId: string;
  ts: number;
  name: string;
  byteLength: number;
  mode?: Mode;
}

/** Removes the File System Access pickers, so Save falls back to a download. */
async function downloadSaves(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
}

/** Every snapshot metadata row in the app's IndexedDB. */
async function metaRows(page: Page): Promise<Meta[]> {
  return page.evaluate(
    () =>
      new Promise<Meta[]>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const all = open.result
            .transaction('autosave_meta')
            .objectStore('autosave_meta')
            .getAll();
          all.onsuccess = () => {
            open.result.close();
            resolve(all.result as Meta[]);
          };
          all.onerror = () => reject(all.error);
        };
      }),
  );
}

/**
 * Writes snapshot rows the way an earlier page load would have left them:
 * metadata, plus the payload rows given in `bytes` (keyed like the meta).
 * Runs on the start screen, once the app has created its database.
 */
async function seedSnapshots(
  page: Page,
  metas: Meta[],
  bytes: { docId: string; ts: number; bytes: number[] }[] = [],
): Promise<void> {
  await expect
    .poll(() =>
      page.evaluate(async () =>
        // The autosave stores exist from schema version 2 on.
        (await indexedDB.databases()).some((d) => d.name === 'pincel' && (d.version ?? 0) >= 2),
      ),
    )
    .toBe(true);
  await page.evaluate(
    ([m, b]) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const tx = open.result.transaction(['autosave_meta', 'autosave_snapshots'], 'readwrite');
          for (const row of m) tx.objectStore('autosave_meta').put(row);
          for (const row of b) {
            tx.objectStore('autosave_snapshots').put({ ...row, bytes: new Uint8Array(row.bytes) });
          }
          tx.oncomplete = () => {
            open.result.close();
            resolve();
          };
          tx.onerror = () => reject(tx.error);
        };
      }),
    [metas, bytes] as const,
  );
}

/** Edits the open document and lets the 30 s autosave write it. */
async function autosavedEdit(page: Page, mode: Mode): Promise<void> {
  if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
  await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.6]);
  await page.clock.fastForward(31_000);
  await expect.poll(async () => (await metaRows(page)).length).toBeGreaterThan(0);
}

/** Gives an autosave tick (and the probe) time to (not) happen. */
async function settle(page: Page): Promise<void> {
  await page.waitForTimeout(500);
}

const recoveryDialog = (page: Page) => page.getByRole('dialog', { name: 'Recover unsaved work?' });

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: an autosaved edit is offered and recovered after a reload`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.6]);
    await page.clock.fastForward(31_000);
    // Let the IndexedDB write settle before leaving the page.
    await page.waitForTimeout(500);

    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = recoveryDialog(page);
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Recover' }).first().click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByTitle(/unsaved changes/)).toBeVisible();
  });

  test(`${mode}: a clean or unchanged document is not snapshotted`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    const undo = page.getByRole('button', { name: 'Undo', exact: true });

    // Just created: clean.
    await page.clock.fastForward(31_000);
    await settle(page);
    expect(await metaRows(page)).toEqual([]);

    // Edited and undone: back at the depth it was opened with.
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.6]);
    await undo.click();
    await expect(undo).toBeDisabled();
    await page.clock.fastForward(31_000);
    await settle(page);
    expect(await metaRows(page)).toEqual([]);

    // Edited: one snapshot, which the next tick leaves alone.
    await autosavedEdit(page, mode);
    const [first] = await metaRows(page);
    await page.clock.fastForward(31_000);
    await settle(page);
    expect(await metaRows(page)).toEqual([first]);

    // A further edit is written again (a newer snapshot replaces it).
    await drag(page, canvasOf(page, mode), [0.2, 0.8], [0.8, 0.8]);
    await page.clock.fastForward(31_000);
    await expect.poll(async () => (await metaRows(page))[0]?.ts).toBeGreaterThan(first?.ts ?? 0);
  });

  test(`${mode}: saving deletes the snapshot and it is not offered again`, async ({ page }) => {
    await downloadSaves(page);
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    await autosavedEdit(page, mode);

    await Promise.all([
      page.waitForEvent('download'),
      menu(page, 'File', mode === 'pixel' ? 'Save (download)' : 'Save'),
    ]);
    await expect.poll(async () => (await metaRows(page)).length).toBe(0);
    // Saved means clean: the next tick writes nothing either.
    await page.clock.fastForward(31_000);
    await settle(page);
    expect(await metaRows(page)).toEqual([]);

    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    await settle(page);
    await expect(recoveryDialog(page)).toHaveCount(0);
  });

  test(`${mode}: a recovered document keeps its snapshot until it is saved`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    await autosavedEdit(page, mode);
    const [written] = await metaRows(page);

    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = recoveryDialog(page);
    await dialog.getByRole('button', { name: 'Recover' }).click();
    await expect(dialog).toHaveCount(0);
    await settle(page);
    expect(await metaRows(page)).toEqual([written]);

    // The tab closes right after recovering: the work is offered again.
    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('listitem')).toHaveCount(1);
  });

  test(`${mode}: a snapshot that cannot be recovered shows its error and keeps the dialog`, async ({
    page,
  }) => {
    await gotoStart(page);
    const ts = Date.UTC(2026, 9, 1, 12, 0);
    await seedSnapshots(
      page,
      [
        { docId: 'missing', ts, name: 'missing-bytes', byteLength: 64, mode },
        { docId: 'garbage', ts: ts - 1000, name: 'garbage-bytes', byteLength: 4, mode },
      ],
      [{ docId: 'garbage', ts: ts - 1000, bytes: [1, 2, 3, 4] }],
    );
    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = recoveryDialog(page);
    await expect(dialog.getByRole('listitem')).toHaveCount(2);

    for (const name of ['missing-bytes', 'garbage-bytes']) {
      const row = dialog.getByRole('listitem').filter({ hasText: name });
      await row.getByRole('button', { name: 'Recover' }).click();
      await expect(row.getByRole('alert')).not.toBeEmpty();
      await expect(dialog).toBeVisible();
    }
    await expect(dialog.getByRole('listitem')).toHaveCount(2);
  });
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: each Recover opens its document as a new active, unsaved tab; the dialog stays for the rest (AC-05)`, async ({
    page,
  }) => {
    await page.clock.install();
    // Two documents with autosaved edits: the first, then a second tab.
    await createDocument(page, mode, { width: 32, height: 32 });
    await autosavedEdit(page, mode);
    await menu(page, 'File', 'New…');
    await page
      .getByRole('dialog', { name: 'New document' })
      .getByRole('button', { name: 'Create' })
      .click();
    await expect(tabs(page)).toHaveCount(2);
    await autosavedEdit(page, mode);
    await expect.poll(async () => (await metaRows(page)).length).toBe(2);
    await settle(page);

    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    await expect(tabs(page)).toHaveCount(1);
    const dialog = recoveryDialog(page);
    const rows = dialog.getByRole('listitem');
    await expect(rows).toHaveCount(2);

    // The first Recover: a second tab, active and unsaved; one row left.
    await rows.first().getByRole('button', { name: 'Recover' }).click();
    await expect(tabs(page)).toHaveCount(2);
    await expect(tabs(page).nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs(page).nth(1).getByText('unsaved changes')).toHaveCount(1);
    await expect(tabs(page).nth(0).getByText('unsaved changes')).toHaveCount(0);
    await expect(dialog).toBeVisible();
    await expect(rows).toHaveCount(1);

    // The last Recover: a third tab, and the dialog closes.
    await rows.first().getByRole('button', { name: 'Recover' }).click();
    await expect(tabs(page)).toHaveCount(3);
    await expect(tabs(page).nth(2)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs(page).nth(2).getByText('unsaved changes')).toHaveCount(1);
    await expect(dialog).toHaveCount(0);
  });
}

test('the dialog lists each document’s latest snapshot of both modes, newest first (AC-04)', async ({
  page,
}) => {
  await gotoStart(page);
  const base = Date.UTC(2026, 9, 1, 12, 0);
  const metas: Meta[] = [
    {
      docId: 'older',
      ts: base - 3_600_000,
      name: 'older.aseprite',
      byteLength: 2048,
      mode: 'pixel',
    },
    { docId: 'newer', ts: base - 60_000, name: 'newer.aseprite', byteLength: 512, mode: 'pixel' },
    // An earlier row of the same document (a partial write): not listed.
    { docId: 'newer', ts: base - 7_200_000, name: 'stale.aseprite', byteLength: 1, mode: 'pixel' },
    { docId: 'photo', ts: base, name: 'photo.ora', byteLength: 4096, mode: 'image' },
  ];
  await seedSnapshots(page, metas);
  const when = (ts: number) => page.evaluate((t) => new Date(t).toLocaleString(), ts);
  const dialog = recoveryDialog(page);
  const rows = dialog.getByRole('listitem');

  // Whichever mode's editor starts first, the dialog lists both modes.
  for (const mode of ['pixel', 'image'] as const) {
    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    await expect(rows).toHaveCount(3);
    await expect(rows.nth(0)).toContainText('photo.ora');
    await expect(rows.nth(0)).toContainText('Image');
    await expect(rows.nth(0)).toContainText(`${await when(base)} · 4.0 KB`);
    await expect(rows.nth(1)).toContainText('newer.aseprite');
    await expect(rows.nth(1)).toContainText('Pixel');
    await expect(rows.nth(1)).toContainText(`${await when(base - 60_000)} · 512 B`);
    await expect(rows.nth(2)).toContainText('older.aseprite');
    await expect(rows.nth(2)).toContainText('Pixel');
    await expect(rows.nth(2)).toContainText(`${await when(base - 3_600_000)} · 2.0 KB`);
    await expect(dialog).not.toContainText('stale.aseprite');
  }
});

test('a discarded snapshot is gone for good; discarding the last one closes the dialog', async ({
  page,
}) => {
  await gotoStart(page);
  const ts = Date.UTC(2026, 9, 1, 12, 0);
  await seedSnapshots(page, [
    { docId: 'first', ts, name: 'first.aseprite', byteLength: 10, mode: 'pixel' },
    { docId: 'second', ts: ts - 1000, name: 'second.aseprite', byteLength: 10, mode: 'pixel' },
  ]);
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const dialog = recoveryDialog(page);
  const rows = dialog.getByRole('listitem');
  await expect(rows).toHaveCount(2);
  await rows.filter({ hasText: 'first.aseprite' }).getByRole('button', { name: 'Discard' }).click();
  await expect(rows).toHaveCount(1);
  await expect(dialog).toBeVisible();

  // After a reload only the kept snapshot is offered.
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(rows).toHaveCount(1);
  await expect(rows.nth(0)).toContainText('second.aseprite');
  await rows.nth(0).getByRole('button', { name: 'Discard' }).click();
  await expect(dialog).toHaveCount(0);
  expect(await metaRows(page)).toEqual([]);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await settle(page);
  await expect(dialog).toHaveCount(0);
});

test('discarding the last snapshot closes the dialog', async ({ page }) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const dialog = recoveryDialog(page);
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Discard' }).click();
  await expect(dialog).toHaveCount(0);
});

test('a discarded document is not offered again, on New or after a reload', async ({ page }) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await page.clock.fastForward(31_000);
  await expect.poll(async () => (await metaRows(page)).length).toBe(1);

  // Closing the tab with Don't Save drops the work (unsaved-changes-prompt
  // AC-05); a New document of the other mode opens afterwards, in the same
  // page load (`newDocument`: `createDocument` would load the page).
  await menu(page, 'File', 'Close');
  await page.getByRole('button', { name: "Don't Save" }).click();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
  await newDocument(page, 'image', { width: 16, height: 16 });
  const recovery = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  // The probe is async; give it time to (not) open the dialog.
  await page.waitForTimeout(500);
  await expect(recovery).toHaveCount(0);

  // The snapshot is deleted in the background; a reload before the delete
  // is stored aborts it, so wait for it as for any IndexedDB write.
  await expect.poll(() => metaRows(page)).toEqual([]);
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.waitForTimeout(500);
  await expect(recovery).toHaveCount(0);
});

test('"Not now" keeps the dialog away for the rest of the page load, not after a reload', async ({
  page,
}) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const recovery = recoveryDialog(page);
  await expect(recovery).toBeVisible();
  await recovery.getByRole('button', { name: 'Not now' }).click();
  await expect(recovery).toHaveCount(0);

  await menu(page, 'File', 'New…');
  const newDialog = page.getByRole('dialog', { name: 'New document' });
  await newDialog.getByRole('button', { name: 'Create' }).click();
  await expect(newDialog).toBeHidden();
  await waitForEditor(page, 'pixel');
  await page.waitForTimeout(500);
  await expect(recovery).toHaveCount(0);

  // The snapshot stayed: the next page load offers it again.
  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect(recovery).toBeVisible();
  await expect(recovery.getByRole('listitem')).toHaveCount(1);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: a failed snapshot write is reported and editing continues`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    // Every snapshot write fails, as on a full disk.
    await page.evaluate(() => {
      const put = IDBObjectStore.prototype.put;
      IDBObjectStore.prototype.put = function (this: IDBObjectStore, ...args) {
        if (this.name === 'autosave_snapshots') {
          throw new DOMException('The quota has been exceeded.', 'QuotaExceededError');
        }
        return put.apply(this, args);
      } as typeof put;
    });
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    const undo = page.getByRole('button', { name: 'Undo', exact: true });
    await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.6]);
    await page.clock.fastForward(31_000);
    await expect(page.getByText(/^Autosave failed: .*quota/)).toBeVisible();

    // The editor still takes edits: a second stroke, then undo leaves one.
    await drag(page, canvasOf(page, mode), [0.2, 0.8], [0.8, 0.8]);
    await undo.click();
    await expect(undo).toBeEnabled();
    await undo.click();
    await expect(undo).toBeDisabled();
  });
}

test('the last Pixel colors start the next session’s new documents', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await setColor(page, 'fg', '#123456');
  await setColor(page, 'bg', '#abcdef');
  await expect.poll(() => storedPref(page, 'lastColor')).toBe('#123456ff');
  await expect.poll(() => storedPref(page, 'lastBgColor')).toBe('#abcdefff');

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect.poll(() => colorOf(page, 'fg')).toBe('#123456');
  await expect.poll(() => colorOf(page, 'bg')).toBe('#abcdef');
});

test.describe('without IndexedDB', () => {
  test.beforeEach(async ({ page }) => {
    await downloadSaves(page);
    await page.addInitScript(() => {
      Object.defineProperty(window, 'indexedDB', { value: undefined, configurable: true });
    });
  });

  test('both editors create, edit and save; storage is skipped silently', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));
    await page.clock.install();
    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode, { width: 32, height: 32 });
      if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
      await drag(page, canvasOf(page, mode), [0.2, 0.2], [0.8, 0.6]);
      await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
      await page.clock.fastForward(31_000);
      const [download] = await Promise.all([
        page.waitForEvent('download'),
        menu(page, 'File', mode === 'pixel' ? 'Save (download)' : 'Save'),
      ]);
      expect(download.suggestedFilename()).toMatch(mode === 'pixel' ? /\.aseprite$/ : /\.ora$/);
      await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);
      await expect(page.getByText(/failed/i)).toHaveCount(0);
    }
    expect(errors).toEqual([]);
  });
});

test.describe('installed web app', () => {
  test.use({ serviceWorkers: 'allow' });

  test('loads offline once cached and opens both editors', async ({ page, context }) => {
    await gotoStart(page);
    // The service worker installs (precaching the app shell and both wasm
    // cores) and takes control of the page.
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
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await createDocument(page, 'image', { width: 16, height: 16 });
  });

  test('serves a standalone web app manifest scoped to the app', async ({ page }) => {
    await gotoStart(page);
    const href = await page.locator('link[rel="manifest"]').getAttribute('href');
    if (!href) throw new Error('no manifest link');
    const url = new URL(href, page.url());
    const res = await page.request.get(url.href);
    expect(res.ok()).toBe(true);
    const manifest = (await res.json()) as {
      name?: string;
      display?: string;
      start_url?: string;
      scope?: string;
    };
    expect(manifest.name).toBe('Pincel');
    expect(manifest.display).toBe('standalone');
    expect(new URL(manifest.start_url ?? '', url).pathname).toBe('/app/');
    expect(new URL(manifest.scope ?? '', url).pathname).toBe('/app/');
  });
});
