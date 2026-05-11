# Status

_Last updated: 2026-05-11_ (M7.8b: `pincel-wasm` selection surface — `setSelection` / `clearSelection` + `hasSelection` / `selectionX` / `selectionY` / `selectionWidth` / `selectionHeight` getters, plus a new `selection-changed` event. UI slice (M7.8c) follows.)

## Completed

### M1 — `pincel-core` skeleton ✅

- Cargo workspace at repo root (edition 2024, shared lints, `unsafe_code = deny`)
- Crate `crates/pincel-core` with no platform dependencies
- Document model per `docs/specs/pincel.md` §3:
  - `Sprite`, `Metadata`, `ColorMode`
  - `Layer` (Image / Tilemap / Group), `LayerId`, `BlendMode` (Aseprite numeric values)
  - `Frame`, `FrameIndex`
  - `Cel`, `CelData` (Image / Tilemap / Linked), `PixelBuffer`, `TileRef`
  - `Palette`, `PaletteEntry`, `Rgba`
  - `Tileset`, `TilesetId`, `TileImage`, `PathRef`
  - `Slice`, `SliceId`, `SliceKey`
  - `Tag`, `TagDirection`
  - `geometry::{Point, Rect}`
  - `error::DocumentError` (`InvalidDimensions`, `DuplicateId`, `UnknownId`)
- Fluent `SpriteBuilder` that validates canvas dimensions and id uniqueness
- 20 unit tests cover construction, defaults, and validation

### M2 — `pincel-core` commands + undo ✅

- `document::CelMap` — `(LayerId, FrameIndex) → Cel` storage (BTreeMap-backed),
  used by commands and `compose()`
- `command::Command` trait with `apply` / `revert` / default `merge`
- `command::AnyCommand` enum dispatching to concrete commands without dyn
- `command::Bus` — linear undo / redo stack with configurable cap
  (default 100, see spec §6.2). `execute` clears redo, attempts a merge with
  the stack top, then pushes; oldest entries drop when the cap is exceeded
- `command::CommandError` (`MissingCel`, `NotAnImageCel`, `PixelOutOfBounds`,
  `UnsupportedColorMode`, `DuplicateLayerId`, `FrameIndexOutOfRange`)
- Three commands per spec §6 / CLAUDE.md M2:
  - `SetPixel` — RGBA-only single-pixel write, sprite-coord input,
    captures the prior color for revert
  - `AddLayer::on_top` / `AddLayer::at` — z-order insertion, rejects
    duplicate ids
  - `AddFrame::append` — append-only for M2 (mid-list insertion would
    require remapping cel `FrameIndex` keys; deferred)
- 17 unit tests in `command` plus a `tests/command_bus.rs` integration
  suite with 6 cases

### M3 — `pincel-core` `compose()` (image layers) ✅

- New `render` module with the spec §4.1 contract:
  - `ComposeRequest` (frame, viewport, zoom, onion-skin, layer filter,
    overlays, dirty hint) — with a `ComposeRequest::full` helper
  - `ComposeResult` (RGBA8 pixels, width, height, generation)
  - `LayerFilter` (`Visible` / `All` / `Only(Vec<LayerId>)`)
  - `Overlays`, `OnionSkin` (defaults match spec §4.2: 76 ≈ 0.3 × 255 alpha)
- `render::compose(&Sprite, &CelMap, &ComposeRequest) → Result<…, RenderError>`:
  - RGBA color mode only (indexed / grayscale return
    `RenderError::UnsupportedColorMode`)
  - Visible image layers in z-order; tilemap and group layers raise
    `UnsupportedLayerKind`
  - Source-over (`Normal`) blend with per-cel and per-layer opacity;
    other `BlendMode` variants raise `UnsupportedBlendMode`
  - Cels clipped to the viewport intersection (negative positions OK)
  - `LayerFilter::All` / `Only` honored; invisible layers skipped under
    the default `Visible` filter
  - Integer zoom 1..=64 via row-replicate + memcpy nearest-neighbor
    upscale (output is `viewport.w * zoom` × `viewport.h * zoom`)
  - `generation` is `0` (purely-functional `compose`); the UI layer is
    expected to maintain its own monotonic counter
- `RenderError` covers: `UnsupportedColorMode`, `InvalidZoom`,
  `EmptyViewport`, `UnknownFrame`, `UnsupportedLayerKind`,
  `UnsupportedBlendMode`
- 23 new unit tests (60 unit total) plus a `tests/render_compose.rs`
  integration suite with 3 cases (two-layer offset, viewport+zoom,
  per-frame cel selection)

### M4 — `aseprite-loader` integration (read) ✅

- New workspace dep `aseprite-loader = "0.4.2"` (MIT OR Apache-2.0,
  matches spec §7.1) wired through `pincel-core::Cargo.toml`
- New `pincel-core::codec` module:
  - `read_aseprite(bytes) → Result<AsepriteReadOutput, CodecError>`
    where `AsepriteReadOutput { sprite, cels }` (cels live outside
    `Sprite` per the §3 model)
  - `CodecError` (`thiserror`) covers `Parse`, `UnsupportedColorMode`,
    `UnsupportedLayerKind`, `UnsupportedBlendMode`, `UnsupportedCelKind`,
    `LayerIndexOutOfRange`, `Image`, and a transparent `From<DocumentError>`
- Adapter rules (M4 scope):
  - `ColorDepth::Rgba` only — indexed / grayscale / unknown raise
    `UnsupportedColorMode`
  - Layer iteration uses the **low-level** `file.layers` so cel
    `layer_index` stays correct in the presence of any future tilemap
    layers; `LayerType::{Tilemap,Unknown}` raise
    `UnsupportedLayerKind`. Image and group survive.
  - Per-layer flags map: `VISIBLE`, `EDITABLE`. `LayerId` assigned by
    layer position (loader has no IDs of its own).
  - All 19 known `BlendMode` variants forwarded; `BlendMode::Unknown`
    raises `UnsupportedBlendMode`
  - `AnimationDirection::{Forward,Reverse,PingPong,PingPongReverse}`
    map directly; future variants fall back to `Forward` (M5 writer
    will preserve unknown tags as opaque chunks)
  - Cels: `CelContent::Image` decoded to RGBA8 via
    `AsepriteFile::load_image`; `CelContent::LinkedCel` preserved as
    `CelData::Linked` for lossless round-trip; `CompressedTilemap` and
    `Unknown` raise `UnsupportedCelKind`
  - Slice chunks dropped (M9 will round-trip)
- Originally tested via a hand-crafted byte-level fixture builder in
  `crates/pincel-core/tests/aseprite_read.rs`. Retired in M5 once the
  `aseprite-writer` adapter could drive integration tests; integration
  coverage now lives in `tests/aseprite_codec.rs` and routes through
  the writer (see the M5 entry below).
- 4 unit tests in `codec::aseprite_read` (empty input, color-mode
  rejection, blend-mode round trip, tag-direction mapping) survive in
  the read module; one was removed alongside the byte-level fixture
  builder.

### M5 — `aseprite-writer` crate (cel chunks) ✅

- New workspace member `crates/aseprite-writer`. Standalone, no
  `pincel-core` dependency. MIT/Apache dual license with the
  Aseprite trademark disclaimer in `README.md`.
- Owning data model that mirrors `aseprite-loader::binary::file::File`:
  `AseFile { header, layers, palette, tags, frames }`, `Header`,
  `Frame`, `LayerChunk`, `PaletteChunk`, `PaletteEntry`, `Tag`. Field
  names match the loader so reader output can be re-emitted without
  translation. Cel data deferred to the next M5 sub-task.
- Enum types with on-disk-matching discriminants: `BlendMode` (0..=18),
  `LayerType` (Normal/Group/Tilemap), `ColorDepth` (Rgba/Grayscale/
  Indexed), `AnimationDirection`, `LayerFlags`, `PaletteEntryFlags`.
- Little-endian byte writers in `bytes.rs` mirroring loader's scalar
  parsers (`write_byte`, `write_word`, `write_short`, `write_dword`,
  `write_string`, `write_zeros`).
- Top-level `pub fn write<W: Write>(file: &AseFile, out: &mut W) ->
  Result<(), WriteError>`. Stages the body in memory to fill in the
  header `file_size` and the per-frame size fields, then streams the
  full buffer to `out`.
- Implemented chunks (CLAUDE.md M5 list): Header (128 bytes),
  Layer (`0x2004`), Palette (`0x2019`), Tags (`0x2018`). Layout
  convention: layer/palette/tags chunks are emitted into frame 0,
  matching Aseprite's own output.
