# Pincel — Pixel-Art and Image Editor

> **Status:** Draft, Specification v0.1
> **Type:** Living Document
> **Last updated:** 2026-09-06
> **Owner:** Daniel Rück / amigo-labs

---

## 1. Goals & Non-Goals

### 1.1 Goals

Pincel is a pixel-art and image editor in one app: a pixel-art editor at the feature level of Aseprite, focused on **game asset creation**, and next to it a general-purpose, paint.net-style image editor for photos, screenshots, mockups and artwork. It ships as:

- A **standalone Progressive Web App** (PWA, installable, offline-capable)
- A **native desktop app** via Tauri (sharing the same UI codebase)

The two builds share one Svelte UI and the same Rust cores. The native build adds OS file-system integration via Tauri commands; the PWA uses File System Access API with download fallback.

**Two modes (since 2026-09-06).** The app hosts two editors behind one shell: **Pixel mode** (this spec — sprites, frames, palettes, tilemaps, `.aseprite`) and **Image mode**, the general-purpose paint.net-style editor formerly shipped as the sister project `amigo-fineliner` (its tool set is §5.4; its crates live in this workspace as `pincel-image-core` / `pincel-image-wasm`, and the archived sibling's `docs/specs/fineliner.md` holds its original design notes). The mode is a property of the open document: `.aseprite` opens in Pixel mode, PNG / JPEG / WebP / BMP / GIF / TIFF open in Image mode, and the New dialog offers both. The two Rust cores stay separate (different document models and compositing semantics); they share the effects crate, the Svelte shell, file I/O, persistence, PWA and Tauri layers. See §15 (2026-09-06, "Two cores, one shell").

### 1.2 Engine Interop Strategy

Pincel does **not** integrate as a library or plugin into `amigo-engine`. Instead, interop is achieved via the Aseprite file format:

- Pincel reads and writes `.aseprite` files
- `amigo_assets` already imports Aseprite files and supports hot-reloading
- The user opens a file in Pincel, edits it, saves it; the running engine instance reloads automatically

This keeps both projects fully decoupled. Pincel works for Amigo Engine users *and* for anyone else using the Aseprite ecosystem (Godot, LDtk, Unity, custom engines).

### 1.3 Non-Goals (Phase 1)

- No proprietary file format. `.aseprite` is Pixel mode's only source-of-truth format; Image mode keeps layers in OpenRaster (`.ora`).
- No Lua scripting / Aseprite extension API compatibility.
- No plugin system.
- No collaborative / multi-user editing.
- No 3D, no vector graphics, no high-bit-depth (HDR) painting.
- No atlas / sprite-sheet packing — `amigo_assets` does this; Pincel only exports flat PNG sheets when explicitly requested.

---

## 2. Architecture Overview

### 2.1 Workspace Structure

```
amigo-pincel/
├── crates/
│   ├── pincel-core/             Rust library: document model, tools, codecs, compose()
│   ├── aseprite-writer/         Rust library: standalone, publishable, MIT/Apache-2.0
│   └── pincel-wasm/             Rust crate: wasm-bindgen layer, cdylib
├── ui/                          Svelte 5 + Vite + Tailwind 4 + shadcn-svelte
│   ├── src/
│   │   ├── lib/
│   │   │   ├── core/            wasm import + typed API wrappers
│   │   │   ├── render/          WebGPU + Canvas2D adapters
│   │   │   ├── tools/           Tool input handlers (mouse/touch → core)
│   │   │   ├── components/      Panels, dialogs, toolbar, timeline
│   │   │   └── stores/          Svelte 5 runes for app state
│   │   └── routes/
│   ├── static/
│   ├── service-worker.ts
│   └── manifest.webmanifest
├── src-tauri/                   Tauri 2 native shell, uses pincel-core directly
│   ├── src/
│   └── tauri.conf.json
├── docs/
│   └── specs/
│       └── pincel.md            ← this document
├── Cargo.toml                   Workspace root
└── README.md
```

The UI build is consumed by both PWA and Tauri. Tauri loads the same Vite output via its built-in dev server / bundled assets. Native-only features (true file-system access, OS dialogs, recent-files registry) are exposed as Tauri commands that the UI feature-detects at runtime.

### 2.2 High-Level Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                      UI Layer (Svelte 5)                     │
│                                                              │
│  Toolbar │ Canvas Viewport │ Layers │ Timeline │ Palette │   │
│          │                 │ Tilesets │ Slices │             │
│          │                 │                                 │
│          ▼                                                   │
│  ┌───────────────┐         ┌──────────────────┐              │
│  │ Render Adapter│◄────────│ WASM API (typed) │              │
│  │ WebGPU/Canvas2│         └──────────────────┘              │
│  └───────────────┘                  │                        │
└─────────────────────────────────────┼────────────────────────┘
                                      ▼
┌──────────────────────────────────────────────────────────────┐
│                   pincel-core (Rust)                         │
│                                                              │
│  Document ── Layers (Image│Tilemap│Group)                    │
│           ── Frames ── Cels                                  │
│           ── Palette ── Tilesets ── Slices ── Tags           │
│                                                              │
│  Tools (Pencil, Bucket, Line, …) ──► Commands                │
│                                          │                   │
│  Command Bus ◄─────────────── Undo/Redo Stack                │
│                                                              │
│  compose(viewport, zoom) ──► RGBA Buffer                     │
│                                                              │
│  Codec ── aseprite-loader (read) ── aseprite-writer (write)  │
│        ── png export                                         │
└──────────────────────────────────────────────────────────────┘
```

### 2.3 Crate Responsibilities

**`pincel-core`** — pure Rust, `no_std`-compatible where possible (currently `std`-only, but no platform-specific I/O). Knows nothing about files-on-disk or rendering APIs. Codec functions take `Read`/`Write` traits. Render output is a `Vec<u8>` RGBA buffer plus metadata (dirty rect, viewport transform).

**`aseprite-writer`** — independent crate, designed to be published to crates.io alongside (and complementing) `aseprite-loader` / `asefile`. Implements the Aseprite v1.3 format spec for writing. Round-trip tests against `aseprite-loader` ensure compliance.

**`pincel-wasm`** — thin wrapper, exposes `pincel-core` to JS via `wasm-bindgen`. Owns the document state in Rust memory. JS calls methods, receives RGBA buffer views (zero-copy where possible) and event notifications.

**`ui/`** — Svelte 5 application. Imports `pincel-wasm` package built via `wasm-pack`. Uses Svelte 5 runes (`$state`, `$derived`, `$effect`) for reactive state. The render adapter translates document buffers into WebGPU textures or Canvas2D `ImageData` depending on capability detection.

**`src-tauri/`** — Tauri 2 shell. Depends on `pincel-core` directly (no WASM round-trip for native I/O). Exposes Tauri commands for native FS, OS dialogs, recent files, and "open with…" association.

---

## 3. Document Model

The Pincel document model is **isomorphic to the Aseprite v1.3 file format**. Every concept maps cleanly to a chunk type in `.aseprite`. This is a deliberate constraint: it guarantees round-trip fidelity and avoids a translation layer.

### 3.1 Top-Level Types

```rust
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
    pub layers: Vec<Layer>,           // Z-order: index 0 is bottom
    pub frames: Vec<Frame>,           // sequential
    pub palette: Palette,
    pub tilesets: Vec<Tileset>,
    pub tags: Vec<Tag>,                // animation tags
    pub slices: Vec<Slice>,
    pub metadata: Metadata,
}

pub enum ColorMode {
    Rgba,                              // 32 bpp
    Indexed { transparent_index: u8 }, // 8 bpp + palette
    Grayscale,                         // 16 bpp (V+A) — Phase 2
}
```

### 3.2 Layers

```rust
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    pub editable: bool,
    pub blend_mode: BlendMode,
    pub opacity: u8,                   // 0..=255
    pub parent: Option<LayerId>,       // for group nesting
}

pub enum LayerKind {
    Image,
    Tilemap { tileset_id: TilesetId },
    Group,
}
```

`Sprite.layers` is flat and bottom-first; a group's subtree is one contiguous run right after the group header. `MoveLayer` / `ReorderLayer` move a layer among its siblings; `MoveLayerTo { layer, target, place: Above | Below | Into }` moves it (with its subtree) across groups — next to `target`, sharing its parent, or into the `target` group as its top child — keeping that run contiguous (§15, 2026-10-12).

Cels are stored separately, keyed by `(LayerId, FrameIndex)`:

```rust
pub struct Cel {
    pub layer: LayerId,
    pub frame: FrameIndex,
    pub position: (i32, i32),          // top-left in sprite coords
    pub opacity: u8,
    pub data: CelData,
}

pub enum CelData {
    Image(PixelBuffer),                // RGBA8 or indexed
    Tilemap {
        grid_w: u32,
        grid_h: u32,
        tiles: Vec<TileRef>,
    },
    Linked(FrameIndex),                // points to another cel in same layer
}

pub struct TileRef {
    pub tile_id: u32,                  // index into Tileset
    pub flip_x: bool,
    pub flip_y: bool,
    pub rotate_90: bool,
}
```

`Linked` cels are an Aseprite optimization: identical content across frames is stored once. Pincel must preserve linkage on read and may optionally create links on write (Phase 1: write expanded only; Phase 2: link-on-write).

### 3.3 Frames

```rust
pub struct Frame {
    pub duration_ms: u16,              // per-frame, Aseprite native
}
```

Frame-level data is intentionally minimal. Per-frame cel data lives in the `Cel` map. Slice keys (per-frame slice geometry) are stored on the `Slice` itself, not the frame.

### 3.4 Tilesets

```rust
pub struct Tileset {
    pub id: TilesetId,
    pub name: String,
    pub tile_size: (u32, u32),         // e.g. (16, 16)
    pub tiles: Vec<TileImage>,         // tile 0 is the empty tile
    pub base_index: i32,               // typically 1; tile 0 = empty
    pub external_file: Option<PathRef>,// Aseprite supports shared tilesets
}

pub struct TileImage {
    pub pixels: PixelBuffer,
}
```

Aseprite tile 0 is conventionally the "empty" tile. Pincel keeps this convention to maintain compatibility.

### 3.5 Slices

Slices are named rectangles overlaid on the sprite, optionally per-frame-keyed.

```rust
pub struct Slice {
    pub id: SliceId,
    pub name: String,
    pub color: Rgba,                   // visual color in editor
    pub keys: Vec<SliceKey>,           // sorted by frame
}

pub struct SliceKey {
    pub frame: FrameIndex,             // first frame this key applies to
    pub bounds: Rect,
    pub center: Option<Rect>,          // 9-patch inner rect (optional)
    pub pivot: Option<(i32, i32)>,     // pivot point (optional)
}
```

A slice key applies from its `frame` until the next key's `frame`. This is the Aseprite semantics; do not reinvent it.

### 3.6 Tags

```rust
pub struct Tag {
    pub name: String,
    pub from: FrameIndex,
    pub to: FrameIndex,
    pub direction: TagDirection,
    pub color: Rgba,
    pub repeats: u16,                  // 0 = infinite
}

pub enum TagDirection {
    Forward,
    Reverse,
    Pingpong,
    PingpongReverse,
}
```

Tags are essential for `amigo_animation`'s state-machine consumption. The state machine identifies states by tag name; ensure tag names are validated as identifier-like in the editor (warn on whitespace/special chars).

### 3.7 Palette

```rust
pub struct Palette {
    pub colors: Vec<PaletteEntry>,     // up to 256 for indexed mode
}

