import { existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * A dev-profile (`opt-level = 0`) Image-mode wasm is about 9 MiB, the release
 * build about 2.2 MiB; neither carries a marker of its profile, so the size
 * tells them apart. The 4K scenarios on a dev build run into the 600 s test
 * timeout instead of measuring anything.
 */
const DEV_WASM_BYTES = 5 * 1024 * 1024;

/**
 * Checks that the bundle under test carries the release wasm, then starts the
 * run with an empty results file.
 */
export default function globalSetup(): void {
  const assets = fileURLToPath(new URL('../dist/app/assets/', import.meta.url));
  const wasm = existsSync(assets) ? readdirSync(assets).filter((f) => f.endsWith('.wasm')) : [];
  if (wasm.length === 0) {
    throw new Error('pnpm perf measures the built bundle: run `pnpm build` first.');
  }
  const dev = wasm.filter((f) => statSync(join(assets, f)).size > DEV_WASM_BYTES);
  if (dev.length > 0) {
    throw new Error(
      `pnpm perf needs the release wasm, but dist/ holds a dev build (${dev.join(', ')}): ` +
        'run `pnpm wasm:build:release && pnpm build` first.',
    );
  }
  rmSync(fileURLToPath(new URL('./results/browser.json', import.meta.url)), { force: true });
}
