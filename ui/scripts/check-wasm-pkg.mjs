// preinstall guard: the UI declares `pincel-wasm: link:../crates/pincel-wasm/pkg`,
// so `pnpm install` before `pnpm wasm:build` fails with a cryptic link error.
// Fail early with a clear message instead.
//
// This lives in a script file on purpose: an inline `node -e "…"` version
// once contained backticks in its message, which the lifecycle shell
// expanded as command substitution — recursively re-running `pnpm install`
// in an infinite wasm-pack loop.
import { existsSync } from 'node:fs';

if (!existsSync(new URL('../../crates/pincel-wasm/pkg/package.json', import.meta.url))) {
  console.error(
    '\npincel-ui: crates/pincel-wasm/pkg is missing.\n' +
      'Run "pnpm wasm:build" before "pnpm install" — the UI links the generated\n' +
      'wasm package (needs wasm-pack and the wasm32-unknown-unknown target).\n',
  );
  process.exit(1);
}