pub struct PaletteEntry {
    pub rgba: Rgba,
    pub name: Option<String>,          // Aseprite supports named entries
}
```

A new Pixel-mode document starts with the **DB32** palette (DawnBringer 32, Aseprite's default). The palette is editable through undoable commands: recolour an entry (`SetPaletteColor`, drags merge), append (`AddPaletteEntry`, max 256), remove (`RemovePaletteEntry`) and reorder (`MovePaletteEntry`). In RGBA documents the palette is a swatch set no pixel references; in indexed documents removing or reordering would repaint pixels, so those two are refused until indexed painting lands (Phase 2).

The Palette panel also loads a **preset** — DB32, DB16, PICO-8, Game Boy (four greens), NES (54-colour 2C02 table) — and imports / exports the **GIMP palette** format (`.gpl`: `GIMP Palette` header, optional `Name:` / `Columns:`, `#` comments, `R G B name` rows, at most 256 entries). Both replace the whole palette through one undoable `ReplacePalette`, refused on indexed sprites for the same reason as remove / reorder.

---

## 4. Render Pipeline

### 4.1 The `compose()` Contract

`pincel-core` provides a single composition entry point:

```rust
pub fn compose(
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
) -> ComposeResult;

pub struct ComposeRequest {
    pub frame: FrameIndex,
    pub viewport: Rect,                // in sprite coordinates
    pub zoom: u32,                     // integer zoom, 1..=64
    pub onion_skin: Option<OnionSkin>,
    pub include_layers: LayerFilter,   // visible-only by default
    pub overlays: Overlays,            // grid, slices, selection marquee
    pub dirty_hint: Option<Rect>,      // for incremental repaint
}

pub struct ComposeResult {
    pub pixels: Vec<u8>,               // RGBA8, len = vp.w*vp.h*4*zoom²
    pub width: u32,                    // viewport.w * zoom
    pub height: u32,
    pub generation: u64,               // monotonic, for the UI to detect staleness
}
```

The function is **pure** with respect to inputs and **must not allocate per-pixel**. It uses pre-allocated scratch buffers stored on the document for compositing. Layer compositing applies blend modes in z-order, then the result is upscaled by zoom (nearest-neighbor) and overlays are drawn on top.

### 4.2 Compositing Strategy

- Convert indexed pixels to RGBA at composite time (palette lookup)
- Apply per-cel opacity and blend mode bottom-up
- Group layers fold their children into a temporary buffer, then blend that buffer into the parent
- Tilemap cels expand to image data via tileset lookup, with flip/rotate flags applied during expansion
- Onion skin renders previous frame at user-defined alpha (default 0.3) tinted red, next frame tinted blue

### 4.3 Dirty-Rect Optimization (Phase 1.5, not blocking MVP)

The document tracks dirty rectangles per-cel. `compose()` accepts a `dirty_hint` and returns only the changed region. The UI render adapter then uploads a sub-rect to the GPU texture instead of the full frame. Skip in initial MVP; revisit when 512×512+ canvases feel sluggish.

### 4.4 UI Render Adapter

The render adapter lives in TypeScript under `ui/src/lib/render/`. Two backends:

**WebGPU backend (`render/webgpu.ts`)**
- Capability detection: `'gpu' in navigator && await navigator.gpu.requestAdapter() !== null`
- Single full-screen quad pipeline, sampling a 2D texture
- Texture upload via `device.queue.writeTexture()` with the RGBA buffer view exposed by `pincel-wasm`
- For pixel-perfect rendering, core composes at the exact zoom (nearest-neighbor in CPU) and GPU just blits. This avoids subpixel sampling artifacts.

**Canvas2D backend (`render/canvas2d.ts`)**
- Used as fallback or when WebGPU is unavailable (Safari pre-GA, restrictive enterprise)
- `ImageData` from the RGBA buffer, drawn via `putImageData`
- Pan/zoom managed by core (same compose contract); Canvas2D does no scaling

A common interface abstracts both:

```typescript
interface CanvasRenderer {
  resize(w: number, h: number): void;
  draw(rgba: Uint8ClampedArray, w: number, h: number): void;
  destroy(): void;
}
```

The adapter chooses the backend at startup; users can force Canvas2D via a setting (debug toggle).

---

## 5. Tool System

### 5.1 Tool Trait

```rust
pub trait Tool {
    fn id(&self) -> ToolId;
    fn cursor(&self, ctx: &ToolContext) -> CursorShape;

    fn on_press(&mut self, pt: PixelPoint, mods: Mods, ctx: &mut ToolContext)
        -> Option<Command>;
    fn on_drag(&mut self, pt: PixelPoint, mods: Mods, ctx: &mut ToolContext)
        -> Option<Command>;
    fn on_release(&mut self, pt: PixelPoint, mods: Mods, ctx: &mut ToolContext)
        -> Option<Command>;
}
```

Tools never mutate the document directly. They emit `Command`s. The command bus applies them and pushes onto the undo stack.

### 5.2 Phase 1 Tool Set (Pixel mode)

| Tool | Behavior |
|------|----------|
| Pencil | Single-pixel line, brush-size variants 1–8, hold Shift = constrain to 0/45/90° |
| Eraser | Clears to transparent (RGBA) or transparent-index (Indexed) |
| Bucket | Paint-fill, contiguous (default) or all-matching, tolerance 0 in MVP |
| Line | Bresenham line between press and release |
| Rectangle | Outline + filled variants, hold Shift = square |
| Ellipse | Midpoint algorithm, outline + filled, hold Shift = circle |
| Eyedropper | Sets foreground color from canvas pixel; hold Alt with any tool for temporary eyedropper |
| Move | Pans canvas with space-drag; moves selection content if active |
| Selection (Rect) | Marquee selection with marching-ants overlay |
| Tilemap Stamp | In tilemap layer: places selected tile from tileset at grid cell |
| Slice Tool | Drags out a new slice rect, drag corners to resize, double-click for properties (name, 9-patch center, pivot) |

### 5.3 Tileset Editor Sub-Mode

When the active layer is a Tilemap, the UI exposes a Tileset Panel showing all tiles in the layer's tileset. Clicking a tile enters "edit tile" sub-mode: the canvas shows that single tile at high zoom, normal image tools (Pencil, Eraser, etc.) operate on it. Returning to the tilemap view auto-updates all instances of that tile in the layer.

Auto-tile-detection (Aseprite's "Auto" mode where painting on the tilemap layer at tile resolution either reuses or creates tiles) is **deferred to Phase 2**. Phase 1 supports manual tile placement and explicit tile editing.

### 5.4 Image mode tool set

Image mode is the paint.net-style editor (§1.1). Its tools live in `crates/pincel-image-core/src/tools/`; the rail, options bar and keys are §9.2 and §9.4. Where a key appears in both modes it means the same thing; a tool only one mode has keeps its key to itself.

| Tool | Key | Behavior |
|------|-----|----------|
| Move | `V` | Three variants: the whole layer, the selected pixels (lifted, leaving transparency, composited back at the offset), or the selection outline alone |
| Pan | `H` | Drags the viewport. The same gesture as Space-drag and middle-drag, as a rail button |
| Zoom | `Z` | Click steps in, Alt+click out; a drag frames the area to fill the viewport |
| Marquee | `M` | Rectangle and ellipse selection, with a feather radius |
| Lasso | `Q` | Freehand and polygonal; the polygon closes on the first vertex, a double-click or Enter |
| Magic Wand | `W` | Contiguous or global color selection with a tolerance, sampling the layer or the composite |
| Brush | `B` | Hard round / soft round / flat tip, size, hardness, opacity, pressure size / opacity, nib angle following tilt |
| Eraser | `E` | To transparent, or to the background color |
| Recolor | `R` | A brush that writes only where it finds the background color, within a tolerance, and paints the foreground there |
| Smudge | `S` | Carries the color under the tip along the stroke. Strength governs how far: 0 changes nothing, 1 drags one color the whole way |
| Paint Bucket | `G` | Flood fill, contiguous or global, with a tolerance, sampling the layer or the composite |
| Gradient | `N` | Linear / reflected / radial / conical / diamond ramp along the drag, in the Color or Transparency channel |
| Eyedropper | `I` | Samples 1 / 3 / 5 / 11 / 31 px from the layer or the composite |
| Line | `L` | A straight line with the shape style (width, dash, anti-alias) |
| Shapes | `U` | Rectangle, rounded rectangle, ellipse and regular polygon, as outline / fill / both |
| Text | `T` | Rasterised on commit, with alignment, faux bold / italic and anti-alias |

All selection modes combine with Shift (add), Alt (subtract) and both (intersect), and every pixel tool is bounded by the active selection, feathered edges included.

**Paste.** Edit ▸ Paste (Ctrl+V / ⌘V) puts the image on the clipboard on a new layer above the active one, at the top-left corner, in one undo step. When the image is larger than the canvas, a dialog asks first, like paint.net: *Expand canvas* grows the canvas to fit it (anchored top-left, within the image-size limits), *Keep canvas size* crops it to the canvas, *Cancel* drops it. Pixel mode has no paste yet.

Not implemented: a clone stamp, bézier curves, and custom or textured brushes. Selection arithmetic beyond the four modes (saving a selection, transforming one) is Phase 2 (§13).

---

## 6. Command / Undo System

### 6.1 Command Pattern

```rust
pub trait Command: Send {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap);
    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap);
    fn merge(&mut self, next: &Self) -> bool { false }
}
```

`merge` returns true if the next command of the same kind can be coalesced (e.g., consecutive pencil strokes within the same press-drag-release sequence become one undo entry).

### 6.2 History

Linear undo stack with a configurable cap (default 100 entries). No branching history in MVP. On document save, the history is **not** cleared — the user can undo across saves until app close. Optional: persist history into the `.aseprite` user-data chunk for cross-session undo (Phase 2).

### 6.3 Diff Granularity

Image-cel commands store **dirty-rect deltas** (the bounding box of changed pixels plus before/after pixel data for that rect), not full cel snapshots. This keeps memory bounded for large canvases. Tilemap commands store affected `(grid_x, grid_y) → (old_tile, new_tile)` lists.

---

## 7. File I/O

### 7.1 Aseprite Read

Use **`aseprite-loader`** (`docs.rs/aseprite-loader`) as the primary parser. Rationale: zero-copy, MIT/Apache, actively maintained, supports format v1.3 including tilemaps and slices.

A thin adapter in `pincel-core::codec::aseprite_read` translates parser output into Pincel's `Sprite` model. Unsupported chunks (custom user-data extensions, future Aseprite versions) are preserved as opaque blobs and round-tripped on save (`unknown_chunks: Vec<RawChunk>` on each carrier type).

Fields Pincel does not edit but must not lose on open → save (implemented 2026-10-11): the header's pixel aspect ratio and grid (`Metadata::pixel_ratio` / `grid`; `color_count` is re-derived from the palette size), layer flag bits beyond visible / editable (`Layer::extra_flags`), the cel z-index (`Cel::z_index`, stored but not yet applied by `compose()`), and user data — text note, colour and property maps — on the sprite, layers, cels and tags (`UserData`). Property maps are kept as the opaque on-disk block (`UserData::properties`, read by a small chunk walk of our own, written back verbatim); a tag direction byte above 3 is kept in `Tag::raw_direction`, plays as forward and is written back until the direction is edited. The External Files chunk (`0x2008`) is kept as `Metadata::external_files` (type byte as read); user data on a tileset and on its tiles as `Tileset::user_data` / `tile_user_data` (sparse by tile id); a slice's note text and property maps as `Slice::user_data`, next to its overlay `color`. Still open: *resolving* an external palette or tileset (a tileset stored only in another file is still refused on open, Phase 2) and the general opaque-chunk mechanism above.

### 7.2 Aseprite Write

Implemented in the **`aseprite-writer`** crate (see Section 8). `pincel-core::codec::aseprite_write` produces a byte stream consumable by `Write`.

### 7.3 PNG Export

Two modes:

- **Single frame**: current frame composited and saved as PNG
- **Sprite sheet**: all frames laid out in a grid (rows/cols configurable, packed left-to-right top-to-bottom by default)

Sprite-sheet export emits an optional sidecar JSON describing frame durations, tags, and slice rects. Format follows Aseprite's `--data` JSON output for ecosystem compatibility (LDtk, Phaser, etc. consume this format).

### 7.4 Engine Build Pipeline

Pincel does **not** export to `.ait` directly. The engine's existing `amigo_assets` pipeline reads `.aseprite` files and produces engine-internal formats. Pincel's only contract with the engine is: produce a valid `.aseprite` file at a known location.

---

## 8. Crate: `aseprite-writer`

A standalone, publishable Rust crate. Lives in the Pincel workspace but has no Pincel dependencies — it exposes the Aseprite types directly.

### 8.1 Goals

- Implement Aseprite v1.3 file format **writing**
- Spec source: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>
- License: MIT OR Apache-2.0 (matches `aseprite-loader`)
- Round-trip tested against `aseprite-loader`: read file → re-write → read again → assert equivalent

### 8.2 API Sketch

```rust
pub struct AseFile { /* matches loader's data model */ }

pub fn write<W: Write>(file: &AseFile, out: &mut W) -> Result<(), WriteError>;

pub struct WriteOptions {
    pub compression: Compression,      // Zlib (default), None
    pub link_identical_cels: bool,     // optimize file size (Phase 2)
}
```

### 8.3 Required Chunks (Phase 1)

| Chunk | Hex | Purpose |
|-------|-----|---------|
| Old Palette | 0x0004 | Compatibility (legacy palette) |
| Layer | 0x2004 | Layer metadata |
| Cel | 0x2005 | Image and tilemap cel data |
| Color Profile | 0x2007 | sRGB declaration |
| External Files | 0x2008 | Reference external tilesets |
| Tags | 0x2018 | Animation tags |
| Palette | 0x2019 | Modern palette with names + alpha |
| Slice | 0x2022 | Slice rectangles + 9-patch + pivot |
| Tileset | 0x2023 | Tile pixel data |
| User Data | 0x2020 | Round-trip preservation |

Compression: Zlib for cel pixel data (matches Aseprite default). Use the `flate2` crate.

### 8.4 Test Strategy

- Fixture-based: a `tests/fixtures/` directory with diverse `.aseprite` files (RGBA, indexed, with tilemaps, with slices, with linked cels, with all blend modes)
- For each fixture: load with `aseprite-loader`, write with `aseprite-writer`, load again, assert structural equality
- Property-based fuzzing (via `proptest`) on randomly-generated documents for serialization robustness
- A separate test verifies the actual Aseprite application can open Pincel-written files (manual gate, run before each release)

---

## 9. UI Architecture

### 9.1 Stack

- **Svelte 5** with runes (`$state`, `$derived`, `$effect`) for reactivity
- **Vite** as bundler
- **Tailwind 4** for styling
- **shadcn-svelte** for components (copied into project, not a runtime dependency)
- **wasm-pack** to build `pincel-wasm` as an npm-importable package

### 9.2 Layout

```text
┌────────────────────────────────────────────────────────────────────────┐
│ Top bar: File Edit Select Image Layer [Frame] Effects Adjustments View │
│          [Help] · document name ● · undo / redo · panels toggle        │
├──────┬────────────────────────────────────────────────┬────────────────┤
│ Tool │ Options bar: variants of the active tool group │  Colors        │
│ rail │ + that tool's options                          ├────────────────┤
│      ├────────────────────────────────────────────────┤  Palette       │
│      │                                                ├────────────────┤
│      │            Canvas viewport                     │  Layers        │
│      │           (WebGPU / Canvas2D)       [− % + 1:1]├────────────────┤
│      ├────────────────────────────────────────────────┤  Tilesets ▸    │
│      │ Timeline: transport · frames · tags · onion    │  Slices ▸      │
│      ├────────────────────────────────────────────────┤                │
│      │ Status: doc size · frame · undo depth · status │                │
└──────┴────────────────────────────────────────────────┴────────────────┘
```

Both modes share this frame (`ui/src/lib/components/layout/`); Image mode lists the panels and menus it has (Colors and Layers; no Frame menu, no timeline). Panels are collapsible and their collapse state and sidebar visibility persist per mode (IndexedDB prefs). The tool rail groups variants of one tool — the six shapes, the two marquee shapes, the two lasso kinds — behind one button; the options bar shows the group's variants as a segmented control, so no variant needs a long-press. Icons are inline SVG strokes (`ui/src/lib/components/icons/`), colors come from one set of `pc-*` theme tokens. A Help menu (About Pincel, and in the desktop app the updater's manual check and its auto-install toggle) closes the bar wherever the OS draws no native menu bar — the web build and the Windows / Linux desktop app; on macOS the native Help menu serves those entries instead (§11.3).

