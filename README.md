# Pincel

A pixel-art editor for game asset creation, at the feature level of
Aseprite. Pincel ships as one Svelte UI and one Rust core, packaged as
a Progressive Web App **and** a native desktop app via Tauri.

> **Status:** Phase 1, pre-1.0. See [`STATUS.md`](STATUS.md) for current
> milestone state.

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

## Build from source

Toolchain:

- Rust 1.85+ (workspace pinned via `rust-version` in
  [`Cargo.toml`](Cargo.toml))
- Node.js 20+, pnpm 9+
- `wasm-pack` (`cargo install wasm-pack`)
- For the native build: Tauri prerequisites
  ([Tauri docs](https://tauri.app/start/prerequisites/))

```bash
git clone https://github.com/amigo-labs/amigo-pincel
cd amigo-pincel

# build the wasm bridge once (required before pnpm install)
pnpm -C ui wasm:build:release

# install UI deps and run dev server
pnpm -C ui install
pnpm -C ui dev          # http://localhost:5173

# or run the native build
pnpm -C ui tauri:dev
```

Full developer reference, including the per-crate conventions,
session-management rules, and pre-commit gate, lives in
[`CLAUDE.md`](CLAUDE.md). The design spec lives in
[`docs/specs/pincel.md`](docs/specs/pincel.md).

## Workspace layout

```
crates/
  pincel-core/       pure Rust: document model, commands, compose, codec
  aseprite-writer/   standalone .aseprite v1.3 writer (publishable)
  pincel-wasm/       wasm-bindgen layer, builds to @amigo-labs/pincel

ui/                  Svelte 5 + Vite frontend (PWA + Tauri shell)
src-tauri/           native desktop shell (Tauri 2)
website/             marketing site (SvelteKit + Cloudflare Workers)

docs/specs/          living design specs
STATUS.md            current session state, next task, milestone status
CLAUDE.md            implementation conventions
```

## License

Dual-licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

Contributions intentionally submitted for inclusion in this work
shall be dual-licensed as above, without any additional terms or
conditions.

## Trademark

"Aseprite" is a trademark of [Igara Studio S.A.][igara] This project
is **not** affiliated with, endorsed by, or sponsored by Igara Studio.
Pincel implements the publicly-documented `.aseprite` file format for
interoperability.

[igara]: https://www.aseprite.org/
