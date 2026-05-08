# Status

_Last updated: 2026-05-08_ (M6.1 `pincel-wasm` crate skeleton)

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

### Build status

`cargo check --workspace`, `cargo test --workspace` (84 pincel-core
unit + 19 aseprite-writer unit + 6 command + 3 render + 5 codec
round-trip + 8 aseprite-writer roundtrip + 5 pincel-wasm unit),
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo fmt --all --check` are all green on the
`claude/continue-from-status-dJv5F` branch.

## M6 task breakdown

CLAUDE.md M6 ("`pincel-wasm` + minimal Svelte UI") is L-sized so it
ships as a sequence of S/M tasks:

- [x] **M6.1** — `pincel-wasm` crate skeleton: `Document::new`,
  `openAseprite`, `saveAseprite`, basic getters. (this commit)
- [ ] **M6.2** — `Document::compose`: expose `pincel_core::compose`
  through a JS-friendly request struct, return RGBA pixels as a
  zero-copy `Uint8ClampedArray` view of WASM memory.
- [ ] **M6.3** — `Document::applyTool` with a Pencil implementation
  routed through `pincel_core::SetPixel` + the command bus.
  Includes default-layer / default-cel bootstrap so a freshly-
  created document has a paintable target.
- [ ] **M6.4** — `Document::drainEvents` skeleton (event enum +
  ring buffer, no producers wired yet beyond `dirty-rect` from
  M6.3 paints).
- [ ] **M6.5** — Svelte 5 + Vite scaffold under `ui/` with Tailwind
  4 set up. wasm-pack build script. Empty canvas page.
- [ ] **M6.6** — Wire `pincel-wasm` package into the UI: open file
  via `<input type=file>`, paint with Pencil on the canvas, save via
  download anchor. Single-tool MVP.
- [ ] **M6.7** — End-to-end demo: open hand-crafted fixture, paint,
  save, reopen the saved file in upstream Aseprite to confirm
  validity. Capture screenshots / clip in the PR.

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
- `Document::new` seeds a single 100 ms frame. Spec §3.3 implies
  every editable document carries ≥1 frame, but the model itself
  does not require it; `aseprite-writer` happily emits a 0-frame
  file that `aseprite-loader` then refuses to parse. Decide whether
  to enforce ≥1 frame in `SpriteBuilder::build`, or leave it as a
  "valid Pincel document, invalid Aseprite file" affordance.
