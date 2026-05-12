# Status

_Last updated: 2026-05-12_

**Branch:** `claude/continue-status-md-ot5GR` · PR [#27](https://github.com/amigo-labs/amigo-pincel/pull/27) (draft) · M8.7a landed (Tileset Panel sidebar).

## Next task

**M8.7b** — Per-tile thumbnails in the Tileset Panel. Adds a new wasm
method `tilePixels(tilesetId, tileId) -> Uint8Array` (or a single
tileset-image buffer) that the panel paints into small Canvas2D tiles.
M-sized.

## Milestone status

| ID | Status | Scope |
|----|--------|-------|
| M1 | ✅ | `pincel-core` skeleton — Sprite / Layer / Frame / Cel / Palette types, SpriteBuilder |
| M2 | ✅ | Commands + linear undo bus (cap 100) — SetPixel, AddLayer, AddFrame |
| M3 | ✅ | `compose()` for image layers — RGBA, source-over, viewport+zoom |
| M4 | ✅ | `aseprite-loader` read adapter (RGBA only) |
| M5 | ✅ | `aseprite-writer` crate + write adapter (header / layer / palette / tags / cel) |
| M6 | ✅ | `pincel-wasm` + minimal Svelte UI (open / Pencil / save). M6.7 human cross-validation deferred. |
| M7 | ✅ | Tools — Eraser, Eyedropper, Line, Rect, Rect-Fill, Ellipse, Ellipse-Fill, Bucket, Move (pan + selection-content drag), Selection (Rect) + marching-ants overlay |
| M8.1–M8.6 | ✅ | Tilemap pipeline below the UI — core accessors, compose path (rotate→flip_x→flip_y), AddTileset / PlaceTile commands, aseprite_read + aseprite_write tileset+tilemap, wasm tileset surface |
| **M8.7** | 🔄 | UI: Tileset Panel + Tilemap Stamp tool + Tileset Editor sub-mode — split into M8.7a–d below |
| M9 | ⬜ | Slice support |
| M10 | ⬜ | PWA polish |
| M11 | ⬜ | Tauri build |
| M12 | ⬜ | Performance pass |

### M8.7 sub-tasks

- [x] **M8.7a** — Tileset Panel + "Add Tileset" form. No new wasm.
- [ ] **M8.7b** — Per-tile thumbnails. New wasm `tilePixels(tilesetId, tileId) -> Uint8Array` (or a single tileset-image buffer) painted into small Canvas2D tiles.
- [ ] **M8.7c** — Active-layer concept + "Add Tilemap Layer" wasm method + Tilemap Stamp tool (click-to-place on the active tilemap layer with a grid overlay during hover).
- [ ] **M8.7d** — Tileset Editor sub-mode + `paintTilePixel(tilesetId, tileId, x, y, color)` wasm. Routes existing image tools through the bus targeting `Tileset::tiles[tile_id].pixels`.

Auto-tile mode (paint-on-tilemap = auto reuse / create tiles) stays Phase 2 per spec §5.3 / §13.2.

## Recent work

- **2026-05-12 — M8.7a (this branch).** `ui/src/lib/components/TilesetPanel.svelte` mounted as right-side sidebar in `App.svelte`. Reads via the M8.6 wasm surface; writes via `addTileset(name, tile_w, tile_h)`. Inline validation + wasm error surfacing. Reactivity over opaque wasm getters via a `tilesetRev` `$state` counter bumped on `newDoc` / `openFile` / `undo` / `redo` / `onChange`. Tile-size number inputs use `step="1"` + `inputmode="numeric"`. PR-27 Copilot review addressed in commit `4884f7a`.
- **2026-05-11 — M8.1–M8.6.** End-to-end tilemap pipeline below the UI. See commits `9c0a6cc` (wasm), `8f9f3ed` + `e4549ea` (write path), `c05a31b` + `d58197e` (read path), and the M8.1–M8.3 commits in `git log` for per-step detail.
- **Earlier 2026-05 — M7.1–M7.8c.** Tools expansion, end with the Selection (Rect) tool + marching-ants overlay. Move tool ships both viewport pan (M7.7a) and selection-content drag (M7.7b).
- **Earlier 2026-05 — M6.** wasm crate + Svelte 5 + Vite + open / paint / save MVP.
- **Earlier 2026-05 — M1–M5.** Core types, command bus, compose, codec read+write.

Full prose history for each milestone lives in `git log` (the prior 1647-line `STATUS.md` is preserved in the commits up to and including `4884f7a`).

## Build status

All gates green on this branch:

- `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm install`, `pnpm check`, `pnpm lint`, `pnpm build`, `pnpm wasm:build`

`cargo fmt --all --check` has pre-existing drift in `crates/pincel-wasm/src/lib.rs` — to clean up in a standalone fmt-only commit (out of scope for the current slice per CLAUDE.md §9).

## Website (Cloudflare Workers Builds)

Marketing site (`website/`) deploys via Cloudflare Workers Builds Git integration (project `amigo-pincel`). `wrangler.toml` at repo root runs `pnpm install --frozen-lockfile && pnpm build` inside `website/` and serves `website/build` with `not_found_handling = "404-page"`. SEO URLs derive from `website/src/lib/config.ts::siteUrl`.

Per-page compressed payload ~10 KB HTML + ~57 KB shared `_app` (well under the 200 KB spec §6.3 budget).

Human action still needed:

1. Confirm the Cloudflare `amigo-pincel` project's Workers Builds settings don't override `wrangler.toml` (or set: build command from `wrangler.toml`, root directory `/`).
2. Decide the production domain (spec §14 Q1) and update `website/src/lib/config.ts::siteUrl` if it differs from `https://pincel.app`.

## Open questions (still actionable)

- **M6.7** — Human-driven cross-validation: open hand-crafted fixture in Pincel, paint, save, reopen in upstream Aseprite. Programmatic round-trip is pinned by `crates/pincel-wasm/tests/paint_save_open_roundtrip.rs`.
- **Slice round-trip carrier** — `aseprite_read` drops slice chunks today. Spec §7.1 wants opaque chunk preservation; needs an `unknown_chunks: Vec<RawChunk>` carrier on `Sprite` / `Layer` / `Cel`. Land alongside M9.
- **Stable LayerIds** — IDs assigned by source-file position today. Stable for read-only sessions but conflicts with spec's "stable id" promise once a reorder command exists. Revisit when reorder lands.
- **Mid-list AddFrame** — Append-only today. Mid-list insertion needs a `FrameIndex` remap on cel map / `Tag` / `Slice` refs. Defer until a tool needs it.
- **Indexed-mode painting** — `SetPixel` is RGBA-only. Indexed needs either a payload enum or a separate command. Land when indexed `compose()` lands.
- **Auto-create cels on empty targets** — `MissingCel` today. Decide when Pencil hits the case in practice.
- **`compose()` allocation** — Allocates output buffer per call. Spec §4.1 wants pre-allocated scratch. Fold into M12.
- **`dirty_hint` not wired** — Accepted but ignored. Needs dirty-rect tracking (spec §4.3). Defer to M12.
- **`pincel-wasm` error type** — Returns `Result<_, String>` for host-target testability. Migrate to `JsError` once `wasm-pack test --node` lands.
- **`Document::undo` / `redo` dirty events** — Emit full-canvas `dirty-canvas` because commands don't carry their own dirty region. Per-command dirty-rect is M12.
- **`Document::new` 0-frame question** — `aseprite-writer` happily emits a 0-frame file that `aseprite-loader` then refuses to parse. Decide whether to enforce ≥1 frame in `SpriteBuilder::build` or leave as a "valid Pincel, invalid Aseprite" affordance.
- **Move/zoom ergonomics** — M7.7 lacks wheel/pinch zoom, auto-fit on open, and cursor-anchored zoom. Cosmetic; not blocking.
- **Selection in undo stack** — `selection` lives on `Sprite` directly, not through a command. Aseprite tracks selection in undo; Pincel does not. Revisit if "select → drag → undo" UX needs the marquee back.
- **`pincel-wasm` link order** — `link:` protocol needs `crates/pincel-wasm/pkg/` to exist before `pnpm install`. CI / contributor docs should encode the order.
- **`wasm-opt` dev profile disabled** — `pincel-wasm/Cargo.toml` `dev` profile disables `wasm-opt` because the bundled downloader fails in the dev env. `release` profile keeps it on. Pin a system `wasm-opt` and point `wasm-pack` at it via `WASM_OPT_PATH` in CI when the deploy story lands.

## Deferred Aseprite chunks

Beyond CLAUDE.md M5 scope but in spec §8.3: Color Profile (`0x2007`, sRGB), Old Palette (`0x0004`), External Files (`0x2008`), User Data (`0x2020`), Slice (`0x2022`), Tileset (`0x2023` — done in M8.5). Land alongside the milestones that need them (M8 tilemaps ✅, M9 slices).
