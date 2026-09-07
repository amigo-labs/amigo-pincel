# Pincel

Pincel is a pixel-art editor for game asset creation, aiming at the feature
level of [Aseprite](https://www.aseprite.org/) — and, since M16, a
paint.net-style image editor in the same app. One Svelte UI and two Rust
cores ship two ways: as an installable PWA (WebAssembly) and as a native
desktop app (Tauri 2).

- **Pixel mode** reads and writes the `.aseprite` file format, including
  tilemaps, tilesets, slices, and animation tags.
- **Image mode** (formerly the sister project `amigo-fineliner`) edits
  layered raster images with soft brushes, selections, effects and
  adjustments; it opens PNG / JPEG / WebP / BMP / GIF / TIFF and exports
  PNG / JPEG / WebP.

The mode follows the document: `.aseprite` opens in Pixel mode, images open
in Image mode, and the New dialog offers both.

> **Status:** Phase 1, pre-1.0. See [`STATUS.md`](STATUS.md) for current
> milestone state.

- **What to build** lives in the spec: [`docs/specs/pincel.md`](docs/specs/pincel.md)
- **How to build it** lives in the working agreement: [`CLAUDE.md`](CLAUDE.md)
- **Current state / next task**: [`STATUS.md`](STATUS.md)

## Why Pincel

- **`.aseprite` is the source of truth.** No proprietary format, no
  lock-in. Open files in Aseprite or any tool in that ecosystem
  (Godot import, LDtk, Phaser, custom engines).
- **Game-asset workflow first.** First-class tilemaps with tileset
  editing, slices with 9-patch + pivots, animation tags. Engine
  hot-reload via file watching (`amigo_assets`).
- **Document state in pure Rust memory.** The canvas is render-target
  only — never a data source. Avoids the architectural failure mode
  that forced Piskel into a multi-year rewrite (browser
  anti-fingerprinting silently corrupting canvas readbacks).
- **PWA-first, mobile-aware.** Modern `PointerEvent` end-to-end, pen
  pressure / tilt from day one, pinch-to-zoom standard. iPad with
  Apple Pencil is a reference target.
- **Embeddable.** `@amigo-labs/pincel` ships as an npm package with a
  stable public API from Phase 1 — embed Pincel into level editors,
  asset pipelines, or other tools.

## Try it

- **Web (PWA):** https://pincel.amigo-labs.dev/app
- **Desktop installers:** see the latest [GitHub release][releases]
  for Windows / macOS / Linux builds.

[releases]: https://github.com/amigo-labs/amigo-pincel/releases/latest

## Repository layout

```
crates/pincel-core/      Pixel mode core: document model, commands + undo,
                         compose(), aseprite codec. No I/O, no platform deps.
crates/fineliner-core/   Image mode core: layered RGBA documents, brush
                         engine, selections, transforms, image codecs.
crates/pincel-effects/   Effects + adjustments shared by both cores.
crates/aseprite-writer/  Standalone .aseprite encoder (MIT OR Apache-2.0),
                         independent of Pincel types.
crates/pincel-wasm/      wasm-bindgen bindings for pincel-core (cdylib).
crates/fineliner-wasm/   wasm-bindgen bindings for fineliner-core (cdylib).
ui/                      Svelte 5 + Vite frontend (PWA + Tauri webview):
                         src/App.svelte is the two-mode shell,
                         src/modes/pixel/ and src/modes/image/ the editors.
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
cargo test -p pincel-core -p pincel-effects -p aseprite-writer -p pincel-wasm -p fineliner-core -p fineliner-wasm

# 2. Build the wasm packages (creates crates/{pincel,fineliner}-wasm/pkg/)
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
| `cargo check` | repo root | build all Rust crates (needs GTK/WebKit libs for `pincel-tauri`) |
| `cargo test -p pincel-core -p pincel-effects -p aseprite-writer -p pincel-wasm -p fineliner-core -p fineliner-wasm` | repo root | test the library crates (what CI runs) |
| `cargo clippy -p pincel-core -p pincel-effects -p aseprite-writer -p pincel-wasm -p fineliner-core -p fineliner-wasm --all-targets -- -D warnings` | repo root | lint (CI-enforced; `--workspace` additionally needs the GTK/WebKit system libraries) |
| `cargo fmt` | repo root | format |
| `pnpm wasm:build` | `ui/` | dev wasm builds into `crates/pincel-wasm/pkg/` and `crates/fineliner-wasm/pkg/` |
| `pnpm dev` | `ui/` | Vite dev server |
| `pnpm lint` / `pnpm check` / `pnpm build` | `ui/` | ESLint / svelte-check / production bundle |
| `pnpm tauri:dev` / `pnpm tauri:build` | `ui/` | native dev / release binary |

CI (`.github/workflows/ci.yml`) runs the Rust gates, a Tauri check, and the
UI and website lint/check/build on every push and pull request.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.

"Aseprite" is a trademark of Igara Studio S.A. Pincel is not affiliated with
or endorsed by Igara Studio; it independently implements the publicly
documented `.aseprite` file format.
