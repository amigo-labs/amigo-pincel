import { defineConfig } from '@playwright/test';

// SIMD vs scalar equivalence (docs/specs/performance-build-simd.md AC-02),
// run by `pnpm simd:check` after `pnpm wasm:build:release` and
// `pnpm wasm:build:scalar` — not by `pnpm test:e2e`, which has no scalar
// build to compare with. Node only: no browser, no server.
export default defineConfig({
  testDir: 'e2e-simd',
  testMatch: /.*\.simd\.ts$/,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: 'list',
  timeout: 300_000,
});
