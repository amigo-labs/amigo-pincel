import { basename } from 'node:path';
import { expect, test } from '@playwright/test';
import { bundleWasm, isDevWasm } from '../e2e-perf/global-setup';

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

// Sizes measured 2026-11-12 (wasm-pack without wasm-opt): since both
// modules carry the agent tools, the release Image module (6.0 MB) passed
// the old 5 MiB line and `pnpm perf` refused every release bundle.
test('the perf setup tells a release bundle from a dev build by size', () => {
  expect(isDevWasm(5_003_400)).toBe(false); // pincel-wasm, release
  expect(isDevWasm(6_026_114)).toBe(false); // pincel-image-wasm, release
  expect(isDevWasm(19_742_983)).toBe(true); // pincel-wasm, dev
});