**Responsive layout.** The frame restructures at Tailwind's `sm` / `lg` widths:

| Window | Menus | Tools | Panels |
|--------|-------|-------|--------|
| ≥ 1024 px (desktop) | menu bar | vertical rail | docked sidebar, toggleable |
| 640–1023 px (tablet) | menu bar (scrolls if needed) | vertical rail | drawer over the canvas |
| < 640 px (phone) | ☰ sheet with every menu | horizontal strip at the bottom | bottom sheet, one tab per panel (timeline included) |

**Layers panel.** Both modes render the same panel (`ui/src/lib/components/layers/`): each row has a drag grip (pointer events, so it works with touch and pen; arrow keys on a focused grip), visibility, lock, a thumbnail, the name and a rename button; the active layer's opacity and blend mode sit below the list. Pixel mode lists layers as a tree — a group's header above its indented children — tags tilemap / group rows, and lets a drag cross groups: the outer quarters of a row drop above / below it, a group's middle drops into it (Alt+← / → on a grip: out of / into a group); Layer ▸ New Group adds an empty group. A locked layer — or one inside a locked group — refuses pixel edits with a status-bar notice. Layer ▸ Rename… opens the same rename dialog in both modes.

**Colors.** Both modes have a foreground and a background color. The left button paints with the foreground, the right button with the background (pencil, fill, line, shapes; the eyedropper samples into whichever button it was pressed with); filled shapes use the other color for their interior. `X` swaps, `D` resets to black / white; right-clicking a palette swatch sets the background.

The layout above is the Pixel-mode editor. It is mounted by a thin shell (`ui/src/App.svelte`) that also owns the start screen (New pixel sprite / New image / Open / Recents — the two New cards open in place as an accordion with the canvas size and Create, no dialog on top), the New-document dialog with a mode choice (File ▸ New… from an editor), an "Open like this on startup" checkbox on each New card and in the New dialog (at most one mode at a time: the next launch skips the start screen and opens an empty document of that mode and size), a New image size taken from the image on the clipboard when there is one (like paint.net), file-type detection on open, and the Tauri menu bridge. Image mode mounts its own editor (`ui/src/modes/image/`) in the same slot; only one editor is mounted at a time, so the two keyboard maps never overlap.

### 9.3 WASM Boundary Contract

**State stays in Rust.** The UI never holds a serialized document. It calls thin methods:

```typescript
import init, { Document } from '@amigo-labs/pincel-wasm';

const doc = Document.new(64, 64, ColorMode.Rgba);
doc.applyTool(toolId, { x, y, button, mods, phase });
const frame = doc.compose(viewport, zoom, options);
//  ^ frame.pixels is a Uint8ClampedArray view into WASM memory (zero-copy)
```

The UI pulls events via:

```typescript
const events = doc.drainEvents();
// events: layer-changed, palette-changed, undo-pushed, dirty-rect, …
```

`drainEvents()` is called every frame (RAF-driven). Stores update from events; views re-render via Svelte reactivity.

### 9.4 Input Handling

Mouse and pen input is captured on the canvas element with the full `PointerEvent` API (pressure, tilt, pointer-type). Touch input is supported with two-finger pan/zoom. Keyboard shortcuts are defined in a single `shortcuts.ts` map (`ui/src/lib/shell/shortcuts.ts`) that mirrors Aseprite's defaults where reasonable and is shared by both modes: the same key means the same thing in Pixel and Image mode, menus and tooltips print their hints from it, and each editor binds the actions it has. Tool keys select a tool group and repeated presses cycle its variants: `V` move, `M` marquee, `Q` lasso, `W` magic wand, `B` pencil / brush, `E` eraser, `G` fill, `I` eyedropper, `L` line, `U` shapes, `T` text, and in Image mode `H` pan, `Z` zoom, `R` recolor, `S` smudge, `N` gradient (§5.4). A key a mode has no group for does nothing there rather than meaning something else. Bare `+` / `-` / `0` / `1` zoom in, out, to fit and to 1:1; Space-drag pans; `[` / `]` resize the brush; `,` / `.` / Enter step and play frames. On macOS the desktop app's native menu owns its accelerators (open / save / export / undo / redo) so they never fire twice; on Windows, Linux and the web the editors bind them themselves. The gate is `hasNativeMenu()` (`ui/src/lib/platform/`), not `isTauri()`.

Touch and pen have no keyboard, so the editor frame shows two latching toggles, Shift and Alt, on touch-first devices (and once a touch or pen pointer touches the stage). Tool code asks `shiftHeld(e)` / `altHeld(e)` (`ui/src/lib/shell/modifiers.svelte.ts`), so a latched toggle and a held key mean the same: Shift squares shapes and snaps lines to 45°, adds to a selection; Alt subtracts; both intersect.

### 9.5 Language

The UI ships in English and German. Strings live in a typed dictionary under `ui/src/lib/i18n/` — `en.ts` defines every key, other locales are typed against it so a missing translation fails the type check — and components read them through `t(key, params)`, which follows the reactive locale. The first launch follows the browser language (`de-*` → German, anything else → English); the choice made on the start screen or under View ▸ Language is stored in the IndexedDB `prefs` store and wins from then on. The macOS desktop app rebuilds its native menu in the same language (`set_menu_language`); on Windows and Linux there is no native menu and the command is a no-op. File formats, layer names and other document content are never translated.

---

## 10. PWA-Specific

### 10.1 Service Worker

`vite-plugin-pwa` with the `injectManifest` strategy. The service worker precaches the WASM bundle, UI assets, and Tailwind CSS. Documents are NOT cached by the service worker — they live in IndexedDB.

### 10.2 File System

- **Modern browsers (Chrome, Edge, Opera)**: File System Access API (`showOpenFilePicker`, `showSaveFilePicker`). User grants per-file or per-directory access; Pincel can save in place.
- **Firefox / Safari**: download fallback. Open via `<input type="file">`, save via `Blob` + anchor download.
- **Capability detection**: feature-flag the UI; show "Save" vs "Save As (download)" labels accordingly.

### 10.3 Persistence

- **IndexedDB**: autosave snapshots every 30 seconds, recent-files registry, settings/preferences
- Each document gets a stable UUID; autosave snapshots are keyed by `(doc_uuid, timestamp)`
- On open, Pincel checks for unsaved changes from a previous session and offers recovery
- Both modes run the same session (`ui/src/lib/idb/autosave-session.svelte.ts`): a clean document (undo depth at the last save) is never written; a save or a fresh open drops the document's snapshots; a **recovered document counts as unsaved** and keeps its snapshot until the user saves; discarding the last listed snapshot closes the Recovery dialog

