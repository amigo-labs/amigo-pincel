import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// The shipped wasm must load. Ubuntu's apt binaryen (108) rebinds
// wasm-bindgen's exported `__wbindgen_externrefs` table to the funcref table;
// v1.6.0's web bundle and npm package failed with `Table.grow()` that way.
// Static checks of the build scripts — no browser, no app.

const root = fileURLToPath(new URL('../../', import.meta.url));
const read = (path: string): string => readFileSync(`${root}${path}`, 'utf8');
/** The file without its comment lines. */
const code = (text: string): string =>
  text
    .split('\n')
    .filter((line) => !line.trim().startsWith('#'))
    .join('\n');

test('the release workflow runs the binaryen release the Cloudflare build pins, never apt binaryen', () => {
  const release = code(read('.github/workflows/release.yml'));
  expect(release).not.toMatch(/apt-get install[^\n]*binaryen/);
  const pinned = /BINARYEN_VERSION=(\d+)/.exec(read('scripts/cf-build.sh'))?.[1];
  expect(pinned).toBeTruthy();
  const versions = [...release.matchAll(/BINARYEN_VERSION: '(\d+)'/g)].map((m) => m[1]);
  // The web, desktop and npm jobs each fetch it before their wasm-opt run.
  expect(versions).toEqual([pinned, pinned, pinned]);
  const steps = release.split(/\n\s+- name: /);
  const install = steps.findIndex((s) => s.startsWith('install binaryen'));
  const optimize = steps.findIndex((s) => s.startsWith('optimize wasm'));
  expect(install).toBeGreaterThan(-1);
  expect(optimize).toBeGreaterThan(install);
});
