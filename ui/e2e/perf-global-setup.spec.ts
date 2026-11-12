import { basename } from 'node:path';
import { expect, test } from '@playwright/test';
import { bundleWasm } from '../e2e-perf/global-setup';

// `pnpm perf` refuses to run without the built bundle's wasm. The e2e suite
// runs against a built bundle, so the setup must find its modules there: a
// stale path (the editor moved from /app/ to the root, pincel.md §15
// 2026-11-11) made every `pnpm perf` and the nightly perf workflow stop at
// "run `pnpm build` first".
test('the perf setup finds the wasm modules of the built bundle', () => {
  const names = bundleWasm().map((p) => basename(p));
  expect(names.some((n) => n.startsWith('pincel_wasm_bg'))).toBe(true);
  expect(names.some((n) => n.startsWith('pincel_image_wasm_bg'))).toBe(true);
});