### 10.4 Threading

Initial MVP: single-threaded WASM. `compose()` runs on the main thread. Acceptable up to ~512×512 canvases at zoom ≤ 16.

Phase 2: move heavy operations (full-canvas compose, large-area paint-fill, filters) to a Web Worker via `comlink`. Requires `SharedArrayBuffer` for zero-copy texture upload, which requires COOP/COEP headers on the hosting. Document this in deployment notes.

---

## 11. Tauri-Specific

### 11.1 Stack

Tauri 2, WebView2 / WKWebView / WebKitGTK depending on platform. The bundled UI is the same Vite output as the PWA build; Tauri loads it from `dist/`.

### 11.2 Native Commands

Defined in `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
async fn open_file(path: String) -> Result<DocumentDto, String>;

#[tauri::command]
async fn save_file(path: String, doc: DocumentDto) -> Result<(), String>;

#[tauri::command]
async fn recent_files() -> Vec<RecentFile>;

#[tauri::command]
async fn show_open_dialog() -> Option<String>;

#[tauri::command]
async fn show_save_dialog(suggested_name: String) -> Option<String>;
```

The native build uses these instead of the File System Access API. `pincel-core` is a direct Cargo dependency of `src-tauri/`, so commands can call codec functions without WASM round-trip.

### 11.3 OS Integration

- File-association: register `.aseprite` and `.ase` (with user opt-in on first launch)
- Recent-files in OS jump lists (Windows) / dock menu (macOS)
- Native menu bar **on macOS only** — there it is the screen's menu bar, not a second row inside the window, and the App menu with Quit / About is a platform requirement. Windows and Linux show the in-app menu bar alone, which lists a superset of the native menu's entries (Help included).

### 11.4 Capability Detection in UI

```typescript
function isTauri(): boolean {
  if (typeof window === 'undefined') return false; // SSR guard
  const w = window as Window & {
    __TAURI_INTERNALS__?: unknown;
    __TAURI__?: unknown;
  };
  return (
    typeof w.__TAURI_INTERNALS__ !== 'undefined' ||
    typeof w.__TAURI__ !== 'undefined'
  );
}

const fs = isTauri() ? tauriFsAdapter : webFsAdapter;
```

Single FS interface, two adapters. UI never branches on platform beyond this.

Tauri 2 exposes the runtime as `__TAURI_INTERNALS__`; the legacy
`__TAURI__` global is checked only as a v1 fallback. `typeof … !==
'undefined'` is used instead of `'x' in window` so the check reports
false when the property exists but the runtime hasn't populated it
yet. The probe lives in
[`ui/src/lib/platform/index.ts`](../../ui/src/lib/platform/index.ts).

---

## 12. Engine Integration Path

No code-level integration. Workflow:

1. Developer runs `cargo run` on their Amigo Engine game
2. Developer opens `assets/sprites/player.aseprite` in Pincel
3. Edits, saves
4. `amigo_assets` hot-reload watcher detects file change
5. Engine reloads the sprite, animation state machine picks up new tag data, tilemap layer updates, slices update collision rects

This requires no Pincel awareness in the engine. The contract is the file format.

If hot-reload coupling needs to be tighter in the future (e.g., live-editing while the game runs in a window next to Pincel), the engine could expose a local WebSocket that Pincel pushes change-notifications to. Out of scope for Phase 1.

---

## 13. Phase 2 Scope

Documented here so they're not forgotten and so Phase 1 designs accommodate them.

### 13.1 Selection Tools

- Lasso (free-form polygon selection)
- Magic Wand (contiguous-color selection with tolerance)
- Selection arithmetic (add, subtract, intersect via modifiers)

### 13.2 Auto-Tilemap Mode

Painting on a tilemap layer at tile-grid resolution either reuses an existing matching tile or creates a new one. Detection: hash each (tile_w × tile_h) area after a stroke; compare against tileset; auto-link or auto-create.

### 13.3 Custom Brushes

User defines a brush from a selection rect. Brush is stored on the document (Aseprite user-data chunk) or globally (preferences). Pencil tool uses brush instead of single-pixel stamp.

### 13.4 Filters

- Outline (configurable thickness, inside/outside/both)
- Drop Shadow
- Hue/Saturation/Lightness shift
- Color replace

All implemented as commands that produce a new cel state, undoable.

### 13.5 Scripting

If scripting is added later, follow the **amigo-downloader plugin pattern**:

- **rquickjs** (QuickJS-NG bindings) as the embedded JS engine
- **SWC** for TypeScript transpilation at plugin-load time
- Plugins are TypeScript files declaring an entry point — for Pincel, this would be e.g. `transform(sprite, options) → sprite` for batch processing, or `export(sprite) → bytes` for custom export formats
- Heavy work stays in Rust; plugins are orchestration only — they call host APIs to do the actual pixel manipulation, mirroring how downloader plugins call host APIs for HTTP/parsing
- Host-API surface is small and explicit: `getLayer`, `getCel`, `setPixel`, `addLayer`, `addFrame`, etc. No raw memory access.

This is **explicitly not Aseprite-Lua-script-compatible**. Aseprite scripts use a Lua API; reimplementing that surface in JS is not a goal. Pincel scripting is a fresh, smaller API designed for the patterns Pincel users actually need (batch export, palette manipulation, asset-pipeline hooks).

Decision deferred until a concrete need arises. No scripting infrastructure ships in Phase 1.

### 13.6 Animation Enhancements

- Per-tag repeat counts UI
- Pingpong direction visualization on timeline
- Cel linking on write (file size optimization)
- Inbetweening / tweening helpers (Phase 3 maybe — not classic pixel-art workflow)

### 13.7 Collaborative / Cloud

Out of scope indefinitely. Local-first by design.

---

## 14. Open Questions

1. **Indexed-mode rendering performance**: composing indexed → RGBA per frame is acceptable up to N×N. What is N before we need a palette-LUT-on-GPU path? (Benchmark in Phase 1.)
2. **Linked-cel write strategy**: do we always expand on write (simpler, larger files) or detect identical cels and link (smaller files, more complex writer)? Defer; see Phase 2.
3. ~~**PWA deployment target**~~ — resolved 2026-10-11: `https://pincel.amigo-labs.dev/app/` on Cloudflare Workers (see §15). COOP/COEP stay unset until a feature needs `SharedArrayBuffer`; they would go in `website/static/_headers` scoped to `/app/*`. Since 2026-10-13 one Worker serves site and editor (§15).
4. **Tauri auto-update**: Tauri 2's update plugin requires a signed-update server. Do we run one, or push releases via GitHub-Releases-only and require manual update?
5. **Touch/pen UX on tablets**: iPad with Apple Pencil is a real target for pixel art. PWA-on-iPad gives us pen pressure via `PointerEvent`. Tauri-on-iPad doesn't exist (Tauri mobile is Android-first; iOS support is alpha). Decide whether iPad is "PWA only" officially.

---

