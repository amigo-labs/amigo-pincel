#!/usr/bin/env node
// Builds both wasm cores without wasm SIMD, as release builds, into
// `target/scalar-pkg/<crate>` — the comparison build for the SIMD
// equivalence check (`pnpm simd:check`, docs/specs/performance-build-simd.md
// AC-02). `RUSTFLAGS` replaces the `+simd128` flag of `.cargo/config.toml`,
// and its own target directory keeps the shipped build's cache intact.

import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const env = {
  ...process.env,
  RUSTFLAGS: '-C target-feature=-simd128',
  CARGO_TARGET_DIR: join(root, 'target', 'scalar'),
};

for (const crate of ['pincel-wasm', 'pincel-image-wasm']) {
  const run = spawnSync(
    'wasm-pack',
    [
      'build',
      join(root, 'crates', crate),
      '--target',
      'web',
      '--release',
      '--out-dir',
      join(root, 'target', 'scalar-pkg', crate),
    ],
    { env, stdio: 'inherit' },
  );
  if (run.status !== 0) process.exit(run.status ?? 1);
}