- `WriteError` (`thiserror`): `Io`, `FrameCountMismatch`, `TooMany`,
  `StringTooLong`, `MissingTilesetIndex`, `EmptyPalette`,
  `PaletteRangeOverflow`, `InvalidTagRange`.
- 19 unit tests + 8 round-trip integration tests (`tests/roundtrip.rs`)
  that build an `AseFile`, write it, parse with `aseprite-loader`, and
  assert structural equality. Coverage: empty RGBA sprite, three-layer
  blend-mode preservation, palette via `parse_raw_file` (RGBA palette
  is dropped by the high-level loader API), tags with all three
  directions, end-to-end (header, layer, palette, tags, multiple
  frames) including a `header.file_size == bytes.len()` check, and the
  cel-chunk coverage below.

### M5 sub-task — Cel chunk (`0x2005`) + zlib ✅

- `flate2 = "1"` added to workspace deps (spec §8.3 explicitly authorizes
  it for cel pixel data) and to `aseprite-writer/Cargo.toml`.
- `CelChunk { layer_index, x, y, opacity, z_index, content }` exposed
  from `lib.rs`. `CelContent::Image { width, height, data }` (Cel Type
  2, Compressed Image) and `CelContent::Linked { frame_position }`
  (Cel Type 1) cover the chunk variants used today.
- `Frame { duration, cels }` — per-frame chunk vector. Cels are
  emitted into the frame they belong to (no duplication into frame 0).
- Wire-format layout matches the spec / `aseprite-loader` parser:
  WORD layer_index, SHORT x, SHORT y, BYTE opacity, WORD cel_type,
  SHORT z_index, 5 reserved bytes, then the variant body. Compressed
  Image bodies prepend WORD width / WORD height before zlib-compressed
  pixel bytes (`flate2::write::ZlibEncoder`, default compression).
  Linked Cel bodies are a single WORD frame_position.
- New `ColorDepth::bytes_per_pixel()` helper (4 / 2 / 1) feeds the cel
  size validation.
- `WriteError` gains `CelImageSizeMismatch`, `CelLayerIndexOutOfRange`,
  and `CelLinkedFrameOutOfRange` so callers catch obviously-wrong cels
  before bytes hit the wire.
- 4 new unit tests + 3 new round-trip integration tests:
  single image cel with pixel-content assertion (zlib-decompressed),
  linked cel pointing back at frame 0, multi-cel across two layers and
  two frames (with one slot left empty so the loader yields `None`).

### M5 — `pincel-core::codec::aseprite_write` adapter ✅

- `aseprite-writer` added as a workspace dep and a `pincel-core`
  dependency (path-resolved through the workspace member, no external
  fetch).
- New module `pincel-core::codec::aseprite_write` plus
  `pub fn write_aseprite<W: Write>(&Sprite, &CelMap, &mut W) ->
  Result<(), CodecError>`. Re-exported from `lib.rs` next to
  `read_aseprite`.
- Translation rules (M5 scope, mirrors M4 reader):
  - `ColorMode::Rgba` only — indexed / grayscale raise
    `UnsupportedColorMode`.
  - `LayerKind::Image` → `LayerType::Normal`, `LayerKind::Group` →
    `LayerType::Group`. `LayerKind::Tilemap` raises
    `UnsupportedLayerKind { kind: 2 }`.
  - Layer flags map back from `visible` / `editable` booleans; per-
    layer `opacity` and `blend_mode` are forwarded one-to-one. `parent`
    is converted into Aseprite's flat `child_level` field via an
    iterative parent-chain walk that detects cycles
    (`LayerCycle { id }`) and missing parents
    (`LayerParentNotFound { id }`).
  - `BlendMode` uses an explicit per-variant match (the enum is closed
    on both sides, so no fallback is needed).
  - Palette: empty palettes produce no chunk; non-empty palettes emit a
    `PaletteChunk { first_color: 0, entries }` carrying `(color, name)`
    pairs as `Aseprite` palette entries.
  - Tags: per-tag `from`/`to` u32 frame indices `try_into` u16; `repeat`
    forwards directly; `color` keeps RGB and drops alpha (the on-disk
    format only carries 3 bytes).
  - Cels: `CelMap` is bucketed by `FrameIndex` into per-frame chunk
    vectors. `CelData::Image` becomes `CelContent::Image`,
    `CelData::Linked(frame)` becomes `CelContent::Linked` after a
    bounds check against `Sprite::frames.len()`. `CelData::Tilemap`
    raises `UnsupportedCelKind { kind: 3 }`.
  - Cel position `(i32, i32)` and pixel-buffer dimensions `(u32, u32)`
    are pre-validated against the on-disk `i16`/`u16` slots; overflow
    raises a structured `CodecError::OutOfRange { what, value }`.
  - Cels referencing a `LayerId` outside `Sprite::layers` raise
    `CelLayerNotFound { id }`; cels with a `frame` past
    `Sprite::frames` raise `CelFrameNotFound { index }`. The writer's
    own post-validation surfaces transparently as
    `CodecError::Write(WriteError)`.
  - Tilesets and slices on the document are silently dropped, matching
    the reader (M9 will round-trip slices).
- `CodecError` gained: `OutOfRange { what, value }`,
  `LinkedFrameNotFound { index }`, `LayerParentNotFound { id }`,
  `LayerCycle { id }`, `CelLayerNotFound { id }`,
  `CelFrameNotFound { index }`, and a transparent
  `Write(#[from] aseprite_writer::WriteError)`.
- 14 new unit tests cover every error variant the adapter raises plus
  a write→read smoke test for the happy RGBA-image path.
- The hand-crafted byte-level fixture builder
  (`crates/pincel-core/tests/aseprite_read.rs`) is retired; the
  replacement
  `crates/pincel-core/tests/aseprite_codec.rs` builds Pincel
  documents and asserts the writer→reader round-trip preserves them
  (5 cases: single-layer pixel content, multi-layer blend mode +
  visibility + offset, group hierarchy, linked cel, tags). Tests that
  needed to read malformed bytes — bogus blend mode, out-of-range
  linked cel — are dropped because they duplicated unit-test coverage
  in `codec::aseprite_read` and the loader's own parse path.

### M6 — `pincel-wasm` skeleton (M6.1) ✅

- New workspace member `crates/pincel-wasm` (`cdylib + rlib`,
  edition 2024, workspace lints incl. `unsafe_code = "deny"`).
  `pincel-core` now also lives in `[workspace.dependencies]` so the
  wasm crate can pick it up via `workspace = true`.
- `wasm-bindgen = "0.2"` added to workspace deps. No
  `getrandom` / `console_error_panic_hook` yet — defer until a
  non-deterministic feature lands. `wasm-pack` itself is a build-time
  CLI, not a Cargo dep.
- New `pincel-wasm::Document` type (the only public class in the
  crate today). Owns a `pincel_core::Sprite` + `CelMap` pair.
  Boundary contract follows spec §9.3 / §17.5; this is the M6
  starter slice of the surface.
- Methods exposed (M6.1 scope):
  - `Document::new(width, height) -> Result<Document, String>` —
    `#[wasm_bindgen(constructor)]`. RGBA-only, seeds a single
    100 ms frame so the empty document round-trips through the
    Aseprite codec.
  - `Document::open_aseprite(bytes) -> Result<Document, String>` —
    `js_name = openAseprite`. Thin wrap around
    `pincel_core::read_aseprite`.
  - `Document::save_aseprite() -> Result<Box<[u8]>, String>` —
    `js_name = saveAseprite`. Thin wrap around
    `pincel_core::write_aseprite`; returns a freshly-allocated
    `Uint8Array` on the JS side.
  - Getters: `width`, `height`, `layerCount`, `frameCount`.