## 15. Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-05-07 | `.aseprite` as source-of-truth format | Engine already imports it; ecosystem interop; no proprietary lock-in |
| 2026-05-07 | No engine code coupling | Hot-reload via file format is sufficient and cleaner |
| 2026-05-07 | UI: Svelte 5 + Tailwind 4 + shadcn-svelte | Component coverage without lock-in; matches amigo-downloader stack |
| 2026-05-07 | Render: WebGPU + Canvas2D fallback in TypeScript | Avoids wgpu-rs WASM bloat; native browser APIs; small adapter surface |
| 2026-05-07 | `aseprite-writer` as separate publishable crate | Gap in Rust ecosystem; useful beyond Pincel |
| 2026-05-07 | Tilemap and Slice support in Phase 1 | Critical for game-asset workflow with `amigo_tilemap` and 9-patch / collision use-cases |
| 2026-05-07 | Scripting deferred; if needed later, downloader pattern (rquickjs + SWC + TS) | Consistent with amigo-labs tooling; out-of-scope for Phase 1 |
| 2026-05-07 | Document state lives exclusively in Rust memory; canvas / WebGPU is render-target only | Avoids the architectural failure mode that forced Piskel into a multi-year rewrite (Piskel #1245). Browser canvas anti-fingerprinting (Brave today, others tomorrow) corrupts pixel readbacks; in-memory state is immune. |
| 2026-05-07 | `pincel-wasm` ships as npm-importable package with documented public API from Phase 1 | Embedding is a recurring Piskel community ask (#1229, #1246). Designing the boundary in early avoids retrofit. |
| 2026-05-26 | §11.4 `isTauri` check accepts both `__TAURI_INTERNALS__` (Tauri 2) and `__TAURI__` (Tauri 1) | The spec originally specified only `__TAURI__`, but Tauri 2 renamed the global. The shipping implementation in `ui/src/lib/platform/index.ts` accepts both so the same probe works against either runtime. Spec aligned with the implementation. |
| 2026-05-31 | Add `@webgpu/types` as a UI devDependency (M12.5) | Type-only, dev-only, W3C `gpuweb`-maintained. TypeScript's bundled `lib.dom` still omits WebGPU types, so the §4.4 WebGPU adapter can't typecheck without it. No runtime bundle impact. |
| 2026-05-31 | Layer reorder (M13) is a sibling block-swap, group-atomic | `MoveLayer { Up \| Down }` swaps a layer's whole subtree with the adjacent sibling's subtree, so a group moves with its children and no layer crosses a group boundary. Preserves the flat-Vec contiguity invariant that `validate_layer_order` enforces, keeping write round-trips lossless. Cross-group / arbitrary drag-and-drop reorder is deferred (needs reparenting + a richer command). **Superseded 2026-10-12 by `MoveLayerTo`.** |
| 2026-07-09 | `compose()` skips group layers and propagates group visibility to children | A group entry in the flat layer Vec carries no pixels; erroring on it made every grouped file unrenderable. Hiding a group hides its subtree (Aseprite semantics). Group opacity / blend-mode folding (render children into a temp buffer) stays Phase 2. |
| 2026-07-09 | `compose()` implements all separable blend modes; the four non-separable HSL modes (Hue / Saturation / Color / Luminosity) render as Normal until Phase 2 | Files using Multiply/Screen/etc. previously opened but failed compose entirely. Separable modes are cheap per-channel functions under the W3C compositing formula; pixel-exact parity with Aseprite's fixed-point blender is explicitly out of scope (values assert against our own integer formula). HSL modes need lum/sat clip helpers and are rare — Normal fallback keeps the file usable. |
| 2026-07-09 | Pencil/eraser strokes merge into one undo entry, delimited by `Bus::seal()` / wasm `endStroke()` | One `SetPixel` per pixel flooded the 100-entry history cap mid-stroke and made undo peel single pixels. `SetPixel` batches writes and merges while unsealed; the UI seals on pointerup, and `undo()`/`redo()` seal so a new stroke never merges into restored history. |
| 2026-08-20 | Add the `png` crate (0.18) as a `pincel-core` dependency for the §7.3 PNG export | Export needs a real PNG encoder — deflate streams, per-chunk CRCs, IHDR/IDAT framing. Hand-rolling that inside `pincel-core` would be a second codec to maintain with no upside. `png` is the de-facto Rust choice (MIT OR Apache-2.0, `image-rs`-maintained), compiles for `wasm32-unknown-unknown`, and its only transitive deps are `crc32fast` / `flate2` / `miniz_oxide` (already in the tree via `aseprite-loader`) plus `fdeflate`. Encoder-only in the library; the decoder is used in tests to read back what we wrote. |
| 2026-09-06 | Fineliner feature parity: vendor `fineliner-effects` into this workspace as `crates/pincel-effects` (no `pincel-core` dependency, `thiserror` only) | The sister project `amigo-fineliner` is on a maintenance hold with the stated plan of becoming one mode of a unified app; its effect / adjustment set (blur ×4, sharpen ×2, distort ×3, noise ×2, nine colour adjustments) is the first block of that parity work. A git dependency on the private sibling repo would break `wasm-pack` and CI without extra credentials, so the crate is copied verbatim (pixel math unchanged; `as_chunks` reverted to `chunks_exact` for the 1.85 MSRV). Effects run per cel via an undoable whole-buffer command in `pincel-core`, keeping the effects crate platform- and document-free. |
| 2026-09-06 | Add `ab_glyph` (0.2) to `pincel-core` for the Text tool; the caller supplies the font bytes, and the app ships Liberation Sans (SIL OFL 1.1) | Fineliner parity block: text is rasterised to pixels on commit (no vector layer), mirroring Fineliner's ADR-012. `ab_glyph` is pure Rust, wasm-safe and has a tiny dependency tree (`ab_glyph_rasterizer`, `owned_ttf_parser`); core never touches the file system or system fonts, so §5.1's no-platform-deps rule holds. Bold / italic are synthesised (smear / shear) because Phase 1 ships one face. The same font is committed as a core test fixture. |
| 2026-09-06 | Two cores, one shell: `fineliner-core` and `fineliner-wasm` (from `amigo-fineliner@01eef70`) join this workspace unchanged in name; `pincel-effects` serves both cores and `fineliner-effects` is retired; one Svelte shell hosts Pixel and Image mode and lazy-loads the mode's wasm module. Inherited runtime deps: `uuid` (v4 + `js` backend), `image` 0.25 (png/jpeg/webp/bmp/gif/tiff, no default features), `serde`, `serde_json`, `serde-wasm-bindgen`, `js-sys`, `web-sys`, `console_error_panic_hook`; dev deps `ts-rs`, `insta`, `proptest`. MSRV 1.85 → 1.88. | The user wants one app with a pixel-art mode and a paint.net-style image mode. The two cores are not two views of one model: Pincel's `Sprite` is sparse per-cel storage with frames / palette / tilesets / slices and u8 sRGB blending for Aseprite parity; Fineliner's `Document` is a dense canvas-sized layer stack with f32 linear-light blending, a 999-layer / 32767 px envelope and a JSON `CommandSpec` wasm ABI with ts-rs-generated TypeScript. Merging them means one side rewrites its compositor and undo system; keeping both behind one shell preserves every shipped behaviour and test (728 + 282 Rust tests) and costs one extra lazily-loaded wasm bundle. Copying instead of `git subtree` keeps the Pincel tree free of Fineliner's workflows / deploy config; history stays in the archived repo. The effects crate is the one genuine fork (pixel math identical), so it is deduplicated first. MSRV 1.88 lets the vendored code use `slice::as_chunks` as upstream does. |
| 2026-10-11 | Production host: the marketing site at `https://pincel.amigo-labs.dev/` (Worker `amigo-pincel`, Custom Domain) and the editor at `https://pincel.amigo-labs.dev/app/` (Worker `pincel-app`, route `pincel.amigo-labs.dev/app*`). The web build uses Vite `base: '/app/'` and emits into `ui/dist/app/`; the Tauri build keeps base `/`. | Resolves §14 Q3 and website-spec §14 Q1. One hostname keeps the website-spec decision "`/app` as a route, not a separate subdomain" (2026-05-07) and gives the PWA a stable `/app/` scope. Two Workers instead of one combined deploy keep the editor's Rust/wasm build out of Cloudflare's Git-integration image; a Workers route runs before a Custom Domain on the same host, so the split needs no proxy Worker. Workers static assets match the full request path, hence the nested `dist/app/` output. |
| 2026-10-11 | Add `@playwright/test` (1.56.1, exact pin) as a `ui/` dev dependency; e2e specs under `ui/e2e/` run in the CI `ui` job against `vite preview` | §7.4 names Playwright and the M5 exit criterion asks for interaction tests; until now the smoke runs were ad-hoc session scripts that never reached the repo. Dev-only (never in the bundle). Exact pin because each Playwright release is bound to one Chromium build: the local sandbox ships that build preinstalled, CI downloads the matching one. |
| 2026-10-11 | Onion-skin math (§4.2): each neighbour frame is composited alone, every colour channel is moved halfway to the tint (`(c + tint) / 2`; previous = red `#ff0000`, next = blue `#0000ff`), and it is blended source-over *under* the current frame at `alpha · (n − k + 1) / n` for distance `k` of `n` requested frames (farthest first). No wrap-around at the sequence ends. `RenderError::OnionSkinUnsupported` is removed. | §4.2 fixes colours and the 0.3 default alpha but not how tint and alpha combine. A half-way mix keeps the neighbour's shading legible (a full replace turns every frame into a flat silhouette), and a linear fade is what Aseprite does for multi-frame onion skins, so artists read depth the way they expect. Compositing neighbours with the same layer filter and viewport reuses `composite_visible_layers` unchanged and keeps dirty-rect composes correct. |
| 2026-10-11 | New Pixel-mode documents seed the DB32 palette; palettes are editable (recolour / add / remove / reorder) through undoable commands, with remove / reorder refused on indexed sprites | Chosen by the user over PICO-8 / DB16 / none. DB32 is Aseprite's default, so a sprite started in Pincel looks familiar in Aseprite and vice versa, and 32 hues cover game-asset ramps without being overwhelming. Pixels reference palette indices only in indexed mode, which Pincel cannot paint yet; refusing structural edits there keeps a palette tweak from silently recolouring an imported indexed file. |
| 2026-10-11 | Rename `fineliner-core` → `pincel-image-core` and `fineliner-wasm` → `pincel-image-wasm` (npm link name, wasm file `pincel_image_wasm_bg.wasm`, CI / release paths follow) | `amigo-fineliner` is archived and Image mode is part of Pincel; the crate names were the last place the product split showed. `pincel-image-*` mirrors the user-facing mode name. Attribution comments that say where code came from keep the old names. |
| 2026-10-11 | Image-mode project format is **OpenRaster** (`.ora`): ZIP with uncompressed `mimetype`, `stack.xml`, a PNG per layer, `mergedimage.png`, `Thumbnails/thumbnail.png`; opacity / visibility / `svg:*` blend ops / Krita's `edit-locked` / `selected` map the layer properties. New dependency `zip` 9 (`default-features = false`, `deflate-flate2` only → crc32fast, flate2 (already in the tree), indexmap, memchr, typed-path); `stack.xml` is read and written by a small in-crate attribute reader, no XML crate. Image-mode autosave stores the same bytes. | Chosen by the user over a bespoke container and MessagePack + zstd: `.ora` opens directly in GIMP, Krita and MyPaint, so a layered Pincel image is never locked into Pincel. Fineliner's planned `.fln` (ADR-004) was never implemented, so there is nothing to migrate. The pure-Rust deflate backend keeps the wasm build working; stack.xml's flat, attribute-only shape does not justify an XML parser dependency. |
| 2026-10-11 | One effect wire format for both modes: `pincel-image-wasm` takes `apply_effect` / `preview_effect(handle, layer, name, params: Float64Array)` parsed by `pincel_effects::EffectSpec::from_params`, and both editors build their menus and dialogs from `ui/src/lib/effects/catalog.ts`. The image core's JSON `EffectSpec`, its ts-rs mirror and the Image-mode catalogs / dialog are removed. | The two catalogs drove the same pixel math through two encodings and had drifted: Pixel mode sent Levels / Curves points in 0–255 where the effect expects [0, 1] (fixed in the shared catalog's `toParams`). One table, one dialog and one parser leave nothing to drift. Image mode loses its one-click Curves presets in exchange for the editable 5-point Curves dialog. |
| 2026-10-11 | One editor frame for both modes: Pixel mode moves onto the menu bar / tool rail / options bar / collapsible panels / status bar Image mode already had (`ui/src/lib/components/layout/`), with one theme, one inline-SVG icon set and one `shortcuts.ts` keymap. The frame is responsive (docked sidebar ≥ 1024 px, drawer 640–1023 px, phone: ☰ menu, bottom tool strip, panel bottom sheet). Pixel mode gains a background color (right button, filled-shape interior, `X` / `D`) and the lasso moves from `M` to `Q` in both modes. No new dependencies. | The two editors had grown apart: Pixel mode was two rows of text buttons over four equal panel columns (on a phone the canvas did not show at all), Image mode a docked layout with emoji icons. One frame makes the modes feel like one app and puts the responsive work in one place. Icons are hand-drawn rather than an icon package to stay dependency-free and to cover Pixel-only tools (tile stamp, slice). `Q` for the lasso (Aseprite's key) frees `M` for the marquee shapes alone and replaces Image mode's `L`, which collides with Pixel mode's line. The background color follows Aseprite and paint.net and Image mode's existing behaviour. |
| 2026-10-12 | UI language: own typed DE/EN dictionary (`ui/src/lib/i18n/`), no i18n dependency; browser language by default, user choice persisted in IndexedDB; the Tauri menu takes its labels from a Rust table switched by `set_menu_language` | About 700 short strings, no plural-heavy or right-to-left needs: a `Record<MessageKey, string>` per locale gives compile-time completeness checks for free, adds no bundle weight beyond the strings, and keeps CLAUDE.md §9's no-new-dependency rule. Revisit if a third locale with complex plural rules arrives. |
| 2026-10-12 | Pixel layers get a lock and drag-reorder: `SetLayerEditable` flips the existing `Layer::editable` (Aseprite flag `0x2`), `Sprite::layer_locked` folds in the parent groups, and every wasm edit path refuses a locked target; `ReorderLayer { layer, target }` runs `MoveLayer` steps as one undo entry and never leaves the layer's group. Thumbnails come from `compose()` with `LayerFilter::Only` (the layer plus its subtree), nearest-downscaled in wasm | Matches Image mode's panel without new document state; the lock already round-trips through `.aseprite`. Reusing `MoveLayer` keeps the group-contiguity invariant (2026-05-31 row) for free. Image mode had a lock flag with no effect; `pincel-image-wasm` now refuses pixel edits and effects on a locked layer too |
| 2026-10-12 | On-screen Shift / Alt toggles for touch and pen, read through `shiftHeld` / `altHeld` | iPad has no keyboard; latching toggles are the smallest stand-in that keeps one code path for keys and touch. Shown only on coarse pointers or after a touch / pen event, so desktop chrome is unchanged |
| 2026-10-12 | Pen pressure in Image mode: `PressureDynamics { size, opacity }` on `Brush`, `pressure: Vec<f32>` per point in `PencilStroke` / `EraserStroke` (empty = full), linear interpolation along a segment, dab radius floor 0.5 px | Spec §17.4 promised pressure from the start; scaling per dab inside the existing rasterizer needs no new command or undo shape, and old command JSON stays valid. Pixel mode is excluded on purpose |
| 2026-10-12 | Image-mode effects honour the selection: `pincel_image_core::masked_region(before, after, mask)` blends the effect result into the layer by per-pixel coverage (`before + (after − before)·cov/255`, so feathered edges mix proportionally) and crops the undo delta to the mask's bounding box; `apply_effect` returns `false` (no undo entry) when the selection is empty, and the preview uses the same blend | Pixel mode already confined effects to the marquee; Image mode ignored it. Blending by coverage instead of a binary in / out makes feathered selections behave as in paint.net, and the bounding-box delta keeps undo memory proportional to the selection rather than the canvas |
| 2026-10-12 | Palette presets and `.gpl`: `PalettePreset { Db32, Db16, Pico8, GameBoy, Nes }` with `Palette::preset`, `codec::gpl::{read_gpl, write_gpl}` (thiserror `GplError`) and one `ReplacePalette` command; wasm `setPalettePreset` / `importGpl` / `exportGpl` / `palettePresetNames`. No new dependency | `.gpl` is the palette format Aseprite, GIMP, Krita and Lospec all read and write, so it is the one exchange format worth supporting; the five presets cover the common retro targets. A single replace command makes a preset or import one undo step instead of N entry edits |
| 2026-10-12 | One autosave / recovery session for both editors (`AutosaveSession`, §10.3), and Pixel-mode editor state moves into `ui/src/modes/pixel/stores/editor.svelte.ts` with its logic in plain modules beside `PixelEditor.svelte` | The two editors had diverged on four details (clean-document writes, the document-swap race, recovered-document dirtiness, discard closing the dialog); one class settles each once, in the safer direction. The store / module split mirrors Image mode and brings the 3 000-line component down to markup, keys and mount wiring, with no behaviour change (the full e2e suite gates every step) |
| 2026-10-12 | Layers move across groups: `MoveLayerTo { layer, target, place: Above \| Below \| Into }` cuts the layer's contiguous subtree, re-parents its root and splices it back next to (or into) the target; one undo entry; refused into its own subtree or `Into` a non-group. Supersedes the 2026-05-31 sibling-only rule, keeps its contiguity invariant. Pixel's panel lists layers as a tree (header above children) | Drag into / out of groups was the one layer operation Aseprite users would miss. Cut-and-splice of the whole run keeps every descendant's parent and the invariant `validate_layer_order` checks, so `.aseprite` round-trips stay lossless (tested). The tree order puts the drop line where the layer lands; the old reversed flat list showed a group's header below its children |
| 2026-10-12 | Aseprite user-data property maps are kept as opaque bytes (`UserData::properties`, read by our own chunk walk, written verbatim by `aseprite-writer`); tag directions > 3 survive as `Tag::raw_direction` and `AnimationDirection::Other(u8)` | Pincel never edits properties, and `aseprite-loader` 0.4.2's typed parse mis-reads nested maps (it expects a key `DWORD` the spec does not have), so a byte copy is both simpler and exact — §7.1 already prescribes opaque preservation. A side field keeps `TagDirection` exhaustive for playback, the sheet exporter and the wasm string mapping |
| 2026-10-12 | Pen tilt in Image mode: `Brush::angle` (flat tip, degrees, default 45 = the old fixed nib) and `follow_tilt`; per-point `azimuth` travels with pressure as `PenSamples`, `null` / NaN where the pen reports no tilt; the nib angle interpolates the short way round mod 180° | Azimuth is what styluses report reliably (twist is rare); the flat nib is the one tip it visibly changes. Mod-180 interpolation because a flat nib is symmetric, so it never spins through 90° between two samples. Round tips ignore it; Pixel mode stays tilt-free like it stays pressure-free |
| 2026-10-13 | The last Aseprite round-trip gaps close by keeping the data, not interpreting it: External Files entries as a list on `Metadata`, tileset and tile user data on `Tileset` (tiles sparse in a `BTreeMap` by id), slice note and properties on `Slice` beside the overlay colour; our raw chunk walk reads the External Files chunk, the writer emits it first in frame 0 | Pincel edits none of it, so a faithful copy is the whole requirement. A sparse map keeps tile data aligned without touching every `TileImage` and without any command having to maintain it (tiles are only appended). `aseprite-loader` 0.4.2 cannot name entry type 3 (its `Unknown` variant shares the discriminant), which is why the type byte is read from the raw chunk |
| 2026-10-13 | One Worker serves `pincel.amigo-labs.dev`: the marketing site at `/` and the editor at `/app/`, both static assets of `amigo-pincel` (root `wrangler.toml`). Cloudflare's Git integration runs `scripts/cf-build.sh`, which installs Rust + wasm-pack, builds the site into `website/build/` and the editor (release wasm) into `ui/dist/app/`, then copies the latter to `website/build/app/`. Supersedes the two-Worker split of 2026-10-11: `ui/wrangler.toml` (`pincel-app`) and the secret-guarded `deploy-web` release job are removed. | The repo is already connected to Cloudflare; the split needed extra API-token secrets nobody had set, so `/app/` never went live, and PR previews could not show the editor at all. One build makes every preview URL carry the editor and needs no secrets. The cost is Rust in Cloudflare's build image, which `amigo-fineliner` already did the same way (its `scripts/cf-build.sh`); the build takes a few minutes. |
| 2026-10-26 | Releases as in amigo-blinzel: release-please keeps a release PR open on `main` (one workspace version in `Cargo.toml`, `ui/package.json`, `src-tauri/tauri.conf.json`; `CHANGELOG.md` from Conventional Commits, which replace the `[crate] action` messages, CLAUDE.md §2.4). Merging it tags `v<version>` and creates a draft; `release.yml` builds the web bundle and the Tauri installers (Windows x64, macOS universal, Linux x64), SHA256SUMS and the updater's `latest.json`, then publishes. The first release is 1.0.0. The desktop app updates itself silently through `tauri-plugin-updater` (new dependency): one check a day, 10 s after start, signature checked against the Tauri updater key, installed for the next start (Windows: when the app exits); Help menu "Check for Updates…" and "Install Updates Automatically" (`updater.json` in the app config dir). Debug builds and Linux .deb / .rpm installs don't update. Supersedes the push-to-main patch releases and the manual `release-desktop` workflow. | One release flow for the amigo apps; a changelog and a reviewable version bump instead of a tag per push; installers and update manifest published together, so no updater sees a release without its files. The Tauri plugin instead of blinzel's own WinHTTP / Ed25519 updater, because Pincel is a Tauri app and the plugin already downloads, verifies and swaps per platform. Its minisign signature replaces blinzel's signed SHA256SUMS (those stay, unsigned). Not ported: blinzel's rollback after three failed starts (no Tauri mechanism); .deb / .rpm would need root, so they stay with the package manager. |
| 2026-10-26 | Desktop setup and start-up as in amigo-blinzel, without its resident mode / tray (closing ends the app). Windows: Tauri's NSIS setup per user without admin, English / German by system language, `/S` silent, hooks (`src-tauri/windows/hooks.nsh`) that register Capabilities / RegisteredApplications / OpenWithProgids and open Default apps after a first interactive install; winget templates in `packaging/winget/`. Linux: `pincel --install` / `--uninstall` from the AppImage (`~/.local/share/pincel`, `~/.local/bin/pincel`, desktop entry, MIME, icons); only that copy updates itself. Start: window hidden on the app background until the page has loaded (≤ 400 ms), second start unminimizes and focuses. | Same install and start-up experience across the amigo apps. Tauri's NSIS setup instead of blinzel's own setup dialog: it already does per-user install, shortcuts, uninstall entry and silent mode, the hooks add what it lacks (Default apps listing). Resident mode and tray were explicitly not wanted. No Windows version gate (blinzel requires 11 24H2 for its Direct3D path; WebView2 runs on 10 1809+). |
| 2026-10-27 | Updater signing key: the one password-protected key of amigo-labs' desktop apps (minisign `5C882412026EA1D4`, organization secrets `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD`, shared with amigo-downloader) replaces Pincel's own key (`313783124C4CF977`) from 1.1.1. `release.yml` checks the key ID of every signature against `plugins.updater.pubkey` before publishing. | One key and one rotation for all of the organization's Tauri apps. The switch needed no transition release: no copy of 1.0.1 / 1.1.0 had been downloaded. `tauri build` signs with any key the secret holds, so a mismatch (e.g. a repository secret shadowing the organization's) would otherwise ship updates nobody can install. |
| 2026-10-27 | The organization's updater key is replaced by a new password-protected pair, minisign `3098504C1538B8F6` (same organization secrets; amigo-downloader gets the same public key). 1.1.2 is the first release signed with it; v1.1.1 stays an unpublished draft. | The password for `5C882412026EA1D4` did not match its key in the organization secrets (1.1.1's builds failed with "Wrong password for that key"), and no app had shipped an update signed with it, so a fresh pair, tested before it was stored, cost nothing. |
| 2026-10-27 | `compose()` renders the four non-separable HSL modes (Hue / Saturation / Color / Luminosity) with the W3C compositing-and-blending definitions: `Lum` = 0.3·R + 0.59·G + 0.11·B, `SetLum` / `ClipColor` / `Sat` / `SetSat` in `f64`, the blend term rounded half away from zero, then the same alpha compositing as the separable modes. Supersedes the 2026-07-09 "render as Normal until Phase 2". | A `.aseprite` with a Color or Luminosity layer looked different in Pincel than in Aseprite, which uses the same definitions and weights. The modes are a few pure functions, not a Phase 2 feature. Aseprite pixel-exact parity stays out of scope (its fixed-point rounding differs); tests pin values from an independent float64 implementation and the luminance invariants. |
| 2026-10-27 | The native menu bar is macOS-only: a `NATIVE_MENU` const (`cfg!(target_os = "macos")`) gates `app.set_menu` in `setup()` and turns the `set_recent_menu` / `set_menu_language` commands into no-ops elsewhere, so the renderer keeps one code path. Windows and Linux show only the in-app menu bar, which gains a Help menu (About Pincel, and under Tauri "Check for Updates…" / "Install Updates Automatically") and File ▸ Open Recent in Image mode. `hasNativeMenu()` (`ui/src/lib/platform/`) replaces `isTauri()` as the gate that leaves `NATIVE_MENU_ACTIONS` to the menu. | Windows and Linux showed two menu bars stacked, the in-app one a superset of the native one: M11.3 installed the native menu on every platform and the one-frame rebuild (2026-10-11) rendered the in-app bar without asking the host. macOS cannot drop its menu — the menu bar belongs to the screen rather than the window, the App menu is a platform requirement, and WKWebView routes Cmd+C / V through Edit ▸ Copy / Paste; WebView2 and WebKitGTK handle those chords in the renderer, as they already do in the web build, so the predefined items are not missed there. Everything the native menu was the only access to moves into the in-app bar, so no platform loses a feature. `cfg!` rather than `#[cfg]` because CI builds the desktop shell on Linux only: an attribute would stop type-checking the macOS path entirely, and would need ~10 annotations plus a `dead_code` allow on `RecentMenuItem`. |
| 2026-10-27 | Image mode gains Gradient, Recolor and Smudge, and the tool rail lists Line, the four shapes, Pan, Zoom and the two selection moves as their own buttons. The set is written down in §5.4 — the first tool spec Image mode has in this repository. | The rail showed ten buttons against paint.net's nineteen, and some of the gap was not even missing code: `Shape::Line` existed with `L` bound in the shared keymap and no group to select, the shapes were a dropdown where §9.2 asks for rail variants, and pan and zoom were gestures a tablet had no way to reach. Gradient, Recolor and Smudge were genuinely absent — `STATUS.md` listed them as deliberately not ported when Fineliner's own M13 never shipped. A clone stamp stays out: it needs a source anchor that survives the stroke and a second cursor, which is a UI problem of its own. Image mode's tools had no spec in this repository at all — the code's `spec §16.2` comments point at the archived sibling's `fineliner.md` — so §5.4 states the set, and §5.2 is retitled as Pixel mode's. |
| 2026-10-27 | The wasm layer is published as `@amigo-labs/pincel-wasm` (crate and directory keep the name `pincel-wasm`; the npm name comes from `wasm-pack --scope amigo-labs`), and `publish-npm` authenticates through npm's OIDC trusted publishing instead of `NPM_TOKEN`, gated by the repository variable `PUBLISH_NPM`. The trusted publisher names `release-please.yml`, the entry workflow of an automatic release, rather than the `release.yml` that holds the `npm publish`; a manual `workflow_dispatch` rebuild therefore skips the npm step. The first version is published by hand once, because a trusted publisher is configured in the package's own settings. | `publish-npm` failed for 1.1.0 and 1.3.0 with `404 Not Found - PUT …/pincel-wasm`, npm's answer when the identity may not publish the package. npm revoked every classic token in December 2025 and now steers automation to granular "Read and write (stage only)" tokens, which may not publish a version at all; direct publishing through bypass-2FA tokens ends in January 2027. Trusted publishing needs no secret, so nothing can leak from the workflow, and npm attaches a provenance attestation on its own. npm validates the entry workflow of the run and allows one trusted publisher per package (npm/documentation#1755), so the reusable `release.yml` cannot be the registered identity and the two publish paths cannot both be covered — the automatic one wins, the manual one falls back to the by-hand sequence. The scope matches every other package of the organization and the name was still free to choose, nothing having been published yet; `@amigo-labs/pincel` stays reserved for the embed package of §17.5. |
| 2026-10-28 | Each New card on the start screen, and the New dialog, carries an "Open like this on startup" checkbox. Ticking it stores `{ mode, width, height }` under the IndexedDB pref `autoNew` (one mode at a time, ticking one card clears the other); on the next launch the shell reads it before drawing anything and opens an empty document of that mode and size instead of the start screen. A file the OS hands over at launch (`open-file`) still replaces that empty document. | Users who always start the same way skip a click. The checkbox has to sit in the New dialog too: with autostart on the start screen never shows, so File ▸ New… is where it gets switched off again. |
| 2026-10-28 | A New image (start-screen card and New dialog) defaults to the size of the image on the clipboard, like paint.net. The size is read when the card or dialog opens, through the async Clipboard API and `createImageBitmap` (no canvas readback); a size the user already typed is kept. The unprompted read only runs where the Permissions API knows `clipboard-read` (Chromium, the Windows WebView2 build) and hasn't denied it; elsewhere the defaults stay. Pixel sprites keep 64×64. | Firefox, Safari and WebKitGTK show a paste prompt for every unprompted clipboard read, which would pop up each time a New card opens. Chromium asks once and remembers the answer. |
| 2026-10-28 | Image mode pastes a clipboard image as a new layer (§5.4), as one `PasteLayer` command that also grows the canvas when the user chose *Expand canvas*. The wasm layer takes the encoded bytes through `paste_layer(handle, bytes, expand)`, not through the JSON `CommandSpec`. Ctrl+V is served by the browser's `paste` event (no permission), Edit ▸ Paste by the async Clipboard API. | paint.net pastes into the current layer as a floating, movable selection; that needs selection transforms, which are Phase 2 (§13). A new layer keeps the paste separate and movable with the Move tool, and matches paint.net's Paste into New Layer. Expand + paste in one command keeps it one undo step. Encoded bytes in JSON would bloat the spec channel and its generated TypeScript mirror. |

