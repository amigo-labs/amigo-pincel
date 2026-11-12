import { existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * A dev-profile (`opt-level = 0`) wasm module is about 20 MB, a release build
 * 5–6 MB (2026-11-12, both modules with the agent tools; wasm-pack without
 * wasm-opt). Neither carries a marker of its profile (no DWARF sections in
 * either), so the size tells them apart. The 4K scenarios on a dev build run
 * into the 600 s test timeout instead of measuring anything.
 */
const DEV_WASM_BYTES = 12 * 1024 * 1024;

/** Whether a wasm module of `bytes` is a dev-profile build. */
export function isDevWasm(bytes: number): boolean {
  return bytes > DEV_WASM_BYTES;
}

/** The built bundle's asset folder (`vite build`, `base: '/'`). */
const ASSETS = fileURLToPath(new URL('../dist/assets/', import.meta.url));

/** Paths of the wasm modules in the built bundle; empty without a build. */
export function bundleWasm(): string[] {
  return existsSync(ASSETS)
    ? readdirSync(ASSETS)
        .filter((f) => f.endsWith('.wasm'))
        .map((f) => join(ASSETS, f))
    : [];
}

/**
 * Checks that the bundle under test carries the release wasm, then starts the
 * run with an empty results file.
 */
export default function globalSetup(): void {
  const wasm = bundleWasm();
  if (wasm.length === 0) {
    throw new Error('pnpm perf measures the built bundle: run `pnpm build` first.');
  }
  const dev = wasm.filter((f) => isDevWasm(statSync(f).size)).map((f) => basename(f));
  if (dev.length > 0) {
    throw new Error(
      `pnpm perf needs the release wasm, but dist/ holds a dev build (${dev.join(', ')}): ` +
        'run `pnpm wasm:build:release && pnpm build` first.',
    );
  }
  rmSync(fileURLToPath(new URL('./results/browser.json', import.meta.url)), { force: true });
}
