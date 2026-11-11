import { expect, test } from '@playwright/test';
import { gotoStart } from './helpers';

// IndexedDB schema upgrades (ui/src/lib/idb/db.ts). A database an earlier
// release left behind must gain every store the current schema has: the
// font stores of system-fonts arrived at version 3 and the OCR models of
// ocr-dialog after them, so a profile already at version 3 must still get
// `ocr_models` when the app opens it.

test('a version 3 database from the system-fonts release gains the OCR model store', async ({
  page,
}) => {
  // A static file of the same origin, so the database is seeded before the
  // app opens it.
  await page.goto('favicon.svg');
  await page.evaluate(
    () =>
      new Promise<void>((resolve, reject) => {
        const req = indexedDB.open('pincel', 3);
        req.onupgradeneeded = () => {
          const db = req.result;
          db.createObjectStore('prefs', { keyPath: 'key' });
          db.createObjectStore('recent_files', { keyPath: 'id' }).createIndex(
            'by_openedAt',
            'openedAt',
          );
          db.createObjectStore('autosave_snapshots', { keyPath: ['docId', 'ts'] });
          db.createObjectStore('autosave_meta', { keyPath: ['docId', 'ts'] });
          db.createObjectStore('font_faces', { keyPath: 'key' });
          db.createObjectStore('font_files', { keyPath: 'id' });
        };
        req.onsuccess = () => {
          req.result.close();
          resolve();
        };
        req.onerror = () => reject(req.error);
      }),
  );

  await gotoStart(page);

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          new Promise<string[]>((resolve, reject) => {
            const req = indexedDB.open('pincel');
            req.onsuccess = () => {
              const names = Array.from(req.result.objectStoreNames);
              req.result.close();
              resolve(names);
            };
            req.onerror = () => reject(req.error);
          }),
      ),
    )
    .toEqual(
      expect.arrayContaining([
        'prefs',
        'recent_files',
        'autosave_snapshots',
        'autosave_meta',
        'font_faces',
        'font_files',
        'ocr_models',
      ]),
    );
});