---

## 16. Phase 1 Milestones

| # | Milestone | Exit Criterion |
|---|-----------|----------------|
| M1 | `pincel-core` document model + commands + undo/redo, no I/O | Unit-tested command application + revert for image cels; CI green |
| M2 | `aseprite-loader` integration, read fixture set | All test fixtures load into Pincel model without loss |
| M3 | `aseprite-writer` MVP (image layers, palette, frames, tags) | Round-trip test passes for image-only documents |
| M4 | `pincel-wasm` + minimal Svelte UI: open file, view, pencil tool, save | Demo: open `.aseprite`, paint, save, reopen in Aseprite — file is valid |
| M5 | Full tool set (Section 5.2 except Tilemap Stamp + Slice Tool) | Each tool has interaction tests; UX feels native |
| M6 | Tilemap support: read/write, Tilemap layer, Stamp tool, Tileset Panel | Round-trip test for tilemap fixtures; `amigo_assets` loads Pincel-saved tilemap correctly |
| M7 | Slice support: read/write, Slice tool, 9-patch + pivot UI | Round-trip test; engine can read Pincel slices as collision rects |
| M8 | PWA polish: service worker, IndexedDB autosave, recent files | Lighthouse PWA score ≥ 90 |
| M9 | Tauri build: native FS, OS dialogs, menu bar, file association | Installable on Windows + macOS + Linux; opens `.aseprite` files via OS |
| M10 | Performance pass: dirty-rect compose, profiling | 256×256 sprite at zoom 32 maintains 60 fps on M1 / mid-tier Windows |

