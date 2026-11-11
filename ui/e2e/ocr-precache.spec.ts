import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';
import { serveModels } from './ocr-helpers';

// The PWA never downloads an OCR model on its own (docs/specs/ocr-dialog.md
// AC-15): its precache manifest lists no model file, and loading the app
// and an Image document requests none. Reads the built bundle (`pnpm
// build` runs before the e2e suite).

const DIST = join(import.meta.dirname, '..', 'dist', 'app');

/** Every file path under `dir`, relative to it. */
function files(dir: string, prefix = ''): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? files(join(dir, e.name), `${prefix}${e.name}/`) : [`${prefix}${e.name}`],
  );
}

test('the precache manifest lists no model file and the bundle holds none (AC-15)', () => {
  const sw = readFileSync(join(DIST, 'sw.js'), 'utf8');
  // The manifest is inlined: it lists the app shell and the wasm cores …
  expect(sw).toContain('index.html');
  expect(sw).toMatch(/pincel_image_wasm_bg[^"']*\.wasm/);
  // … and no language model.
  expect(sw).not.toMatch(/traineddata|tessdata/);
  expect(files(DIST).filter((f) => /traineddata/.test(f))).toEqual([]);
});

test('loading the app and an Image document requests no model (AC-15)', async ({ page }) => {
  const requests = await serveModels(page);
  await createDocument(page, 'image', { width: 32, height: 32 });
  await page.waitForTimeout(500);
  expect(requests.urls).toEqual([]);
});
