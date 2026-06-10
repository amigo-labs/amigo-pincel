# Pincel

Pincel is a pixel-art editor for game asset creation, aiming at the feature
level of [Aseprite](https://www.aseprite.org/). One Rust core and one Svelte
UI ship two ways: as an installable PWA (WebAssembly) and as a native
desktop app (Tauri 2). It reads and writes the `.aseprite` file format,
including tilemaps, tilesets, slices, and animation tags.

- **What to build** lives in the spec: [`docs/specs/pincel.md`](docs/specs/pincel.md)
- **How to build it** lives in the working agreement: [`CLAUDE.md`](CLAUDE.md)
- **Current state / next task**: [`STATUS.md`](STATUS.md)

## Repository layout

```
crates/pincel-core/      Pure logic: document model, commands + undo,
                         compose(), aseprite codec. No I/O, no platform deps.
crates/aseprite-writer/  Standalone .aseprite encoder (MIT OR Apache-2.0),
                         independent of Pincel types.
crates/pincel-wasm/      wasm-bindgen bindings (cdylib); built into pkg/.
ui/                      Svelte 5 + Vite frontend (PWA + Tauri webview).
src-tauri/               Native desktop shell (Tauri 2).
website/                 Marketing site (deployed via Cloudflare Workers).
docs/specs/              Design specifications.
```

## Prerequisites

- Rust (stable) with the `wasm32-unknown-unknown` target:
  `rustup target add wasm32-unknown-unknown`
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/) (e.g. `cargo install wasm-pack`)
- Node 22 + [`pnpm`](https://pnpm.io/) 10
- For the native shell only: the [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/)
  (on Linux: WebKitGTK / GTK3 dev libraries)

## Clone → running

Build order matters: `ui/package.json` links `pincel-wasm` from
`crates/pincel-wasm/pkg/` (a generated, gitignored directory), so the wasm
package must exist **before** `pnpm install`.

```bash
git clone https://github.com/amigo-labs/amigo-pincel
cd amigo-pincel

# 1. Rust core — check the library crates build and pass tests
cargo test -p pincel-core -p aseprite-writer -p pincel-wasm

# 2. Build the wasm package (creates crates/pincel-wasm/pkg/)
cd ui
pnpm wasm:build

# 3. Install UI dependencies and start the dev server
pnpm install
pnpm dev          # → http://localhost:5173
```

For the native app, after the steps above: `pnpm tauri:dev` (from `ui/`).

## Common commands

| Command | Where | What |
|---------|-------|------|
| `cargo check` / `cargo test` | repo root | build / test all Rust crates |
| `cargo clippy --workspace -- -D warnings` | repo root | lint (CI-enforced) |
| `cargo fmt` | repo root | format |
| `pnpm wasm:build` | `ui/` | dev wasm build into `crates/pincel-wasm/pkg/` |
| `pnpm dev` | `ui/` | Vite dev server |
| `pnpm lint` / `pnpm check` / `pnpm build` | `ui/` | ESLint / svelte-check / production bundle |
| `pnpm tauri:dev` / `pnpm tauri:build` | `ui/` | native dev / release binary |

CI (`.github/workflows/ci.yml`) runs the Rust gates, a Tauri check, and the
UI lint/check/build on every push and pull request.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.

"Aseprite" is a trademark of Igara Studio S.A. Pincel is not affiliated with
or endorsed by Igara Studio; it independently implements the publicly
documented `.aseprite` file format.