Estimated Phase 1 effort: 3–4 months solo with focused work; longer with the `aseprite-writer` work being more thorough than minimal.

---

## 17. Lessons from Prior Art (Piskel)

Piskel is the closest peer to Pincel: web-based, open-source, sprite-and-animation-focused. It has 12.4k GitHub stars, ran for ~14 years, and entered a multi-year modernization effort starting 2026. Reading its issue tracker is a gift — both for what to avoid and for what users have begged for over a decade.

### 17.1 Architectural Anti-Pattern: Canvas as Source of Truth

Piskel issue **#1245** ("Move Away from Canvas as Data Source") documents that Piskel uses `<canvas>` elements as the authoritative pixel store: layer compositing runs through canvas APIs, file imports go via canvas-draw-then-readback, exports re-render from canvas. This worked for 14 years until browser anti-fingerprinting started "nudging" canvas pixel values. Brave broke first; other browsers will follow. Piskel's stated solution direction is exactly the architecture Pincel ships with on day one:

- Pixel state in pure memory (`Vec<u8>`, owned by `pincel-core`)
- All compositing manual, in Rust
- Canvas / WebGPU treated as render-target only, never a data source

This is encoded in the `compose()` contract (Section 4) and is non-negotiable. **Never read pixels back from the canvas.** If a feature seems to require it, it's the wrong design.

### 17.2 Stack Validation

Piskel issue **#1246** ("Piskel modernization") describes the migration the Piskel team is currently running:

- Build system → Vite
- JS → TypeScript
- IIFEs / namespaces → ES modules
- E2E → Playwright
- Goal: ship as an importable npm library

This is precisely the Pincel stack as specified in Sections 9 and 11. Pincel starts where Piskel hopes to arrive after multi-year migration. No design changes needed; this is validation.

### 17.3 Recurring Feature Requests → Pincel Mappings

These have been requested for years in Piskel's tracker. Pincel should accommodate them by design, not as afterthoughts.

| Piskel Issue | Request | Pincel Approach |
|--------------|---------|-----------------|
| #1264 | Rotate / flip selection | Phase 2 selection ops; Aseprite has it, free with format compatibility |
| #1243 | Text tool | Phase 2; bitmap-font rendering, optional |
| #1234 | Bucket fill across multiple frames | First-class `FrameScope { Current, AllFrames, TagRange, Selection }` modifier on every applicable tool / command |
| #1232 | Better downscaling / resize quality | Configurable resampling: Nearest, Scale2x/3x/4x, hqx, RotSprite |
| #1229 | npm-package / embeddable | Section 15 decision; `pincel-wasm` is a documented public API from Phase 1 |
| #681 | JSON export | Section 7.3; sprite-sheet sidecar JSON in Aseprite `--data` format for ecosystem compat |
| #661 | CLI args (open file from terminal) | Tauri build supports this natively; UI receives `initial-file` event on launch |
| #636 | Pixel grid display option | Standard view toggle; on by default at zoom ≥ 8 |
| #562 | Bigger canvases | No artificial limit; document model is bounded only by available memory |
| #403 | Align to center | Sprite-level transform commands (rotate, flip, align, trim) |
| #402 | Tiling / seamless mode | Phase 2; Aseprite-style canvas wrap-around for seamless authoring |
| #658 | Better camera / pan ergonomics | Standard: space-drag, middle-click-drag, mouse-wheel-zoom-to-cursor |
| #762 | Better UI space usage | No persistent top-banner / account chrome; panels collapsible and dock-tabbable (Section 9.2) |

### 17.4 Mobile / Tablet Support

Piskel README states: *"There is no support for mobile."* This is a self-imposed limitation rooted in event-handling assumptions made in 2012. The pixel-art community on iPad with Apple Pencil is large and underserved.

Pincel's input layer uses the modern `PointerEvent` API end-to-end, capturing pen pressure, tilt, and pointer-type from day one (Section 9.4). Two-finger pan/zoom is part of the standard input handling, not a tablet special case. On-screen Shift / Alt toggles stand in for the keyboard (§9.4). In Image mode the brush and eraser read stylus pressure per sample (coalesced events included) and scale the dab size and / or opacity, each switchable in the tool options; mouse and touch paint at full pressure. The flat tip has an angle (default 45°) and, with "Tilt turns the nib" on, follows the stylus azimuth per sample (Pointer Events `azimuthAngle`, else `tiltX` / `tiltY`); an upright pen or one without tilt uses the angle. Pixel-mode tools stay pressure-free — a 1-px pencil has nothing to scale. Pincel's PWA build is the reference target for iPad — Tauri-on-iPad is not a supported path (Tauri mobile is Android-first, iOS support is alpha).

This means Pincel should design its tool palette and shortcut layout to be **usable without keyboard modifiers**. Long-press for context menus, on-screen modifier keys (the Aseprite mobile pattern), pinch-to-zoom. Validate this in M5 (full tool set milestone).

### 17.5 Embeddability as a Phase-1 Concern