- Errors cross the boundary as `Result<_, String>` rather than
  `Result<_, JsError>`. `JsError::new` panics on non-`wasm32-unknown-
  unknown` targets ("cannot call wasm-bindgen imported functions on
  non-wasm targets"), and the host-target unit tests need the error
  paths to be reachable. wasm-bindgen converts `String` Errs into a
  thrown JS exception, so the JS surface is unchanged.
- 5 unit tests in `pincel-wasm`: constructor success, two
  zero-dimension rejections, save→open round-trip on a fresh
  document, and a garbage-bytes rejection.
- Copilot review on PR #8 picked up:
  - Replaced `Frame::default()` with `Frame::new(100)` so the
    documented "100 ms first frame" invariant lives at the call
    site rather than depending on `Frame::default` staying at
    100 ms.
  - Rewrote the `Document::new` error doc to describe the failure
    condition (zero width / height) instead of naming a
    `pincel_core` internal variant; the `Err(String)` body is
    surfaced from `Display`, not part of the JS contract.
  - Added "failed to build sprite" / "failed to open Aseprite" /
    "failed to save Aseprite" prefixes on every `map_err`, so a
    thrown JS exception identifies which step failed.

### M6 — `Document::compose` (M6.2) ✅

- New `pincel_core::compose` re-export plumbed through to JS as
  `Document.compose(frame, zoom)`.
- Surface kept narrow for the M6.2 slice: full-canvas viewport,
  default `LayerFilter::Visible`, no overlays / onion skin,
  no `dirty_hint`. Viewport / filter / overlay knobs land in a
  follow-up sub-task once the UI surfaces a need.
- Companion struct `ComposeFrame { width, height, pixels }` with
  three `#[wasm_bindgen(getter)]` accessors. The `pixels` getter
  returns a fresh `Box<[u8]>` (materialized as `Uint8Array` on the
  JS side); spec §9.3 calls for a zero-copy `Uint8ClampedArray`
  view of WASM memory, which lands once `js-sys` is wired up
  (M6 follow-up).
- `frame: u32` → `FrameIndex::new`, `zoom: u32` clamped by
  `pincel_core::compose` itself (`InvalidZoom` for `0` or
  `> MAX_ZOOM`). `RenderError` surfaces with a "failed to compose"
  prefix.
- 5 new unit tests (10 total): zero-layer transparent output at
  1× zoom, integer-zoom output dimensions at 4×, plus three error
  paths (unknown frame, zoom 0, zoom 65).

### M6 — `Document::applyTool` Pencil + bootstrap (M6.3) ✅

- `Document::new` now bootstraps a default image layer
  (`LayerId(0)`, `"Layer 1"`) and a transparent
  canvas-sized image cel at frame 0 so a freshly-created
  document is paintable without explicit layer / cel
  creation. `open_aseprite` is unchanged (it preserves
  whatever the file carries).
- New `bus: pincel_core::Bus` field on `Document` tracks
  command history. `Document::applyTool(tool_id, x, y,
  color)` routes a `SetPixel` through the bus when
  `tool_id == "pencil"`; unknown tools yield
  `Err("unknown tool: <id>")`. Errors from the underlying
  command (out-of-bounds pixel, missing cel, ...) surface
  with a `"failed to apply pencil"` prefix.
- `color` is packed as `0xRRGGBBAA` (red high byte, alpha
  low byte) — single positional arg keeps the JS surface
  ergonomic (`doc.applyTool('pencil', x, y, 0xff0000ff)`)
  and avoids `clippy::too_many_arguments`. The richer
  `{ button, mods, phase, brushSize }` options struct
  from spec §9.3 lands when the second tool ships in M7.
- The active layer / frame is implicit (layer 0, frame 0)
  for now. Active-target state lands when the UI surfaces
  layer / frame selection in M6.6.
- 4 new unit tests (14 total in `pincel-wasm`): pencil
  writes a pixel that surfaces through `compose`, two
  paints leave the bus at `undo_depth == 2` and undo
  restores the second pixel, unknown tool ids reject, and
  out-of-bounds coordinates reject.

### M6 — `Document::drainEvents` + undo / redo (M6.4) ✅

- New `pincel_wasm::events` module: `Event` (wasm-bindgen
  exported, uniform shape with a `kind` getter) and a private
  `EventQueue` ring buffer (`VecDeque`-backed, drop-oldest, cap
  `1024`). Today the only `kind` shipped is `"dirty-rect"`;
  future kinds (`layer-changed`, `palette-changed`,
  `undo-pushed`, …) append to the same struct without changing
  the JS wire shape.
- `Event` exposes seven getters: `kind` (string), `layer`,
  `frame` (`u32`), `x`, `y` (`i32`), `width`, `height` (`u32`).
  Two `kind` strings ship today: `"dirty-rect"` (a single cel
  region changed; all numeric fields are meaningful) and
  `"dirty-canvas"` (the whole canvas should be re-rendered;
  numeric fields are unspecified and consumers must not key
  off them).
- `Document` gained an `events: EventQueue` field plumbed
  through `Document::new` and `Document::open_aseprite`.
- `Document::applyTool` pencil now enqueues a 1×1
  `dirty-rect` event after a successful paint (post-
  `Bus::execute`, so failed paints leave the queue alone).
  Coordinates are sprite-space and `layer` / `frame` reflect
  the paint target. Per-stroke merging (one dirty rect per
  drag instead of per pixel) is deferred until the UI surfaces
  pointer-event sequences in M6.6.
- New JS-facing methods on `Document`:
  - `undo() -> bool` — returns `true` when a command was
    undone. Pushes a `dirty-canvas` event so the UI re-renders
    without per-command tracking. Per-command dirty rects land
    in M12.
  - `redo() -> Result<bool, String>` — same shape, surfaces
    the underlying command error with a `"failed to redo"`
    prefix.
  - `undoDepth` / `redoDepth` getters (`u32`) backed by
    `Bus::undo_depth` / `Bus::redo_depth`.
  - `drainEvents() -> Vec<Event>` — returns the queue contents
    in FIFO order and clears the buffer. Verified to compile
    against `wasm-bindgen` 0.2.121 with no extra deps.
- 6 new unit tests in `pincel-wasm` plus 5 events-module
  tests (27 wasm unit total): empty drain on fresh doc,
  pencil emits a 1×1 dirty rect, failed paint emits nothing,
  undo / redo round-trip emits `dirty-canvas` and tracks
  depth, undo / redo on an empty stack are no-ops.

### M6 — Svelte 5 + Vite + Tailwind 4 scaffold (M6.5) ✅

- New `ui/` workspace member managed by `pnpm` (no SvelteKit; plain
  Svelte 5 + Vite per spec §9.1). `node_modules/`, `dist/`,
  `.svelte-kit/`, and `pkg/` are already gitignored at repo root.
- Versions pinned in `ui/package.json` (caret ranges, resolved by
  `pnpm-lock.yaml`):
  - `svelte 5.55`, `@sveltejs/vite-plugin-svelte 5.1`
  - `vite 6.4`
  - `tailwindcss 4.3` + `@tailwindcss/vite 4.3` (no PostCSS config —
    Tailwind 4 ships its own pipeline through the Vite plugin)
  - `typescript 5.9` (strict, `noUncheckedIndexedAccess`,
    `exactOptionalPropertyTypes`, `verbatimModuleSyntax`,
    `noEmit`)
  - `eslint 9.39` flat config + `typescript-eslint 8.59` +
    `eslint-plugin-svelte 3.17` + `svelte-eslint-parser 1.6`
  - `svelte-check 4.4` for `pnpm check`
  - `prettier 3.8` + `prettier-plugin-svelte 3.5`
- npm scripts (`ui/package.json`):
  - `dev` / `build` / `preview` — Vite
  - `check` — `svelte-check`
  - `lint` — `eslint .`
  - `format` — `prettier --write .`
  - `wasm:build` — `wasm-pack build ../crates/pincel-wasm --target
    web --out-dir pkg`. Verified end-to-end in this session;
    produces `pincel_wasm.{js,d.ts}` plus the wasm binary in
    `crates/pincel-wasm/pkg/`. The actual `import` from `ui/` lands
    in M6.6.
- `crates/pincel-wasm/Cargo.toml` opts out of `wasm-opt` for the
  `dev` wasm-pack profile only
  (`[package.metadata.wasm-pack.profile.dev]`); release stays
  optimized so CI / deploy can pick up the size win. The local
  `pnpm wasm:build` script uses `--dev` for fast iteration in
  environments without `wasm-opt`; `pnpm wasm:build:release`
  triggers the optimized variant for release builds. (Open
  question below.)
- Empty canvas page (`ui/src/App.svelte`): a 64×64 `<canvas>` shown
  at 8× via CSS with `image-rendering: pixelated`, framed by a
  header / footer shell that previews the spec §9.2 layout. The
  effect runs a single Canvas2D `fillRect` to make the canvas
  visible — no wasm interaction yet (M6.6 wires `Document.compose`).
- `ui/src/main.ts` mounts `App` on `#app` via Svelte 5
  `mount()`. `app.css` declares Tailwind via the v4 `@import
  'tailwindcss'` directive plus an `@layer base` block that fills
  body / html / #app to full height and sets the dark default.
- `ui/eslint.config.js` is a flat config wiring `@eslint/js`,
  `typescript-eslint` (recommended), and
  `eslint-plugin-svelte` (recommended) with the Svelte parser
  delegating to `ts.parser`. `dist/`, `node_modules/`, `pkg/`, and
  `.svelte-kit/` are ignored.
- Verified: `pnpm install` (clean), `pnpm check` (0 errors / 0
  warnings across 381 files), `pnpm lint` (clean), `pnpm build`
  (8 KB CSS / 28 KB JS gzipped to 2.3 KB / 11 KB), `pnpm wasm:build`
  (publishes a `pkg/` directory). `cargo check --workspace`,
  `cargo test --workspace`, `cargo clippy --workspace --all-targets
  -- -D warnings`, and `cargo fmt --all --check` remain green.

### M6 — UI wired to `pincel-wasm` (M6.6) ✅

- `ui/package.json` gains a single runtime dep,
  `"pincel-wasm": "link:../crates/pincel-wasm/pkg"`. pnpm creates a
  symlink into `ui/node_modules/pincel-wasm/` pointing at the
  wasm-pack output, which keeps the import surface clean
  (`from 'pincel-wasm'`) and avoids relative-path noise in source
  files. The pkg directory is gitignored (M6.5 carry-over); a
  clean checkout runs `pnpm wasm:build` before `pnpm install` /
  `pnpm dev` / `pnpm build`. CI orchestration lands later.
- New `ui/src/lib/core/index.ts` adapter (per CLAUDE.md §5.4 —
  "UI never imports `pincel-wasm` directly"):
  - `loadCore()` — idempotent wasm initializer. Internally calls
    `__wbg_init({ module_or_path })` exactly once, caching the
    resulting `Promise<void>`. The `?url` import of
    `pincel_wasm_bg.wasm` is the canonical Vite asset-URL form;
    it survives the production bundler unchanged and dropped the
    inline `new URL(…, import.meta.url)` fallback the wasm-pack
    JS would otherwise rely on.
  - Re-exports `Document`, `type ComposeFrame`, `type PincelEvent`
    (`Event` is renamed at the boundary to avoid colliding with
    DOM `Event`).
- New `ui/src/lib/render/canvas2d.ts`:
  - `blitFrame(canvas, frame)` resizes the backing store to the
    frame dimensions and writes a single `putImageData` with a
    fresh `Uint8ClampedArray` view of the wasm-side RGBA8 bytes.
    The CSS dimensions stay fixed at 512×512 with
    `image-rendering: pixelated`, so a 64×64 sprite renders as an
    8× crisp blit and other canvas sizes scale to fit the same
    box. The WebGPU renderer per spec §9.2 is M12.
- Rewritten `ui/src/App.svelte` (single component, Svelte 5
  runes — `$state` for reactive fields, no stores yet):
  - Toolbar: **New** (64×64 blank doc), **Open…** (`<input
    type=file accept=".aseprite,.ase">` triggered by a hidden
    input + button), **Save** (`saveAseprite` → `Blob` →
    download-anchor click → `URL.revokeObjectURL`), **Undo**,
    **Redo**, color picker (`<input type=color>` → packed
    `0xRRGGBBAA` with alpha `0xff`).
  - Canvas: 512×512 CSS, pointer-capture-based pencil drag
    (`pointerdown` → `setPointerCapture` + paint, `pointermove`
    paints while painting flag is set, `pointerup` /
    `pointercancel` release). Sprite-space coordinates derived
    from `getBoundingClientRect()` + `canvas.width / rect.width`
    scaling, `Math.floor`-snapped. Out-of-bounds drags are
    swallowed — the wasm `applyTool` raises on
    `PixelOutOfBounds`, the UI doesn't surface it.
  - RAF loop drains `Document.drainEvents()` once per frame;
    any non-empty drain marks the canvas dirty, which triggers a
    full `compose(0, 1)` + `blitFrame`. The per-frame coalescing
    means a 60 fps pencil drag emits one paint per RAF tick
    regardless of how many `applyTool` calls landed in between.
    Per-event ring-buffer entries are freed after draining (the
    `compose` follow-up frees its own).
  - Status bar surfaces the active operation (`ready`, `opened
    foo.aseprite · 32×32`, `saved 1234 bytes`, …) plus
    `width×height` / `undo N / redo N`.
- Verified: `pnpm install`, `pnpm wasm:build` (1.4 MB dev wasm,
  367 kB gzipped), `pnpm check` (0 errors / 0 warnings across
  384 files), `pnpm lint`, `pnpm build` (47 KB JS / 9 KB CSS, the
  wasm asset is the dominant payload). `cargo check --workspace`
  and `cargo fmt --all --check` still green; no Rust source
  changed. End-to-end paint round-trip not yet exercised in a
  real browser — that's the M6.7 demo.

### M6 — paint → save → open round-trip integration test (M6.7 prep) ✅

- New integration test file
  `crates/pincel-wasm/tests/paint_save_open_roundtrip.rs`. Exercises
  the full wasm surface end-to-end without a browser, pinning the
  byte-level promise the M6.7 demo relies on. The remaining
  human-driven steps (run the dev server, paint visually, reopen
  the saved file in upstream Aseprite, capture screenshots) are
  unchanged and still required to mark M6.7 done.
- Two cases:
  - `paint_save_open_roundtrip_preserves_pixels`: paints three
    distinct colors at three corners on an 8×8 doc, calls
    `saveAseprite`, parses the bytes back via `openAseprite`,
    asserts `width` / `height` / layer + frame counts, then
    composes frame 0 at zoom 1× and asserts the painted pixels are
    present and unpainted ones stay fully transparent.
  - `paint_save_open_roundtrip_preserves_undo_target_state`:
    confirms the reopened doc starts with a fresh undo / redo
    stack (the file format does not carry one) and that a
    follow-up `apply_tool` paints on top of the persisted
    pixels — i.e. the reopened doc is a fully editable session,
    not a read-only view.
- Picked up trivially by `cargo test --workspace`; no extra
  dev-deps. Adds 2 to the pincel-wasm test count (27 unit + 2
  integration, 29 total).

### M7 — Eraser tool (M7.1) ✅

- `pincel-wasm::Document::apply_tool` now accepts `tool_id ==
  "eraser"` and routes it through the same `SetPixel` command as
  the Pencil, with the color hard-coded to `Rgba { 0, 0, 0, 0 }`
  (spec §5.2 — "Clears to transparent (RGBA) or transparent-index
  (Indexed)"). The `color` argument is documented as ignored for
  the eraser so the JS surface stays a single positional-arg
  signature for every Phase-1 image tool. Dirty-rect events,
  layer / frame targeting, and out-of-bounds handling are
  identical to Pencil; the only behavioral difference is the
  written pixel value.
- The error prefix in `applyTool` was generalized from
  `"failed to apply pencil"` to `"failed to apply {tool_id}"`
  so the JS-side `console.error` / status-bar message identifies
  the active tool. No tests asserted on the old prefix.
- 4 new unit tests (31 wasm unit total): eraser clears a
  previously painted pixel (and joins the bus → undo depth 2),
  eraser ignores the color arg, eraser emits a 1×1 dirty rect,
  eraser rejects out-of-bounds pixels.
- UI gains a Pencil / Eraser tool group in the toolbar
  (`role="group" aria-label="Active tool"`, `aria-pressed` mirrors
  selection, `.toolbar-btn-active` style class for the active
  button). `tool` is a Svelte 5 rune-backed `$state` of a `Tool`
  string union (initially `'pencil' | 'eraser'`; M7.2 widened it
  to include `'eyedropper'`) defaulting to `'pencil'`. `paintAt`
  forwards the current tool to `doc.applyTool` and the
  pointer-event pipeline is otherwise unchanged.

### M7 — Eyedropper tool (M7.2) ✅

- New `pincel-wasm::Document::pick_color(frame, x, y) → u32`
  (`js_name = pickColor`). Returns the packed non-premultiplied
  `0xRRGGBBAA` value at the requested sprite coordinate, sampled
  through `pincel_core::compose` with a 1×1 viewport at `(x, y)`,
  the default `Visible` layer filter, and no overlays. The 1×1
  viewport is the natural way to keep the existing M3 compose
  pipeline (with all its blend / layer-filter semantics) as the
  single source of truth — what the user sees is what they pick.
- Read-only by design: no command emitted, no event enqueued, no
  bus interaction. Out-of-canvas coordinates are not rejected; they
  fall outside every cel's intersection and yield transparent
  (`0x00000000`), matching the spec §4.1 "cels clipped to the
  viewport intersection" semantics. Errors propagate from
  `compose()` (unknown frame, unsupported color mode, …).
- 5 new unit tests (36 wasm unit total): pick of a painted pixel
  returns the painted color, pick of a transparent pixel returns
  `0`, out-of-canvas reads (negative and far-positive) return
  transparent, unknown-frame is rejected, pick does not disturb the
  command bus depth.
- UI gains an Eyedropper toolbar button alongside Pencil / Eraser
  (same `aria-pressed` + `.toolbar-btn-active` pattern). `Tool`
  union widened to `'pencil' | 'eraser' | 'eyedropper'`. `paintAt`
  dispatches on the active tool: eyedropper samples via
  `pickColor` and rebinds the foreground color input through a
  new `unpackColor(0xRRGGBBAA) → "#RRGGBB"` helper. Alpha is
  dropped at the UI surface for now — the color input has no
  alpha control yet. Drags keep sampling so the user can scrub for
  the pixel they want.

### M7 — Line tool (M7.3) ✅

- New `pincel_core::DrawLine` command (`crates/pincel-core/src/command/
  draw_line.rs`). Rasterizes a 1-pixel-wide Bresenham line between two
  sprite-space endpoints into the target image cel, records each
  modified pixel's prior RGBA for revert, and skips pixels outside the
  cel's pixel buffer silently. The shared `bresenham(x0, y0, x1, y1)
  → Vec<(i32, i32)>` helper is internal to the command for now; if
  outline-rectangle / ellipse tools want to share it, lift to
  `geometry` when M7.4 / M7.5 land. Errors mirror `SetPixel`
  (`MissingCel`, `NotAnImageCel`, `UnsupportedColorMode`). No
  `merge`: each line is its own command, one press-drag-release per
  undo step.
- `AnyCommand::DrawLine` variant + `From<DrawLine>` impl plumb the
  command through the bus. Re-exported from `pincel_core::lib.rs`
  alongside `SetPixel`.
- `pincel-wasm::Document::apply_line(x0, y0, x1, y1, color)`
  (`js_name = applyLine`). Packed `0xRRGGBBAA` color, targets the
  same active layer / frame as the pencil (lowest-z `LayerKind::Image`,
  frame 0). Emits a single `dirty-rect` event covering the line's
  axis-aligned bounding box (always positive `width` / `height`,
  endpoint order doesn't matter). A new `line_bbox` helper isolates
  the bounding-box math so it's testable on its own. `applyLine` is
  not exposed through `applyTool` because that surface is
  `(x, y, color)` only — a multi-coord tool needs its own entry
  point until the spec-§9.3 options struct lands.
- 13 new `pincel-core` unit tests (104 total): Bresenham helper
  covers horizontal / vertical / diagonal / reverse / single-pixel
  cases; `DrawLine` apply / revert / round-trip cases plus
  out-of-cel clipping, offset-cel local-coord translation,
  missing-cel error, no-merge.
- 7 new `pincel-wasm` unit tests (43 total): line writes pixels,
  joins the bus + undo restores transparency, dirty-rect bbox is
  correct in both endpoint orders, errors when no image layer
  exists, single-pixel line, `line_bbox` helper exhaustive.
- UI gains a Line toolbar button alongside Pencil / Eraser /
  Eyedropper. `Tool` union widened to include `'line'`. New press /
  drag / release pipeline on the canvas: `pointerdown` records the
  start, `pointermove` updates the live endpoint, `pointerup`
  commits via `doc.applyLine`. During the drag, `recompose` calls a
  new `paintLinePreview(canvas, x0, y0, x1, y1, color)` helper in
  `lib/render/canvas2d.ts` that overlays a Bresenham preview on top
  of the freshly-blitted composed frame using `ctx.fillRect(x, y,
  1, 1)` per rasterized pixel. The preview uses the same algorithm
  as the Rust `DrawLine`, so the in-flight preview is pixel-exact
  with what `applyLine` will commit.

### M7 — Ellipse tool (M7.5) ✅

- New `pincel_core::DrawEllipse` command (`crates/pincel-core/src/
  command/draw_ellipse.rs`). Rasterizes the ellipse inscribed in the
  axis-aligned bbox of two sprite-space corners — outline if `fill
  == false`, filled disk if `fill == true`. Uses Alois Zingl's
  integer midpoint algorithm ("A Rasterizing Algorithm for Drawing
  Curves") with i64 internal math. The outline plots each rim pixel
  through a `HashSet` dedupe so the four-quadrant emission and the
  tail loop's tip pixels don't double-record a `PriorPixel` and
  break revert. The fill collects per-row x extents (a Vec sized to
  the cel height, so out-of-cel rows allocate nothing) and emits
  contiguous horizontal spans. Degenerate (`a == 0` / `b == 0`)
  bboxes fall back to a single-axis line — the Zingl algorithm
  assumes both axes ≥ 1 and otherwise leaves the tip pixels
  unplotted. Pathological bboxes (axis span > 2^20) short-circuit
  to a no-op so the perimeter walk terminates within a frame and
  the algorithm's `a*b*b` products stay in i64. Errors mirror
  `DrawLine` / `DrawRectangle` (`MissingCel`, `NotAnImageCel`,
  `UnsupportedColorMode`). No `merge`: each ellipse is its own
  command, one press-drag-release per undo step. Constructor takes
  corners as `(i32, i32)` tuples to satisfy
  `clippy::too_many_arguments`.
- `AnyCommand::DrawEllipse` variant + `From<DrawEllipse>` impl plumb
  the command through the bus. Re-exported from `pincel_core::lib.rs`
  alongside `SetPixel` / `DrawLine` / `DrawRectangle`.
- `pincel-wasm::Document::apply_ellipse(x0, y0, x1, y1, color, fill)`
  (`js_name = applyEllipse`). Packed `0xRRGGBBAA` color, targets the
  same active layer / frame as the pencil (lowest-z `LayerKind::
  Image`, frame 0). Emits a single `dirty-rect` event covering the
  ellipse's bbox; reuses the `endpoint_bbox` helper introduced for
  Line / Rect.
- 17 new `pincel-core` unit tests (139 total): normalize, single-
  pixel, zero-width / zero-height degenerate lines, outline
  symmetry about the bbox center, outline touches the four axis
  extremes, fill includes the center, fill rows are contiguous,
  reversed endpoints equivalence, outline / fill revert, cel-bounds
  clipping, bbox-entirely-outside no-op, pathological-bbox short-
  circuit, offset cel local coords, missing-cel error, no-merge.
- UI gains **Ellipse** and **Ellipse Fill** toolbar buttons. The
  `Tool` union widens to include `'ellipse'` and `'ellipse-fill'`;
  `isDragShapeTool` extends accordingly so the same press / move /
  release pipeline drives Line, Rect, and Ellipse. The new
  `paintEllipsePreview(canvas, x0, y0, x1, y1, color, fill)` helper
  in `lib/render/canvas2d.ts` mirrors the Rust rasterizer's midpoint
  algorithm pixel-for-pixel; for fill it uses a `Float64Array` row-
  extent buffer keyed by the clipped canvas y range so far-out
  drags stay O(perimeter + visible rows).
- Shift-to-circle reuses `constrainedEndpoint()` — a square bbox
  inscribes a circle, so the existing `|dx| == |dy|` constraint
  serves Ellipse / Ellipse Fill the same way it serves Rect / Rect
  Fill. The Rust command takes raw corners; the modifier is applied
  in JS before the endpoint reaches `doc.applyEllipse`.

### M7 — Bucket tool (M7.6) ✅

- New `pincel_core::FillRegion` command (`crates/pincel-core/src/
  command/fill_region.rs`). Flood-fills the 4-connected region of
  pixels that match the seed pixel's exact RGBA (tolerance 0 per spec
  §5.2 MVP) and replaces them with the new color. Traversal is a
  queue-based BFS over the cel-local buffer with a `Vec<bool>` visited
  bitmap; worst-case work is `O(width * height)` and each pixel is
  enqueued at most once. Seed-color equals new-color is treated as a
  no-op (otherwise the queue would re-match every neighbor and the
  cel pixels never observably change anyway). Seeds whose sprite
  coordinates translate to a negative or out-of-buffer local position
  are also no-ops — natural drawing-tool clipping. Errors mirror the
  other image commands (`MissingCel`, `NotAnImageCel`,
  `UnsupportedColorMode`). No `merge`: each bucket-fill is its own
  command. The struct exposes `filled_count()` so tests can assert
  exact-size flood regions without re-walking the buffer.
- `AnyCommand::FillRegion` variant + `From<FillRegion>` impl plumb the
  command through the bus. Re-exported from `pincel_core::lib.rs`.
- `pincel-wasm::Document::apply_bucket(x, y, color)`
  (`js_name = applyBucket`). Packed `0xRRGGBBAA` color, targets the
  same active layer / frame as the pencil (lowest-z `LayerKind::
  Image`, frame 0). Emits a `dirty-canvas` event since the fill can
  affect any subset of the cel; the UI's RAF coalescer turns that
  into a single recompose. The wasm method joins the bus even on
  no-op fills (out-of-canvas seed, seed-color equals new-color) for
  undo symmetry with the other paint tools.
- 11 new `pincel-core` unit tests (150 total): blank-canvas full
  flood, color-boundary clipping (vertical line splits the canvas),
  4-connected-not-8 (a diagonal blue line confines a fill to a
  triangle), seed-equals-target no-op, revert restores every filled
  pixel, out-of-cel and negative-local seed no-ops, offset-cel local
  coords, missing-cel error, enclosed-region containment (a red
  frame stops the fill from leaking out), no-merge. 6 new
  `pincel-wasm` unit tests (64 total): blank-canvas fill, line-
  boundary stop, dirty-canvas event, bus integration, group-only
  layer error, out-of-canvas no-op-but-joins-bus.
- UI gains a **Bucket** toolbar button between Eyedropper and Line.
  The `Tool` union widens to include `'bucket'`. `onPointerDown`
  detects bucket and commits a single `doc.applyBucket(x, y, color)`
  via a new `commitBucket` helper without entering painting mode —
  drag-induced `pointermove`s do not fire the fill again (which
  would push redundant no-op fills onto the bus). The Bucket is not
  a drag-shape tool so the existing Line / Rect / Ellipse preview
  pipeline is untouched.

### M7 — `pincel-wasm` selection surface (M7.8b) ✅

- Builds on M7.8a's `Sprite::selection` model. Exposes the marquee
  rect through the JS boundary so the UI slice (M7.8c) can drive a
  Selection tool without reaching past the wasm adapter into
  `pincel-core`.
- New `Document` methods (`#[wasm_bindgen]` exported):
  - `setSelection(x, y, width, height)` — `js_name = setSelection`.
    Replaces the active marquee with the given sprite-space rect.
    An empty rect (`width == 0` or `height == 0`) clears, matching
    `Sprite::set_selection`. Always enqueues a `selection-changed`
    event (the RAF loop coalesces duplicates).
  - `clearSelection()` — `js_name = clearSelection`. Drops the
    selection; always enqueues `selection-changed` (zero bounds).
    No-op on the data model when nothing was selected, but still
    emits — symmetric with the rest of the "every write emits"
    paint API (e.g. `applyBucket` on a no-op fill).
  - Getters: `hasSelection`, `selectionX`, `selectionY`,
    `selectionWidth`, `selectionHeight`. When `hasSelection ==
    false`, the position / size getters return `0`; the UI is
    expected to pair the bounds with `hasSelection` to distinguish
    "selection at (0, 0)" from "no selection".
- New `EventKind::SelectionChanged` in `pincel-wasm::events`. JS
  kind string `"selection-changed"`; numeric fields carry the new
  rect (or all-zeros when cleared); `layer` / `frame` unspecified.
  The kind catalog comment + module-level doc are updated.
- 8 new wasm unit tests (74 wasm unit total, +10 incl. the 2 new
  events-module tests): fresh-doc has no selection, set stores +
  exposes via getters, set emits event with new bounds, set with
  empty rect clears + emits zeros, clear drops + emits zeros,
  clear emits even with no prior selection, paint between two
  selection changes is the only thing undone (selection not in
  undo stack), off-canvas rect round-trips through the getters.
- Verified: `cargo check --workspace`, `cargo test --workspace`
  (157 pincel-core + 19 aseprite-writer + 8 roundtrip + 5 codec +
  6 command-bus + 3 render + 74 pincel-wasm unit + 2 paint-save-
  open), `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check` all green. `pnpm wasm:build`
  re-generates the JS bindings; the new methods compile cleanly.

### M7 — Selection model on `Sprite` (M7.8a) ✅

- `pincel-core` only; no commands, no wasm, no UI in this slice. Wires
  the data model that M7.8b (`pincel-wasm` surface) and M7.8c (marching-
  ants overlay + Selection tool) build on.
- New `selection: Option<Rect>` field on `Sprite` (sprite coordinates,
  may extend past the canvas — clipping is the consumer's job). The
  builder seeds it to `None`. Spec §5.2 ("Selection (Rect)") is the
  reference; the marquee overlay flag already exists in `Overlays`
  from M3.
- New `Sprite` methods:
  - `set_selection(rect)` — replaces the active selection. An empty
    rect (zero width or height) clears instead of storing a
    degenerate marquee, matching the convention `Rect::is_empty` and
    the Aseprite "drag a zero-width box = no selection" affordance.
  - `clear_selection()` — drops the selection.
  - `has_selection()` — `bool` convenience.
- Intentionally out of the undo stack for M7.8a — Aseprite tracks
  selection-as-command, but the rect-marquee MVP doesn't need it and
  threading selection writes through `Bus` blocks the M7.8c overlay
  work. Listed in open questions; revisit alongside M7.7b (Move
  selection content) or when `SelectionTool` lands undo coverage.
- 6 new unit tests (156 pincel-core unit total): builder default,
  set stores rect, set replaces existing rect, empty rect clears
  (both axes), `clear_selection` drops the rect, off-canvas
  coordinates round-trip without clipping at the model layer.
- Verified: `cargo check --workspace`, `cargo test --workspace`
  (156 pincel-core unit + 19 aseprite-writer + 8 roundtrip + 5
  codec + 6 command-bus + 3 render-compose + 64 pincel-wasm unit +
  2 paint-save-open), `cargo clippy --workspace --all-targets --
  -D warnings`, and `cargo fmt --all --check` green.

### M7 — Move tool viewport pan (M7.7) ✅

- UI-only slice (no wasm changes, no command emitted). Selection-
  content move waits for M7.8.
- `Tool` union widens to include `'move'`. A new **Move** button
  joins the toolbar after Ellipse Fill (`role="group"` /
  `aria-pressed` / `.toolbar-btn-active` pattern unchanged from the
  other tool buttons).
- New state in `App.svelte`: `zoom` (integer multiplier, 1×–64×,
  defaults to 8× — preserves the M6.6 64×64 → 512×512 default look),
  `panX` / `panY` (CSS-pixel offset applied as a
  `transform: translate(...)` on the canvas, relative to the flex-
  centered layout box), `panning` / `panStartClient` / `panStartOffset`
  (in-flight pan-drag state), and `spaceDown` (window-level space-
  key tracker).
- Canvas markup rebuilt: the hardcoded `width="64" height="64"`
  attributes and the `h-[512px] w-[512px]` fixed-CSS classes are
  dropped. The CSS display size is now `canvasW * zoom` × `canvasH *
  zoom`, the canvas is `shrink-0` so it overflows the flex container
  when zoomed beyond viewport, and `style:cursor` switches between
  `grabbing` (active pan) / `grab` (Move tool active or space held)
  / `crosshair` (any other tool). `getBoundingClientRect()` returns
  post-transform dimensions, so `spriteCoord()` keeps working without
  changes — the `(client - rect.left) * canvas.width / rect.width`
  math reduces to `(client - rect.left) / zoom`.
- Pointer pipeline gains a pan branch: `onPointerDown` checks
  `tool === 'move' || spaceDown` before the drag-shape / bucket /
  paint branches and snapshots `(clientX, clientY)` + `(panX, panY)`
  at press; `onPointerMove` applies cursor deltas to pan when
  `panning` is set; `onPointerUp` releases. `paintAt` is hardened to
  ignore the Move tool so a mid-drag tool switch from a paint tool
  to Move during a paint stroke doesn't route through `applyTool`.
- Space-drag (spec §5.2 — Move tool "Pans canvas with space-drag"):
  window-level `keydown` / `keyup` listeners toggle `spaceDown` after
  filtering out events whose target is an `<input>` / `<textarea>` /
  contenteditable. The keydown handler also `preventDefault()`s to
  stop the browser from page-scrolling on space.
- Zoom controls: new `−` / `+` / Reset button group between Move and
  the color picker. `zoomIn` / `zoomOut` double / halve clamped to
  `MIN_ZOOM = 1` / `MAX_ZOOM = 64`; `resetView` resets zoom to 8 and
  pan to zero. The `{zoom}×` readout sits between the +/− buttons.
- The canvas's natural-size flex centering means a sprite at zoom 8
  is centered by default with `panX = panY = 0`; zooming in past the
  viewport size lets the canvas overflow under `overflow-hidden`,
  and pan offsets shift it from the centered position. Sprites
  larger than the default 64×64 also center automatically.
- Verified: `pnpm check` (0 errors / 0 warnings across 384 files),
  `pnpm lint` (clean), `pnpm build` (59 KB JS / 9.7 KB CSS — the
  wasm asset is unchanged at 1.55 MB). `cargo check --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo fmt --all --check` still green.

### M7 — Rectangle tool (M7.4) ✅

- New `pincel_core::DrawRectangle` command (`crates/pincel-core/src/
  command/draw_rectangle.rs`). Rasterizes an axis-aligned rectangle
  between two sprite-space corners — outline if `fill == false`, filled
  if `fill == true`. The outline walks the four edges with corner
  deduplication; the fill walks the interior rect. Both paths capture
  per-modified-pixel prior RGBA for revert. Endpoint order is
  irrelevant; the rasterizer normalizes to `(min_x, min_y, max_x,
  max_y)` before iterating. Pixels outside the target cel are skipped
  silently per the natural drawing-tool clipping semantics; the
  iteration is clipped to the cel bounds up front so even
  `(i32::MIN, i32::MIN)` → `(i32::MAX, i32::MAX)` corners stay
  bounded in memory. Errors mirror `DrawLine` (`MissingCel`,
  `NotAnImageCel`, `UnsupportedColorMode`). No `merge`: each rect is
  its own command, one press-drag-release per undo step. Constructor
  takes corners as `(i32, i32)` tuples so the 6-arg signature
  satisfies `clippy::too_many_arguments` (adding a `fill` flag to a
  4-scalar-coords signature would have crossed the 7-arg limit).
- `AnyCommand::DrawRectangle` variant + `From<DrawRectangle>` impl plumb
  the command through the bus. Re-exported from `pincel_core::lib.rs`
  alongside `SetPixel` / `DrawLine`.
- `pincel-wasm::Document::apply_rectangle(x0, y0, x1, y1, color, fill)`
  (`js_name = applyRectangle`). Packed `0xRRGGBBAA` color, targets the
  same active layer / frame as the pencil (lowest-z `LayerKind::Image`,
  frame 0). Emits a single `dirty-rect` event covering the rectangle's
  axis-aligned bounding box (always positive `width` / `height`,
  endpoint order doesn't matter). Reuses the bbox helper from M7.3,
  which was renamed `line_bbox → endpoint_bbox` to reflect its new
  dual purpose (line segments and rectangles share the same 2-point
  bbox math).
- 18 new `pincel-core` unit tests (122 total): outline / fill apply +
  revert + reversed-endpoint equivalence + single-pixel + 1-D
  degenerate outline + cel-bounds clipping (entire rect outside,
  partial overlap) + offset-cel local coords + missing-cel error +
  no-merge + extreme-endpoint clipping (both modes). 7 new
  `pincel-wasm` unit tests (51 total): outline / fill writes, undo bus
  integration, bbox dirty-rect (forward and reversed endpoints),
  missing image layer error, single-pixel outline.
- UI gains **Rect** and **Rect Fill** toolbar buttons alongside
  Pencil / Eraser / Eyedropper / Line. The `Tool` union widens to
  include `'rectangle'` and `'rectangle-fill'`. The Line tool's
  press / drag / release plumbing is generalized into a shared
  drag-shape pipeline (`isDragShapeTool`, `dragStart`,
  `dragPreview`, `dragTool`, `dragShift`) so the same press / move /
  release handlers drive Line, Rect, and Rect Fill. The new
  `paintRectanglePreview(canvas, x0, y0, x1, y1, color, fill)` helper
  in `lib/render/canvas2d.ts` overlays the in-flight rectangle on
  top of the freshly-blitted composed frame; the helper mirrors the
  Rust rasterizer's outline / fill semantics pixel-for-pixel.
- Shift-to-square is a pure UI affordance: while a Rect / Rect Fill
  drag is in flight, the `constrainedEndpoint()` helper transforms
  the live endpoint so `|dx| == |dy|` (extending the smaller axis to
  match the larger). The Rust command takes raw corners — the
  modifier is applied on `pointermove` / `pointerup` before the
  endpoint is handed to `doc.applyRectangle`. The Line tool does not
  apply the constraint, matching the Aseprite-style "rectangle Shift
  = square, line Shift = TBD" mapping in spec §5.2.

### Build status

`cargo check --workspace`, `cargo test --workspace` (150 pincel-core
unit + 19 aseprite-writer unit + 6 command + 3 render + 5 codec
round-trip + 8 aseprite-writer roundtrip + 64 pincel-wasm unit + 2
pincel-wasm paint-save-open-roundtrip integration),
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo fmt --all --check` are all green on the
`claude/continue-work-f6nLq` branch. `pnpm install`,
`pnpm check`, `pnpm lint`, `pnpm build`, and `pnpm wasm:build` all
pass under `ui/`.

## M7 task breakdown

CLAUDE.md M7 ("Tools expansion") is L-sized so it ships as a
sequence of S/M tasks, one tool per task. Each tool gets its own
`tool_id` slot in `Document::apply_tool` (or a follow-up
single-call API for tools whose semantics need a press / release
pair), a UI button in the toolbar, and a set of tests pinning the
behavior.

- [x] **M7.1** — Eraser. `SetPixel(transparent)` routed through the
  existing command bus; UI toolbar gains a Pencil / Eraser tool
  group.
- [x] **M7.2** — Eyedropper. Read-only sampling of the composed
  canvas at sprite coords through a 1×1 `compose()` viewport. New
  `Document::pickColor(frame, x, y) → u32` method, no command
  emitted. UI button + foreground-color binding (alpha dropped at
  the surface until the input grows alpha support).
- [x] **M7.3** — Line. Bresenham line between press and release; new
  `DrawLine` command storing per-pixel deltas; UI Bresenham preview
  overlay while dragging via a `paintLinePreview` helper that mirrors
  the Rust rasterizer pixel-for-pixel.
- [x] **M7.4** — Rectangle (outline + filled). New
  `DrawRectangle { fill }` command stores per-pixel deltas for revert
  with cel-bounds clipping up front (so extreme corners stay bounded
  in memory); UI ships Rect + Rect Fill buttons sharing a generalized
  drag-shape pipeline with Line, and Shift-to-square is applied as a
  pure UI endpoint transform via `constrainedEndpoint()`.
- [x] **M7.5** — Ellipse (outline + filled). New
  `DrawEllipse { fill }` command uses Zingl's integer midpoint
  algorithm with i64 internal math; outline dedupes per-pixel writes
  through a HashSet so revert restores correctly, fill collects
  per-row x extents to emit horizontal spans, and pathological
  bboxes short-circuit at `> 2^20` axis span. UI ships Ellipse +
  Ellipse Fill buttons on the existing drag-shape pipeline; Shift-
  to-circle is the same `|dx| == |dy|` constraint that already gave
  Rect its square mode (a square bbox inscribes a circle).
- [x] **M7.6** — Bucket. Contiguous 4-connected flood-fill at
  tolerance 0 via a new `FillRegion` command (BFS over a visited
  bitmap, prior-pixel list for revert). UI ships a single-click
  Bucket button; the drag-shape pipeline is untouched.
- [x] **M7.7a** — Move tool viewport pan. UI-only slice: Move
  toolbar button, space-drag pan modifier, integer-zoom 1×–64× with
  `−` / `+` / Reset controls. Canvas display size scales with zoom
  via CSS (the wasm `compose` zoom arg stays at 1×); pan offsets
  apply as `transform: translate(...)` on the canvas. The selection-
  content-move half of M7.7 waits for M7.8.
- [ ] **M7.7b** — Move tool selection-content drag. Depends on M7.8
  (the selection model that defines what gets moved).
- [x] **M7.8a** — `pincel-core` selection model: `selection:
  Option<Rect>` on `Sprite` + `set_selection` / `clear_selection` /
  `has_selection` helpers. Empty rects clear instead of storing a
  degenerate marquee; off-canvas rects round-trip (consumer clips).
- [x] **M7.8b** — `pincel-wasm` selection surface: `setSelection(x,
  y, w, h)` / `clearSelection()` + `hasSelection` / `selectionX` /
  `selectionY` / `selectionWidth` / `selectionHeight` getters. New
  `selection-changed` event kind on the existing event ring so the
  UI can repaint the marching-ants overlay. Selection state is
  intentionally not on the undo stack in this slice (pinned by a
  regression test).
- [ ] **M7.8c** — UI Selection (Rect) tool + marching-ants overlay.
  New toolbar button, marquee press-drag-release pipeline reusing
  the drag-shape infrastructure, animated dashed-rectangle overlay
  in `lib/render/canvas2d.ts`.

Stopping points (per CLAUDE.md §3.3) between each sub-task: every
new public API surface, every dep added to `Cargo.toml` /
`package.json`, every new command type.

## M6 task breakdown

CLAUDE.md M6 ("`pincel-wasm` + minimal Svelte UI") is L-sized so it
ships as a sequence of S/M tasks:

- [x] **M6.1** — `pincel-wasm` crate skeleton: `Document::new`,
  `openAseprite`, `saveAseprite`, basic getters.
- [x] **M6.2** — `Document::compose` returning a `ComposeFrame`
  struct (`width`, `height`, `pixels`). Today the buffer is
  copied into a fresh `Uint8Array`; zero-copy `Uint8ClampedArray`
  views of WASM memory are deferred until `js-sys` is wired up.
- [x] **M6.3** — `Document::applyTool` with a Pencil implementation
  routed through `pincel_core::SetPixel` + the command bus.
  Includes default-layer / default-cel bootstrap so a freshly-
  created document has a paintable target.
- [x] **M6.4** — `Document::drainEvents` skeleton (event enum +
  ring buffer, dirty-rect from M6.3 paints + full-canvas dirty
  on undo / redo) + JS-facing `undo` / `redo` / `undoDepth` /
  `redoDepth`.
- [x] **M6.5** — Svelte 5 + Vite scaffold under `ui/` with Tailwind
  4 set up. `pnpm wasm:build` script invokes `wasm-pack build
  --target web` and produces `crates/pincel-wasm/pkg/`. Empty
  canvas page (8×-zoomed 64×64 Canvas2D placeholder).
- [x] **M6.6** — Wire `pincel-wasm` package into the UI: open file
  via `<input type=file>`, paint with Pencil on the canvas, save via
  download anchor. Single-tool MVP.
- [ ] **M6.7** — End-to-end demo: open hand-crafted fixture, paint,
  save, reopen the saved file in upstream Aseprite to confirm
  validity. Capture screenshots / clip in the PR. Programmatic
  paint→save→open→compose round-trip is now pinned by
  `crates/pincel-wasm/tests/paint_save_open_roundtrip.rs`; the
  remaining work is the human-driven browser + upstream-Aseprite
  verification.

Stopping points (per CLAUDE.md §3.3) between each sub-task: every
new public API surface, every dep added to `Cargo.toml` /
`package.json`.

## Deferred items

**M5 follow-ups beyond CLAUDE.md M5 scope but in spec §8.3.**
Color Profile (`0x2007`, sRGB), Old Palette (`0x0004`, compatibility),
External Files (`0x2008`), User Data (`0x2020`), Slice (`0x2022`),
Tileset (`0x2023`). Land these alongside the milestones that need
them (M8 tilemaps, M9 slices).

**M3 follow-up — additional `BlendMode` variants in `compose()`**
(`docs/specs/pincel.md` §4.2). Still deferred: only `Normal` is
implemented; 18 more for full read/write parity. Not blocking M5–M7.
Plan when a fixture surfaces the need:

1. Decide the canonical reference (Aseprite's `doc/blend_funcs.cpp` is
   the source of truth — link in module docs).
2. Implement per-channel blend functions, dispatched once per pixel
   instead of per blend mode.
3. Snapshot tests against fixtures created in Aseprite.

## Open questions

- M4 drops slice chunks on read. Spec §7.1 says unsupported chunks should
  be "preserved as opaque blobs and round-tripped on save"; that wiring
  needs an `unknown_chunks: Vec<RawChunk>` carrier on `Sprite` (and on
  `Layer` / `Cel` for chunk-attached user data). Defer to M9 alongside
  full slice support.
- `LayerId`s today are assigned by source-file position. That is stable
  across read-only sessions but conflicts with the spec's "stable id"
  promise once the user reorders layers. Revisit once a reorder command
  lands (post-M2 follow-up — not on the current critical path).
- `AddFrame` in M2 is append-only. Mid-list insertion needs a
  `FrameIndex` remap on the cel map (and on `Tag`/`Slice` references).
  Postpone until a tool actually needs it; revisit when the Pencil tool
  starts exercising frame navigation.
- `SetPixel` only supports RGBA color mode. Indexed-mode painting will
  need a separate command (or a payload enum) once an indexed `compose`
  path lands.
- Whether commands should auto-create cels when targeting an empty
  `(layer, frame)` slot. Current behavior: error out with `MissingCel`.
  Defer; the Pencil tool in M6 will be the first caller that has an
  opinion.
- `compose()` currently allocates the output buffer per call. Spec §4.1
  says "must not allocate per-pixel" and mentions pre-allocated scratch
  buffers stored on the document. This is a perf concern, not a
  correctness one — fold into M12.
- `dirty_hint` is accepted but ignored. Wiring it requires the dirty-rect
  tracking described in spec §4.3 (Phase 1.5). Defer to M12.
- Indexed-mode `compose` will need palette lookup; the palette type is
  already in the document model. Add when M3 image-only is no longer
  enough (likely alongside an indexed `SetPixel`).
- `pincel-wasm` returns `Result<_, String>` to keep the surface
  testable on the host target (where `JsError::new` panics). Migrate
  to `JsError` (or a typed `JsValue` payload) once a `wasm-pack
  test --node` job lands and exercises the wasm-only error paths.
- `Document::undo` / `Document::redo` emit a `dirty-canvas` event
  because commands do not yet carry their own dirty region (and an
  arbitrary command — `AddLayer`, `AddFrame`, future `SetPixel` on a
  non-default layer — cannot be attributed without that). Per-command
  dirty-rect tracking is M12 (perf pass); until then full-canvas re-
  render is acceptable for the canvas sizes Phase 1 targets
  (≤ 512×512).
- The `dirty-rect` event queue is bounded at 1024 entries with
  drop-oldest semantics. A pencil that paints one pixel per RAF
  tick can stall ~17 s before any event is dropped. Coalescing a
  pointer drag into a single dirty rect lands when the UI wires
  `applyTool` to pointer events in M6.6.
- `Document::new` seeds a single 100 ms frame. Spec §3.3 implies
  every editable document carries ≥1 frame, but the model itself
  does not require it; `aseprite-writer` happily emits a 0-frame
  file that `aseprite-loader` then refuses to parse. Decide whether
  to enforce ≥1 frame in `SpriteBuilder::build`, or leave it as a
  "valid Pincel document, invalid Aseprite file" affordance.
- M7.7 ships with integer-zoom 1×–64× and a `transform: translate`
  pan offset, but lacks: (1) wheel / pinch zoom, (2) auto-fit on
  document open (a 256×256 sprite at the default 8× zoom overflows
  the viewport; the user must hit `−` until it fits), (3) cursor-
  anchored zoom (the viewport center anchors instead). These are
  ergonomic follow-ups, not correctness gaps; the existing controls
  are enough to verify the Move tool's pan + space-drag behavior.
- Move tool's selection-content drag is not implemented in M7.7a —
  the Move toolbar button only pans the viewport today. The
  selection-content half lands as M7.7b after M7.8 introduces the
  selection model.
- M7.8a stores `selection` on `Sprite` directly (not through a
  command). Aseprite tracks selection changes in its undo stack;
  Pincel currently does not. Revisit when M7.7b ships and the user
  can drag selection content (because that combination — "select →
  drag → undo" — needs the selection edges to come back, not just
  the pixel moves).
- `pincel-wasm` is linked via pnpm's `link:` protocol, which
  expects the `crates/pincel-wasm/pkg/` directory to exist at
  install time. The pkg/ directory is gitignored, so a clean
  checkout has to run `pnpm wasm:build` before `pnpm install`.
  CI / contributor docs should encode that order; a `prepare`
  hook is one option once the deploy story is fleshed out.
- `wasm-opt` is disabled for the `dev` wasm-pack profile in
  `pincel-wasm/Cargo.toml` (M6.5, narrowed after PR #11 review).
  The bundled `wasm-pack` downloader fails to fetch the `binaryen`
  release tarball in the dev environment even when GitHub itself
  is reachable. The `release` profile keeps `wasm-opt` enabled so
  CI / deploy can pick up the size win once those workflows exist;
  contributors building locally use `pnpm wasm:build` (`--dev`)
  for fast iteration and `pnpm wasm:build:release` only when they
  have a working `wasm-opt`. Pin a system `wasm-opt` and point
  `wasm-pack` at it via `WASM_OPT_PATH` in CI when the deploy
  story lands.