Repeated Piskel requests for "Piskel as a React component" (#1229) and "Piskel as a JS library" (#1246 stated goal) show this is a real recurring need. Game engines, level editors, and asset-pipeline tools want an embedded pixel editor, not a separate app.

`pincel-wasm` ships with a stable, documented public API from Phase 1 (M4 acceptance):

```typescript
// Public package surface
import { Pincel, ColorMode, ToolId } from '@amigo-labs/pincel';

const pincel = await Pincel.create({
  width: 64,
  height: 64,
  colorMode: ColorMode.Rgba,
  // ... options
});

pincel.mount(htmlElementForCanvas);
pincel.openFile(asepriteBytes);
const bytes = await pincel.saveAseprite();

pincel.on('change', () => {/* … */});
pincel.on('save', (bytes) => {/* … */});
```

The Pincel app itself is then a thin shell on top of this same package. This is a deliberate constraint: if the public API is sufficient to build the app, it's sufficient for embedders.

---

## 18. Competitive landscape

Snapshot as of 2026-10-28, checked against vendor sites, docs, release pages and app stores (§18.6). GIMP and Photoshop are left out on purpose: Pincel doesn't compete with full-scale photo suites. §17 covers Piskel's architecture in depth; this section compares features and positioning.

Legend: ✓ yes · ✗ no · ~ partial (see note) · ? unverified. An entry marked \* comes from prior knowledge and was not re-checked in this pass. Re-check before quoting any of it externally; several of these tools ship monthly.

### 18.1 Pixel-art editors — platforms and business

| Tool | Win | Mac | Linux | Web | iPad | Android | Price / license | Latest release | Scripting | Embeddable |
|------|-----|-----|-------|-----|------|---------|-----------------|----------------|-----------|------------|
| **Pincel** | ✓ | ✓ | ✓ | ✓ PWA, offline | ~ PWA | ~ PWA | Free, MIT OR Apache-2.0 | active | ✗ (§13.5) | ✓ npm `@amigo-labs/pincel-wasm` |
| Aseprite | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ | $19.99 one-time; source-available EULA | 1.3.18.6, 2026-09 | ✓ Lua + extensions | ~ CLI batch |
| LibreSprite | ✓ | ✓ | ✓ | ~ old online build | ✗ | ~ old APK | Free, GPL-2.0 (pre-EULA Aseprite fork) | 1.3, 2026-10 | ✓ JavaScript | ✗\* |
| Pixelorama | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ APK | Free, MIT | 1.2.3, 2026-09 | ✓ extensions, shaders | ~ CLI export |
| Piskel | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ | Free, Apache-2.0 | 0.15.0, 2018 (master still gets fixes) | ✗\* | ✗\* |
| Lospec Pixel Editor | ✗ | ✗ | ✗ | ✓ desktop browsers | ✗ | ✗ | Free, GPL-3.0 | discontinued (last commit 2024-06) | ✗ | ✗ |
| Pro Motion NG | ✓ | ? | ? | ✗ | ✗ | ✗ | Free edition + $19; proprietary | V8.0.11, 2025-02 | ~ plugin interface | ? |
| Pyxel Edit | ✓ | ✓ | ✗ | ✗ | ✗ | ✗ | Paid beta; proprietary | no dated release found | ? | ✗ |
| Pixaki 4 | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | $20 one-time (Pro); proprietary | 4.6, 2025-02 | ✗? | ✗ |
| Resprite | ✓ | ✓ | ✓ (since 2026-10) | ✗ | ✓ | ✓ | Trial, then subscription or $19.99; proprietary | 1.32.6, 2026-10 | ? | ✗? |

### 18.2 Pixel-art editors — features

| Tool | Layers / groups / blend | Timeline / onion / tags | Tilemap | Slices / 9-patch | Indexed | `.aseprite` R/W | Pixel-perfect | Adjustments / filters | Magic wand | Text | Pen pressure | PSD |
|------|---|---|---|---|---|---|---|---|---|---|---|---|
| **Pincel** | ✓ / ✓ / ✓ | ✓ / ✓ / ✓ | ✓ | ✓ / ✓ + pivot | ~ read / write, painting Phase 2 | ✓ native | ✗ | ✓ (shared `pincel-effects`) | ✓ | ✓ | ~ Image mode only | ✗ |
| Aseprite | ✓ / ✓ / ✓ | ✓ / ✓ / ✓ (tag groups) | ✓ | ✓ / ✓ + pivot | ✓ | ✓ native | ✓ | ~ hue/sat, brightness, curves\* | ✓\* | ✓ | ✓ | ✗ (extensions may add it) |
| LibreSprite | ✓ / ✗ / ?\* | ✓ / ✓ / ✓\* | ✗\* | ✗\* | ✓\* | ~ old format, groups flattened | ✓\* | ~\* | ✓\* | ✓ | ✓ | ✗\* |
| Pixelorama | ✓ / ✓ / ✓ | ✓ / ✓ / ✓ (+ keyframes) | ✓ rect / iso / hex, autotile | ? | ✓ | ✓ import / ✓ export (1.2.2) | ✓ | ~ curves, HSV, gradient map, layer effects | ✓\* | ✓ | ✓ | ✓ import |
| Piskel | ✓ / ✗\* / ✗\* | ✓ / ✓ / ✗\* | ✗ | ✗ | ✗\* | ✗\* | ? | ✗ | ✓\* | ✗ | ? | ✗\* |
| Pro Motion NG | ✓ / ✓ / ? | ✓ / ✓ / ? | ✓ | ? | ✓ | ? | ✓ | ? | ? | ✓ bitmap fonts | ✓ | ? |
| Pixaki 4 | ✓ / ✓ / ✓ | ✓ / ✓ / ? | ? | ? | ? | ✓ (Pro) | ✓ | ? | ~ colour select | ? | ? | ✓ layered (Pro) |
| Resprite | ✓ / ? / ✓ + clipping masks, layer styles | ✓ / ✓ / ? | ✓ (Tiled `.tmx` / `.tsx`) | ? | ? | ✓ / ✓ | ✓ | ~ HSV, brightness / contrast, sharpen | ? | ✓ TTF / OTF + pixel fonts | ✓ | ✓ / ✓ |

Lospec and Pyxel Edit are left out of the feature table: neither publishes a feature list that could be checked, and Lospec is discontinued.

### 18.3 Image editors — platforms and business

| Tool | Win | Mac | Linux | Web | iPad | Android | Price / license | Latest release | Scripting / plugins | Embeddable |
|------|-----|-----|-------|-----|------|---------|-----------------|----------------|---------------------|------------|
| **Pincel** | ✓ | ✓ | ✓ | ✓ PWA, offline | ~ PWA | ~ PWA | Free, MIT OR Apache-2.0 | active | ✗ | ✓ npm (Pixel core) |
| Paint.NET | ✓ | ~ Wine (5.2 beta) | ~ Wine (5.2 beta) | ✗ | ✗ | ✗ | Free (paid Store version); proprietary | 5.1.12, 2026-03; 5.2 beta 2026-10 | ✓ .NET effect / FileType plugins | ✗ |
| Pinta | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ | Free, MIT | 3.1.2, 2026-04 | ✓ add-ins | ✗ |
| Photopea | browser | browser | browser | ✓ works offline once loaded | browser | browser | Free with ads; Premium subscription; proprietary | continuous | ✓ scripts / actions | ✓ iframe + JSON API |
| Pixlr E | ~ desktop app | ? | ✗ | ✓ | ~ Express app | ~ Express app | Free tier; Plus / Premium subscriptions; proprietary | continuous | ✗\* | ? |
| miniPaint | ✗ | ✗ | ✗ | ✓ self-hostable | browser | browser | Free, MIT | 4.14.3, 2026-04 (maintenance) | ~ | ✓ iframe, no formal API |
| Krita | ✓ | ✓ | ✓ | ✗ | ✗ | ✓ (5.x) | Free, GPL-3.0 (paid store builds) | 5.3.4.1 / 6.0.4, 2026-09/10 | ✓ Python\* | ✗ |
| Affinity (Canva) | ✓ | ✓ | ✗ | ✗ | ~ announced | ✗ | Free for individuals since 2025-10-30 (Canva account; AI needs Canva Premium); proprietary | 3.3, 2026-09 | ✓ JavaScript (3.3) | ✗ |

### 18.4 Image editors — features

| Tool | Layers / groups / blend | Adjustments / filters | Magic wand | Text | Pen pressure | PSD | Animation | Pixel-art features |
|------|---|---|---|---|---|---|---|---|
| **Pincel** | ✓ / ✗ (Pixel mode has groups) / ✓ | ✓ | ✓ | ✓ | ✓ size, opacity, tilt | ✗ | ✓ in Pixel mode | ✓ Pixel mode (tilemaps, slices, palettes, `.aseprite`) |
| Paint.NET | ✓ / ✗ (folders planned for 6.0, 2027) / ✓\* | ✓ levels, curves, hue/sat | ✓ | ✓ simple | ✓ | ~ third-party plugin\* | ✗ | ✗ |
| Pinta | ✓ / ✗\* / ✓ | ✓ levels, curves, effects | ✓ | ✓ | ? | ? | ✗ | ✗ |
| Photopea | ✓ / ✓\* / ✓\* | ✓ | ✓\* | ✓ | ✓ | ✓ core feature | ~ frames via layer names, no onion skin | ✗ |
| Pixlr E | ✓ / ✓ / ✓ | ✓ levels, curves, hue/sat | ✓ | ✓ | ✓ | ~ simple PSDs only | ✗ | ✗ |
| miniPaint | ✓ / ✗\* / ✓ | ~ basic adjustments + effects | ✓ | ✓ | ? | ? | ~ animated GIF export | ✗ |
| Krita | ✓ / ✓ / ✓ | ✓ | ✓\* | ✓ | ✓ | ✓\* | ✓ timeline, onion skin | ~ no indexed mode |
| Affinity | ✓ / ✓ / ✓ | ✓ | ✓\* | ✓ | ✓\* | ✓ layers kept | ✗\* | ✗ |

### 18.5 Positioning

**What nobody else combines.** No tool in §18.1–18.4 pairs an Aseprite-class sprite editor with a paint.net-class image editor in one app. And none runs the same build as a native desktop app (Windows, macOS, Linux) and as an offline-capable PWA. The two core arguments:

- **paint.net has no Mac, Linux or web build.** Its 5.2 Wine support is experimental.
- **Aseprite has no browser or tablet build.** Pincel's PWA covers the browser and the iPad.

Embedding through npm is a third differentiator (§17.5). Photopea's iframe API is the only comparable offer, and it is closed source with paid white-labelling.

**Closest competitors and the risks they carry.**

| Competitor | Why it matters | What Pincel answers with |
|------------|----------------|--------------------------|
| Pixelorama | Free, MIT, web + desktop + Android. Since 1.2.2 it writes `.aseprite` too, and it already has indexed painting, autotiling, layer effects and PSD import. The closest overall match. | Native `.aseprite` as the source of truth (not an import / export path), slices with 9-patch + pivot, a full image editor beside it, an embeddable core |
| Resprite | The fastest-moving commercial tool: every platform, `.aseprite` + PSD read / write, Tiled tilemaps, layer styles | Free and open source, a browser build, an image mode |
| Aseprite | The reference for pixel features and Lua scripting | Same file format, browser + tablet reach, an image mode |
| Photopea | The strongest image editor in the browser, PSD-centred | No ads, native desktop builds, pixel-art mode, open source |
| Paint.NET 5.2 / Pinta 3 | The paint.net feature set on Windows (Paint.NET) and on Mac / Linux (Pinta, MIT) | Pixel-art mode, a browser build, tablet input |

**Gaps the comparison exposes** (candidates for later phases, not commitments):

- **Pixel-perfect pencil stroke.** Aseprite, LibreSprite, Pixelorama, Pixaki, Resprite and Pro Motion all have it; Pincel doesn't.
- **Pen pressure in Pixel mode.** Aseprite, LibreSprite, Pixelorama and Resprite support it; Pincel has it in Image mode only.
- **Indexed painting, autotiling, scripting.** Already listed in §13.
- **PSD import.** Pixelorama, Resprite, Pixaki and Photopea have it; Pincel opens neither PSD nor Krita files.
- **Layer groups in Image mode.** Pixel mode already has them.

### 18.6 Sources

- aseprite.org (release notes, docs, buy, FAQ) and github.com/aseprite/aseprite/releases
- github.com/Orama-Interactive/Pixelorama (README, releases) and pixelorama.org
- github.com/LibreSprite/LibreSprite/releases and libresprite.github.io
- github.com/piskelapp/piskel
- github.com/lospec/pixel-editor
- cosmigo.com and the Pro Motion NG Steam page
- pyxeledit.com
- pixaki.com and its App Store listing
- resprite.fengeon.com (docs, release notes) and the Resprite Steam page
- paint.net (features, roadmap, blog)
- github.com/PintaProject/Pinta and pinta-project.com
- photopea.com/learn and photopea.com/api
- pixlr.com (pricing, editor, desktop)
- github.com/viliusle/miniPaint
- krita.org (download, release notes)
- affinity.studio, AppleInsider (2025-10-30, Affinity goes free) and CG Channel (2026-09, Affinity 3.3)

---

*End of specification v0.1. This is a living document — amend in place via PR; significant decisions go in Section 15.*
